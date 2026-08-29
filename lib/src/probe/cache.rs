//!
//! ## Probe caching
//!
//! Probes are cached in-process by `(type_name, effective_deps, probe_kind)`. A cache hit skips
//! temp-dir creation and file writes entirely. The cache holds an [`Arc`] to each `CachedProbe`;
//! when all references are dropped the directory is deleted automatically. Call
//! [`cache_entries`] to inspect what is currently cached, and [`clear_probe_cache`] to evict
//! everything immediately.
//!
//! When the [`Probe`] instance goes out of scope, its [`Drop`] implementation automatically deletes
//! the entire temporary directory and its contents from the disk, unless the entry is still held
//! by the cache.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

// -- Hashing --
//
// Simple FNV-1a hash

pub(super) fn fnv1a(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Computes the persistent cache key for a probe configuration.
///
/// Returns a 16-character hex string derived from `type_name`, `effective_deps`,
/// `method_name`, the `rust-analyzer` version, and the `rust-meth-lib` version.
/// Any change to these inputs produces a different key, so upgrades to either
/// binary automatically invalidate stale cache entries.
#[must_use]
pub fn cache_key_hash(
    type_name: &str,
    effective_deps: Option<&str>,
    method_name: Option<&str>,
    ra_version: &str,
) -> String {
    let canonical = format!(
        "type={}\ndeps={}\nmethod={}\nra={}\nlib={}",
        type_name,
        effective_deps.unwrap_or(""),
        method_name.unwrap_or(""),
        ra_version,
        env!("CARGO_PKG_VERSION"),
    );
    format!("{:016x}", fnv1a(&canonical))
}

// ── Persistent cache ──────────────────────────────────────────────────────────

/// Metadata stored alongside each persistent probe directory.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct ProbeMeta {
    pub(super) type_name: String,
    pub(super) effective_deps: Option<String>,
    pub(super) method_name: Option<String>,
    pub(super) ra_version: String,
    pub(super) lib_version: String,
    pub(super) dot_line: u32,
    pub(super) dot_col: u32,
}

/// Returns the root directory of the persistent probe cache.
///
/// Respects `$XDG_CACHE_HOME` if set, otherwise falls back to `~/.cache`.
/// The full path is `$XDG_CACHE_HOME/rust-meth/probes/`.
#[must_use]
pub fn persistent_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map_or_else(
        || {
            std::env::var_os("HOME")
                .map_or_else(std::env::temp_dir, PathBuf::from)
                .join(".cache")
        },
        PathBuf::from,
    );
    base.join("rust-meth").join("probes")
}

/// Public view of a single persistent cache entry.
#[derive(Debug, Clone)]
pub struct PersistentCacheEntry {
    /// The Rust type that was probed.
    pub type_name: String,
    /// The effective TOML dependency string, if any.
    pub deps: Option<String>,
    /// `None` for completion probes; `Some(method_name)` for definition probes.
    pub method_name: Option<String>,
    /// The `rust-analyzer` version this entry was created with.
    pub ra_version: String,
    /// The `rust-meth-lib` version this entry was created with.
    pub lib_version: String,
    /// Absolute path to the cached probe directory on disk.
    pub dir: PathBuf,
}

/// Returns all entries currently in the persistent probe cache.
///
/// Reads `meta.json` from each subdirectory of [`persistent_cache_dir`].
/// Entries whose `meta.json` is missing or malformed are silently skipped.
#[must_use]
pub fn persistent_cache_entries() -> Vec<PersistentCacheEntry> {
    let dir = persistent_cache_dir();
    let Ok(read_dir) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let probe_dir = entry.path();
            let meta_path = probe_dir.join("meta.json");
            let meta_bytes = fs::read(&meta_path).ok()?;
            let meta: ProbeMeta = serde_json::from_slice(&meta_bytes).ok()?;
            Some(PersistentCacheEntry {
                type_name: meta.type_name,
                deps: meta.effective_deps,
                method_name: meta.method_name,
                ra_version: meta.ra_version,
                lib_version: meta.lib_version,
                dir: probe_dir,
            })
        })
        .collect()
}

