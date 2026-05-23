//! A utility for generating ephemeral, minimal Cargo projects ("probes") used to query
//! Language Server Protocol (LSP) intelligence like autocompletions or go-to-definitions.
//!
//! A `Probe` creates a temporary directory containing a valid Cargo package with a single source file.
//! The source file declares an isolated variable statement `let _x: TYPE = todo!();` followed by a target
//! interaction point (such as `_x.` or `_x.method()`).
//!
//! When the [`Probe`] instance goes out of scope, its [`Drop`] implementation automatically deletes
//! the entire temporary directory and its contents from the disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
/// Deletes itself automatically when dropped.
pub struct Probe {
    /// The absolute path to the root directory of the temporary Cargo project.
    pub dir: PathBuf,
    /// The absolute path to the generated `src/main.rs` file.
    pub src_path: PathBuf,
    /// The 0-indexed line number in `src/main.rs` pointing to the target interaction point (the dot trigger).
    pub dot_line: u32,
    /// The 0-indexed character/column offset pointing exactly after the dot (`_x.`) in `src/main.rs`.
    pub dot_col: u32,
}

impl Probe {
    /// Creates a new probe project without dependencies (for stdlib types).
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if creating the underlying probe project directory
    /// or writing its files fails.
    pub fn new(type_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, None, None)
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
        Self::create_probe(type_name, None, deps)
    }

    /// Creates a probe file with `_x.METHOD_NAME()` for go-to-definition queries.
    /// The cursor position points at the start of the method name.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the workspace initialization or file creation fails
    /// on disk.
    pub fn for_definition(type_name: &str, method_name: &str) -> std::io::Result<Self> {
        Self::create_probe(type_name, Some(method_name), None)
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
            return None; // primitives, String, Vec<u8>, etc. — never need an inferred dep
        }
        let crate_name = type_name.split("::").next()?;
        if crate_name.chars().next()?.is_ascii_lowercase() {
            Some(format!(r#"{crate_name} = "*""#))
        } else {
            None
        }
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
        Self::create_probe(type_name, Some(method_name), deps)
    }

    /// Internal probe creation logic shared by all constructors.
    ///
    /// # Arguments
    /// * `type_name` - The Rust type to query
    /// * `method_name` - If Some, creates a definition probe; if None, creates a completion probe
    /// * `deps` - Optional TOML dependencies to add to Cargo.toml
    fn create_probe(
        type_name: &str,
        method_name: Option<&str>,
        deps: Option<&str>,
    ) -> std::io::Result<Self> {
        let id = PROBE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let suffix = method_name.map_or("probe", |_| "probe-def");
        let dir =
            std::env::temp_dir().join(format!("rust-meth-{suffix}-{}-{id}", std::process::id()));

        // let dir = std::env::temp_dir().join(format!("rust-meth-{suffix}-{}", std::process::id()));
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir)?;

        let effective_deps = deps
            .map(str::to_owned)
            .or_else(|| Self::infer_dep(type_name));

        let cargo_toml = effective_deps.map_or_else(
        || "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n".to_string(),
        |d| format!(
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{d}\n"
        ),
    );

        // Build Cargo.toml with optional dependencies

        fs::write(dir.join("Cargo.toml"), cargo_toml)?;

        // Source file layout (preamble lines + fn main):
        //
        //   0..N  preamble use statements
        //   N+0:  fn main() {
        //   N+1:      let _x: TYPE = todo!();
        //   N+2:      _x.         <-- completion trigger after the dot (or _x.METHOD() for definition)
        //   N+3:  }
        let preamble_lines =
            u32::try_from(PREAMBLE.lines().count()).expect("Preamble is too long to fit in u32");

        // Generate source based on whether we're doing completion or definition
        let source = method_name.map_or_else(|| format!("{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n"), |method| format!(
                      "{PREAMBLE}fn main() {{\n    let _x: {type_name} = todo!();\n    _x.{method}();\n}}\n"
                 ));

        let src_path = src_dir.join("main.rs");
        fs::write(&src_path, &source)?;

        // Dot is at preamble_lines + 2, col = len("    _x.")
        let dot_line = preamble_lines + 2;
        let dot_col = u32::try_from("    _x.".len()).expect("failed");

        if std::env::var("RUST_METH_DEBUG").is_ok() {
            eprintln!("=== probe source ===");
            for (i, line) in source.lines().enumerate() {
                eprintln!("{i:3}: {line}");
            }
            eprintln!("cursor → line={dot_line} col={dot_col}");
        }
        Ok(Self {
            dir,
            src_path,
            dot_line,
            dot_col,
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

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
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

    // ── cleanup ──────────────────────────────────────────────────────────────

    #[test]
    fn drop_removes_temp_directory() {
        let dir = {
            let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
            assert!(p.dir.exists(), "dir should exist while probe is alive");
            p.dir.clone()
        };
        assert!(!dir.exists(), "temp dir should be removed after drop");
    }

    #[test]
    fn definition_probe_drop_removes_temp_directory() {
        let dir = {
            let p = Probe::for_definition_with_deps("Vec<u8>", "len", None).unwrap();
            p.dir.clone()
        };
        assert!(!dir.exists());
    }
}
