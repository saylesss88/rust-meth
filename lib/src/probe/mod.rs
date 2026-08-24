//! A utility for generating ephemeral, minimal Cargo projects ("probes") used to query
//! Language Server Protocol (LSP) intelligence like autocompletions or go-to-definitions.
//!
//! A `Probe` creates a temporary directory containing a valid Cargo package with a single source file.
//! The source file declares an isolated variable statement `let _x: TYPE = todo!();` followed by a target
//! interaction point (such as `_x.` or `_x.method()`).

mod cache;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub use cache::{
    CacheEntry, PersistentCacheEntry, cache_entries, clear_persistent_cache, clear_probe_cache,
    persistent_cache_dir, persistent_cache_entries,
};
use cache::{
    CacheKey, CachedProbe, global_cache, load_persistent_probe, ra_version, save_persistent_probe,
};

// -- Counter --

/// Global atomic counter ensuring that concurrently generated probe projects
/// receive unique names within the OS temporary directory.
static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);

// Preamble added to every probe file so common std types resolve without
// the user needing to fully qualify them (e.g. `HashMap` not `std::collections::HashMap`).
const PREAMBLE: &str = "\
#![allow(unused_imports)]
use std::collections::*;
use std::sync::*;
use std::cell::*;
use std::rc::Rc;
use std::io::{self, Read, Write, BufRead};
use std::fmt;
use std::ops::*;
use std::path::{Path, PathBuf};
";

/// Represents an ephemeral Cargo project written to disk for LSP interrogation.
///
/// Backed by a cache [`Arc<CachedProbe>`] the directory is only deleted when
/// both the cache entry and this `Probe` are dropped.
pub struct Probe {
    /// Shared ownership of the underlying cached probe data.
    #[allow(dead_code)]
    inner: Arc<CachedProbe>,
    /// The 0-indexed line number in `src/main.rs` pointing to the target interaction point (the dot trigger).
    pub dot_line: u32,
    /// The 0-indexed character/column offset pointing exactly after the dot (`_x.`) in `src/main.rs`.
    pub dot_col: u32,
    /// The absolute path to the root directory of the temporary Cargo project.
    pub dir: PathBuf,
    /// The absolute path to the generated `src/main.rs` file.
    pub src_path: PathBuf,
}