/// Removes all entries from the persistent probe cache.
///
/// Deletes the entire [`persistent_cache_dir`] and its contents.
///
/// # Errors
///
/// Returns an error if the directory cannot be removed.
pub fn clear_persistent_cache() -> std::io::Result<()> {
    let dir = persistent_cache_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub(super) fn load_persistent_probe(
    type_name: &str,
    effective_deps: Option<&str>,
    method_name: Option<&str>,
    ra_version: &str,
) -> Option<(PathBuf, PathBuf, u32, u32)> {
    let hash = cache_key_hash(type_name, effective_deps, method_name, ra_version);
    let probe_dir = persistent_cache_dir().join(&hash);
    if !probe_dir.exists() {
        return None;
    }
    let meta_bytes = fs::read(probe_dir.join("meta.json")).ok()?;
    let meta: ProbeMeta = serde_json::from_slice(&meta_bytes).ok()?;
    let src_path = probe_dir.join("src").join("main.rs");
    if !src_path.exists() || !probe_dir.join("Cargo.toml").exists() {
        return None;
    }
    Some((probe_dir, src_path, meta.dot_line, meta.dot_col))
}

pub(super) fn save_persistent_probe(
    probe_dir: &Path,
    type_name: &str,
    effective_deps: Option<&str>,
    method_name: Option<&str>,
    ra_version: &str,
    dot_line: u32,
    dot_col: u32,
) {
    let hash = cache_key_hash(type_name, effective_deps, method_name, ra_version);
    let cache_dir = persistent_cache_dir().join(&hash);
    let cache_src_dir = cache_dir.join("src");

    let save = || -> std::io::Result<()> {
        fs::create_dir_all(&cache_src_dir)?;
        fs::copy(probe_dir.join("Cargo.toml"), cache_dir.join("Cargo.toml"))?;
        fs::copy(
            probe_dir.join("src").join("main.rs"),
            cache_src_dir.join("main.rs"),
        )?;
        let meta = ProbeMeta {
            type_name: type_name.to_string(),
            effective_deps: effective_deps.map(str::to_owned),
            method_name: method_name.map(str::to_owned),
            ra_version: ra_version.to_string(),
            lib_version: env!("CARGO_PKG_VERSION").to_string(),
            dot_line,
            dot_col,
        };
        fs::write(
            cache_dir.join("meta.json"),
            serde_json::to_vec_pretty(&meta)?,
        )?;
        Ok(())
    };

    if let Err(e) = save()
        && std::env::var("RUST_METH_DEBUG").is_ok()
    {
        eprintln!("[debug] failed to save persistent probe: {e}");
    }
}

/// Queries the `rust-analyzer` binary for its version string.
#[must_use]
pub fn ra_version(ra_path: &Path) -> String {
    std::process::Command::new(ra_path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

// -- In-process Cache types --

// Identifies a unique probe configuration for cache lookup.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(super) struct CacheKey {
    pub(super) type_name: String,
    /// Effective deps after `infer_dep`, so `serde_json::Value` with `deps=None`
    /// and with `deps=Some(r#"serde_json = "*""#)` share the same cache entry.
    pub(super) effective_deps: Option<String>,
    /// `None` for completion probes, `Some(method_name)` for definition probes.
    pub(super) method_name: Option<String>,
}

/// The heap-allocated probe data kept alive by the cache [`Arc`].
///
/// Deletes its directory when the last [`Arc`] is dropped (i.e. when both
/// the cache entry and the [`Probe`] borrowing it are gone).
#[derive(Debug)]
pub(super) struct CachedProbe {
    pub(super) dir: PathBuf,
    pub(super) src_path: PathBuf,
    pub(super) dot_line: u32,
    pub(super) dot_col: u32,
    pub(super) owned: bool,
}

impl Drop for CachedProbe {
    fn drop(&mut self) {
        if self.owned {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Public view of a single cache entry, returned by [`cache_entries`].
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The Rust type that was probed (e.g. `"Vec<u8>"`).
    pub type_name: String,
    /// The effective TOML dependency string, if any.
    pub deps: Option<String>,
    /// `None` for completion probes; `Some(method_name)` for definition probes.
    pub method_name: Option<String>,
    /// Absolute path to the cached probe directory on disk.
    pub dir: PathBuf,
}

// -- Global cache --

type Cache = Mutex<HashMap<CacheKey, Arc<CachedProbe>>>;

pub(super) fn global_cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns a snapshot of all probe directories currently held in the cache.
///
/// ## Panics
///
/// A poisoned mutex means the thread panicked while holding the lock, leaving
/// the cache in an unknown state.
#[must_use]
pub fn cache_entries() -> Vec<CacheEntry> {
    let cache = global_cache().lock().expect("probe cache lock poisoned");
    cache
        .iter()
        .map(|(key, probe)| CacheEntry {
            type_name: key.type_name.clone(),
            deps: key.effective_deps.clone(),
            method_name: key.method_name.clone(),
            dir: probe.dir.clone(),
        })
        .collect()
}

/// Evicts all entries from the probe cache.
///
/// Each entry's directory is deleted when its [`Arc`] reference count reaches
/// zero immediately if no `Probe` is currently borrowing the entry, or
/// when the last borrowing `Probe` is dropped otherwise.
///
/// ## Panics
///
/// A poisoned mutex means the thread panicked while holding the lock, leaving
/// the cache in an unknown state.
pub fn clear_probe_cache() {
    let mut cache = global_cache().lock().expect("probe cache lock poisoned");
    cache.clear();
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::probe::Probe;
    use serial_test::serial;

    #[test]
    #[serial]
    fn cache_hit_returns_same_directory() {
        clear_probe_cache();
        let p1 = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let p2 = Probe::new_with_deps("Vec<u8>", None).unwrap();
        assert_eq!(p1.dir, p2.dir, "cache hit should reuse the same directory");
    }

    #[test]
    #[serial]
    fn cache_miss_different_type_names() {
        clear_probe_cache();
        let p1 = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let p2 = Probe::new_with_deps("String", None).unwrap();
        assert_ne!(p1.dir, p2.dir);
    }

    #[test]
    #[serial]
    fn cache_miss_different_deps() {
        clear_probe_cache();
        let p1 = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "1.0""#)).unwrap();
        let p2 = Probe::new_with_deps("serde_json::Value", Some(r#"serde_json = "2.0""#)).unwrap();
        assert_ne!(p1.dir, p2.dir);
    }

    #[test]
    #[serial]
    fn cache_entries_reflects_current_cache() {
        clear_probe_cache();
        let _p1 = Probe::new_with_deps("Vec<u8>", None).unwrap();
        let _p2 = Probe::new_with_deps("String", None).unwrap();
        let entries = cache_entries();
        let names: Vec<&str> = entries.iter().map(|e| e.type_name.as_str()).collect();
        assert!(names.contains(&"Vec<u8>"));
        assert!(names.contains(&"String"));
    }

    #[test]
    #[serial]
    fn clear_probe_cache_evicts_all_entries() {
        let _p = Probe::new_with_deps("Vec<u8>", None).unwrap();
        clear_probe_cache();
        assert!(cache_entries().is_empty());
    }

    #[test]
    #[serial]
    fn directory_persists_while_cache_holds_arc() {
        clear_probe_cache();
        let dir = {
            let p = Probe::new_with_deps("Vec<u8>", None).unwrap();
            p.dir.clone()
        };
        {
            let _guard = global_cache().lock().expect("probe cache lock poisoned");
            assert!(dir.exists(), "dir should persist while cache holds the Arc");
        }
        clear_probe_cache();
        assert!(
            !dir.exists(),
            "dir should be deleted after cache is cleared"
        );
    }

    // -- persistent cache --

    #[test]
    fn persistent_cache_dir_ends_with_rust_meth_probes() {
        let dir = persistent_cache_dir();
        assert!(dir.ends_with("rust-meth/probes"));
    }

    #[test]
    fn cache_key_hash_differs_by_type() {
        let h1 = cache_key_hash("Vec<u8>", None, None, "rust-analyzer 1.0");
        let h2 = cache_key_hash("String", None, None, "rust-analyzer 1.0");
        assert_ne!(h1, h2);
    }

    #[test]
    fn cache_key_hash_differs_by_ra_version() {
        let h1 = cache_key_hash("Vec<u8>", None, None, "rust-analyzer 1.0");
        let h2 = cache_key_hash("Vec<u8>", None, None, "rust-analyzer 2.0");
        assert_ne!(h1, h2);
    }

    #[test]
    fn cache_key_hash_differs_by_deps() {
        let h1 = cache_key_hash(
            "serde_json::Value",
            Some(r#"serde_json = "1.0""#),
            None,
            "ra",
        );
        let h2 = cache_key_hash(
            "serde_json::Value",
            Some(r#"serde_json = "2.0""#),
            None,
            "ra",
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn meta_json_round_trips() {
        let meta = ProbeMeta {
            type_name: "Vec<u8>".to_string(),
            effective_deps: None,
            method_name: None,
            ra_version: "rust-analyzer 1.80.0".to_string(),
            lib_version: "0.4.0".to_string(),
            dot_line: 11,
            dot_col: 7,
        };
        let json = serde_json::to_vec_pretty(&meta).unwrap();
        let back: ProbeMeta = serde_json::from_slice(&json).unwrap();
        assert_eq!(back.type_name, "Vec<u8>");
        assert_eq!(back.dot_line, 11);
        assert_eq!(back.dot_col, 7);
        assert_eq!(back.ra_version, "rust-analyzer 1.80.0");
        assert!(back.effective_deps.is_none());
    }
}
