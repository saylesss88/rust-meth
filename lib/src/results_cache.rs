//! Persistent cache for `query_methods` results.
//!
//! Stores `Vec<Method>` as JSON on disk, keyed by
//! `(type_name, effective_deps, ra_version, lib_version)`. A cache hit
//! returns results immediately with zero LSP cost — no rust-analyzer
//! subprocess, no Cargo resolution, no probe directory needed.
//!
//! Cache entries are stored in `$XDG_CACHE_HOME/rust-meth/results/`
//! (falling back to `~/.cache/rust-meth/results/`). The key includes
//! the `rust-analyzer` version and `rust-meth-lib` version so upgrades
//! automatically invalidate stale entries.

use std::fs;
use std::path::PathBuf;

use crate::analyzer::Method;
use crate::probe::cache_key_hash;

// Directory

/// Returns the root directory of the persistent results cache.
///
/// Respects `$XDG_CACHE_HOME` if set, otherwise falls back to `~/.cache`.
/// The full path is `$XDG_CACHE_HOME/rust-meth/results/`.
#[must_use]
pub fn results_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").map_or_else(
        || {
            std::env::var_os("HOME")
                .map_or_else(std::env::temp_dir, PathBuf::from)
                .join(".cache")
        },
        PathBuf::from,
    );
    base.join("rust-meth").join("results")
}

// Entry

/// Metadata and results for a single results cache entry.
#[derive(Debug, Clone)]
pub struct ResultsCacheEntry {
    /// The Rust type that was queried.
    pub type_name: String,
    /// The effective TOML dependency string, if any.
    pub deps: Option<String>,
    /// The `rust-analyzer` version this entry was created with.
    pub ra_version: String,
    /// The `rust-meth-lib` version this entry was created with.
    pub lib_version: String,
    /// Number of methods stored in this entry.
    pub method_count: usize,
    /// Absolute path to the cache file on disk.
    pub path: PathBuf,
}

/// Serialized form stored on disk.
#[derive(serde::Serialize, serde::Deserialize)]
struct ResultsFile {
    type_name: String,
    effective_deps: Option<String>,
    ra_version: String,
    lib_version: String,
    methods: Vec<Method>,
}

// Public API

/// Returns all entries currently in the persistent results cache.
///
/// Reads each `.json` file in [`results_cache_dir`]. Entries that are
/// missing or malformed are silently skipped.
#[must_use]
pub fn results_cache_entries() -> Vec<ResultsCacheEntry> {
    let dir = results_cache_dir();
    let Ok(read_dir) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()?.to_str()? != "json" {
                return None;
            }
            let bytes = fs::read(&path).ok()?;
            let file: ResultsFile = serde_json::from_slice(&bytes).ok()?;
            Some(ResultsCacheEntry {
                type_name: file.type_name,
                deps: file.effective_deps,
                ra_version: file.ra_version,
                lib_version: file.lib_version,
                method_count: file.methods.len(),
                path,
            })
        })
        .collect()
}

/// Removes all entries from the persistent results cache.
///
/// Deletes the entire [`results_cache_dir`] and its contents.
///
/// # Errors
///
/// Returns an error if the directory cannot be removed.
pub fn clear_results_cache() -> std::io::Result<()> {
    let dir = results_cache_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Attempts to load cached methods for the given query parameters.
///
/// Returns `None` if no entry exists or if the file is malformed.
pub(crate) fn load_results(
    type_name: &str,
    effective_deps: Option<&str>,
    ra_version: &str,
) -> Option<Vec<Method>> {
    let path = entry_path(type_name, effective_deps, ra_version);
    let bytes = fs::read(&path).ok()?;
    let file: ResultsFile = serde_json::from_slice(&bytes).ok()?;

    if std::env::var("RUST_METH_DEBUG").is_ok() {
        eprintln!("[debug] results cache hit: {}", path.display());
    }

    Some(file.methods)
}

/// Saves method results to the persistent cache.
///
/// Failures are silent, a failed write doesn't affect the query result.
pub(crate) fn save_results(
    type_name: &str,
    effective_deps: Option<&str>,
    ra_version: &str,
    methods: &[Method],
) {
    let save = || -> std::io::Result<()> {
        let dir = results_cache_dir();
        fs::create_dir_all(&dir)?;
        let file = ResultsFile {
            type_name: type_name.to_string(),
            effective_deps: effective_deps.map(str::to_owned),
            ra_version: ra_version.to_string(),
            lib_version: env!("CARGO_PKG_VERSION").to_string(),
            methods: methods.to_vec(),
        };
        let path = entry_path(type_name, effective_deps, ra_version);
        fs::write(path, serde_json::to_vec_pretty(&file)?)?;
        Ok(())
    };

    if let Err(e) = save()
        && std::env::var("RUST_METH_DEBUG").is_ok()
    {
        eprintln!("[debug] failed to save results cache: {e}");
    }
}

fn entry_path(type_name: &str, effective_deps: Option<&str>, ra_version: &str) -> PathBuf {
    let hash = cache_key_hash(type_name, effective_deps, None, ra_version);
    results_cache_dir().join(format!("{hash}.json"))
}