impl Probe {
    /// Creates a new probe project without dependencies (for stdlib types).
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if creating the underlying probe project directory
    /// or writing its files fails.
    pub fn new(type_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, None, None)
    }

    /// Creates a new probe project with optional dependencies (for 3rd party crates).
    ///
    /// # Arguments
    /// * `type_name` - The Rust type to query (e.g., "`Vec<u8>`", "`serde_json::Value`")
    /// * `deps` - Optional TOML dependencies section (e.g., "`serde_json` = \"1.0\"")
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if generating the probe files or writing the dependency
    /// configuration fails.
    pub fn new_with_deps(type_name: &str, deps: Option<&str>) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, deps, None)
    }

    /// Creates a probe project with optional dependencies and persistent caching.
    ///
    /// Passing `ra_path` enables the persistent cache, the probe directory is saved
    /// to `$XDG_CACHE_HOME/rust-meth/probes/` and reused across process restarts,
    /// skipping Cargo resolution on subsequent calls.
    ///
    /// ## Errors
    ///
    /// Errors on I/O failures, Cache/File System errors
    pub fn new_with_deps_cached(
        type_name: &str,
        deps: Option<&str>,
        ra_path: &Path,
    ) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, deps, Some(ra_path))
    }

    /// Creates a probe file with `_x.METHOD_NAME()` for go-to-definition queries.
    /// The cursor position points at the start of the method name.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the workspace initialization or file creation fails
    /// on disk.
    pub fn for_definition(type_name: &str, method_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), None, None)
    }

    /// Creates a probe file for go-to-definition with custom dependencies.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the underlying project boilerplate, file buffers,
    /// or custom dependency sections cannot be written.
    pub fn for_definition_with_deps(
        type_name: &str,
        method_name: &str,
        deps: Option<&str>,
    ) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), deps, None)
    }

    /// Creates a go-to-definition probe with custom dependencies and persistent caching.
    ///
    /// ## Errors
    ///
    /// On I/O failures, and Cache/File System errors
    pub fn for_definition_with_deps_cached(
        type_name: &str,
        method_name: &str,
        deps: Option<&str>,
        ra_path: &Path,
    ) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), deps, Some(ra_path))
    }

    /// Infers a minimal Cargo dependency string from a type path when no explicit
    /// `--deps` argument is provided.
    ///
    /// Returns `Some("crate_name = \"*\"")` if the leading path segment starts with
    /// a lowercase letter (indicating a third-party crate), or `None` for stdlib/
    /// primitive types whose leading segment is uppercase (e.g. `Vec`, `HashMap`).
    ///
    /// # Examples
    /// ```text
    /// assert_eq!(infer_dep("serde_json::Value"), Some(r#"serde_json = "*""#.into()));
    /// assert_eq!(infer_dep("Vec<u8>"),           None);
    /// assert_eq!(infer_dep("HashMap<K, V>"),     None);
    /// ```
    fn infer_dep(type_name: &str) -> Option<String> {
        if !type_name.contains("::") {
            return None; // primitives, String, Vec<u8>, etc. never need an inferred dep
        }
        let crate_name = type_name.split("::").next()?;
        if crate_name.chars().next()?.is_ascii_lowercase() {
            Some(format!(r#"{crate_name} = "*""#))
        } else {
            None
        }
    }

    /// Internal probe creation logic shared by all constructors.
    ///
    /// Checks the global cache first. On a hit, clones the [`Arc`] and returns
    /// immediately without touching the filesystem. On a miss, creates the probe
    /// directory, writes the files, inserts into the cache, and returns.
    ///
    /// # Arguments
    /// * `type_name` - The Rust type to query
    /// * `method_name` - If Some, creates a definition probe; if None, creates a completion probe
    /// * `deps` - Optional TOML dependencies to add to Cargo.toml
    fn create_probe(
        type_name: &str,
        method_name: Option<&str>,
        deps: Option<&str>,
        ra_path: Option<&Path>,
    ) -> std::io::Result<Self> {
        let effective_deps = deps
            .map(str::to_owned)
            .or_else(|| Self::infer_dep(type_name));

        let key = CacheKey {
            type_name: type_name.to_string(),
            effective_deps: effective_deps.clone(),
            method_name: method_name.map(str::to_owned),
        };

        // 1. In-process cache
        if let Some(probe) = Self::from_memory_cache(&key) {
            return Ok(probe);
        }

        // 2. Persistent cache
        if let Some(ra) = ra_path
            && let Some(probe) = Self::from_persistent_cache(&key, ra)
        {
            return Ok(probe);
        }

        // 3. Create on disk
        Self::create_on_disk(
            key,
            type_name,
            method_name,
            effective_deps.as_ref(),
            ra_path,
        )
    }

    /// Checks the in-process Arc cache. Returns `Some(Probe)` on a hit.
    fn from_memory_cache(key: &CacheKey) -> Option<Self> {
        let inner = {
            let cache = cache::global_cache()
                .lock()
                .expect("probe cache lock poisoned");
            let cached = cache.get(key)?;
            if !cached.dir.exists() {
                return None;
            }
            let inner = Arc::clone(cached);
            drop(cache);
            inner
        };
        Some(Self {
            dir: inner.dir.clone(),
            src_path: inner.src_path.clone(),
            dot_line: inner.dot_line,
            dot_col: inner.dot_col,
            inner,
        })
    }

    /// Checks the persistent disk cache. On a hit, also populates the in-process
    /// cache so subsequent calls in the same process are instant.
    fn from_persistent_cache(key: &CacheKey, ra_path: &Path) -> Option<Self> {
        let version = ra_version(ra_path);
        let (dir, src_path, dot_line, dot_col) = load_persistent_probe(
            &key.type_name,
            key.effective_deps.as_deref(),
            key.method_name.as_deref(),
            &version,
        )?;

        if std::env::var("RUST_METH_DEBUG").is_ok() {
            eprintln!("[debug] persistent cache hit: {}", dir.display());
        }

        let inner = Arc::new(CachedProbe {
            dir: dir.clone(),
            src_path: src_path.clone(),
            dot_line,
            dot_col,
            owned: false,
        });

        {
            let mut cache = global_cache().lock().expect("probe cache lock poisoned");
            cache.insert(key.clone(), Arc::clone(&inner));
        }

        Some(Self {
            inner,
            dot_line,
            dot_col,
            dir,
            src_path,
        })
    }

    /// Creates a new probe directory on disk, writes Cargo.toml and src/main.rs,
    /// saves to the persistent cache if `ra_path` is provided, and inserts into
    /// the in-process cache.
    fn create_on_disk(
        key: CacheKey,
        type_name: &str,
        method_name: Option<&str>,
        effective_deps: Option<&String>,
        ra_path: Option<&Path>,
    ) -> std::io::Result<Self> {
        let id = PROBE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let suffix = method_name.map_or("probe", |_| "probe-def");
        let dir =
            std::env::temp_dir().join(format!("rust-meth-{suffix}-{}-{id}", std::process::id()));
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir)?;

        let cargo_toml = effective_deps.map_or_else(
        || "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n".to_string(),
        |d| format!(
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{d}\n"
        ),
    );
        fs::write(dir.join("Cargo.toml"), cargo_toml)?;

        let preamble_lines = u32::try_from(PREAMBLE.lines().count())
            .expect("PREAMBLE is a fixed compile-time constant");

        let source = method_name.map_or_else(
        || format!("{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n"),
        |method| format!(
            "{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.{method}();\n}}\n"
        ),
    );

        let src_path = src_dir.join("main.rs");
        fs::write(&src_path, &source)?;

        let dot_line = preamble_lines + 2;
        let dot_col =
            u32::try_from("    _x.".len()).expect("string literal length always fits in u32");

        if std::env::var("RUST_METH_DEBUG").is_ok() {
            eprintln!("=== probe source ===");
            for (i, line) in source.lines().enumerate() {
                eprintln!("{i:3}: {line}");
            }
            eprintln!("cursor → line={dot_line} col={dot_col}");
        }

        if let Some(ra) = ra_path {
            let version = ra_version(ra);
            save_persistent_probe(
                &dir,
                type_name,
                effective_deps.map(String::as_str),
                method_name,
                &version,
                dot_line,
                dot_col,
            );
        }

        let inner = Arc::new(CachedProbe {
            dir: dir.clone(),
            src_path: src_path.clone(),
            dot_line,
            dot_col,
            owned: true,
        });

        {
            let mut cache = global_cache().lock().expect("probe cache lock poisoned");
            cache.insert(key, Arc::clone(&inner));
        }

        Ok(Self {
            inner,
            dot_line,
            dot_col,
            dir,
            src_path,
        })
    }

    /// Converts the generated `src/main.rs` file path into a formatted `file://` URI string.
    ///
    /// Useful for protocols like LSP that require document paths formatted as URLs.
    #[must_use]
    pub fn src_uri(&self) -> String {
        path_to_uri(&self.src_path)
    }

    /// Converts the root workspace directory path into a formatted `file://` URI string.
    #[must_use]
    pub fn root_uri(&self) -> String {
        path_to_uri(&self.dir)
    }

    /// Reads and returns the contents of the source file as a string.
    ///
    /// # Errors
    ///
    /// This function will return an `Err` if the file cannot be read.
    /// Common reasons include:
    /// * The file at `src_path` does not exist.
    /// * The user lacks permissions to read the file.
    /// * The file contents are not valid UTF-8.
    pub fn source(&self) -> std::io::Result<String> {
        fs::read_to_string(&self.src_path)
    }
}

// `CachedProbe::drop` handles cleanup
// when the Arc refcount reaches zero. Nothing to do here.
impl Drop for Probe {
    fn drop(&mut self) {}
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // -- helpers -------------------------------------------------------

    fn preamble_line_count() -> u32 {
        u32::try_from(PREAMBLE.lines().count()).unwrap()
    }

    // -- Cargo.toml generation ------------------------------------------

    #[test]
    fn no_deps_omits_dependencies_section() {
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
        assert!(
            !cargo.contains("[dependencies]"),
            "Cargo.toml should not have a [dependencies] section when deps is None"
        );
        assert!(cargo.contains("[package]"));
        assert!(cargo.contains(r#"name = "probe""#));
        assert!(cargo.contains(r#"edition = "2024""#));
    }

    #[test]
    fn with_deps_injects_dependencies_section() {
        let p = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#)).unwrap();
        let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[dependencies]"));
        assert!(cargo.contains(r#"serde_json = "1.0""#));
    }

    #[test]
    fn multiple_deps_all_appear_in_cargo_toml() {
        let deps = "serde = { version = \"1.0\", features = [\"derive\"] }\nserde_json = \"1.0\"";
        let p = Probe::new_with_deps("serde_json::Value", Some(deps)).unwrap();
        let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[dependencies]"));
        assert!(cargo.contains("serde ="));
        assert!(cargo.contains(r#"serde_json = "1.0""#));
    }

    // ── source content ───────────────────────────────────────────────────────

    #[test]
    fn completion_probe_source_has_dot_trigger() {
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let src = p.source().unwrap();
        assert!(
            src.contains("let _x: Vec<u8> = todo!();"),
            "source should declare the type"
        );
        // ends with `_x.` NOT `_x.something()`
        assert!(
            src.contains("    _x.\n"),
            "completion probe should have bare dot trigger"
        );
    }

    #[test]
    fn definition_probe_source_has_method_call() {
        let p = Probe::for_definition_with_deps("Vec<u8>", "push", None).unwrap();
        let src = p.source().unwrap();
        assert!(src.contains("let _x: Vec<u8> = todo!();"));
        assert!(
            src.contains("_x.push();"),
            "definition probe should contain the method call"
        );
    }

    #[test]
    fn completion_probe_with_deps_type_in_source() {
        let p = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#)).unwrap();
        let src = p.source().unwrap();
        assert!(src.contains("let _x: serde_json::Value = todo!();"));
    }

    #[test]
    fn definition_probe_with_deps_cargo_and_source_correct() {
        let p = Probe::for_definition_with_deps(
            "serde_json::Value",
            "as_str",
            Some(r#"serde_json = "1.0""#),
        )
        .unwrap();
        let cargo = fs::read_to_string(p.dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[dependencies]"));
        let src = p.source().unwrap();
        assert!(src.contains("serde_json::Value"));
        assert!(src.contains("_x.as_str();"));
    }

    // ── dot position ─────────────────────────────────────────────────────────

    #[test]
    fn dot_col_is_seven() {
        // "    _x." is always 7 characters
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        assert_eq!(p.dot_col, 7, r#""    _x." should be 7 chars"#);
    }

    #[test]
    fn dot_line_is_preamble_plus_two() {
        // layout: preamble lines, fn main() {, let _x = …, _x.
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        assert_eq!(p.dot_line, preamble_line_count() + 2);
    }

    #[test]
    fn dot_line_same_for_definition_probe() {
        let p = Probe::for_definition_with_deps("Vec<u8>", "len", None).unwrap();
        assert_eq!(p.dot_line, preamble_line_count() + 2);
    }

    // ── URI helpers ──────────────────────────────────────────────────────────

    #[test]
    fn src_uri_is_file_uri_ending_in_main_rs() {
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let uri = p.src_uri();
        assert!(
            uri.starts_with("file://"),
            "src_uri should be a file:// URI"
        );
        assert!(
            uri.ends_with("/src/main.rs"),
            "src_uri should end in /src/main.rs"
        );
    }

    #[test]
    fn root_uri_is_file_uri_not_ending_in_main_rs() {
        let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let uri = p.root_uri();
        assert!(uri.starts_with("file://"));
        assert!(!uri.ends_with("main.rs"));
    }
}
