use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Errors from workspace creation or key normalization.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cargo metadata failed: {0}")]
    CargoMetadata(String),
    #[error("failed to parse cargo metadata output")]
    MetadataParse,
}

// -- Session key --

/// Uniquely identifies a rust-analyzer session by its immutable workspace
/// configuration. Two queries with the same key share one session.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SessionKey {
    /// Resolved and locked dependency versions, e.g. `[("serde_json", "1.0.219")]`.
    /// Empty for stdlib-only queries.
    pub locked_deps: Vec<(String, String)>,
    /// rust-analyzer version string.
    pub ra_version: String,
    /// Active rustup toolchain, e.g. `"nightly-x86_64-unknown-linux-gnu"`.
    pub toolchain: String,
}

impl SessionKey {
    /// Derives a filesystem-safe directory name from the key.
    #[must_use]
    pub fn dir_name(&self) -> String {
        if self.locked_deps.is_empty() {
            format!("stdlib-{}", &self.ra_version.replace(' ', "_"))
        } else {
            let deps = self
                .locked_deps
                .iter()
                .map(|(k, v)| format!("{k}@{v}"))
                .collect::<Vec<_>>()
                .join("+");
            format!("{deps}-{}", &self.ra_version.replace(' ', "_"))
        }
    }
}

// -- Workspace --

/// A stable on-disk Cargo workspace for a daemon LSP session.
pub struct DaemonWorkspace {
    /// Root directory of the workspace.
    pub dir: PathBuf,
    /// Path to `src/scratch.rs`, rewritten per query.
    pub scratch_path: PathBuf,
    /// 0-indexed line of the dot trigger in `scratch.rs`.
    pub dot_line: u32,
    /// 0-indexed column of the dot trigger in `scratch.rs`.
    pub dot_col: u32,
}

impl DaemonWorkspace {
    /// Returns the workspace root as a `file://` URI.
    #[must_use]
    pub fn root_uri(&self) -> String {
        path_to_uri(&self.dir)
    }

    /// Returns the `scratch.rs` path as a `file://` URI.
    #[must_use]
    pub fn scratch_uri(&self) -> String {
        path_to_uri(&self.scratch_path)
    }

    /// Rewrites `scratch.rs` with the given type injected at the dot trigger.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn set_type(&self, type_name: &str) -> std::io::Result<()> {
        let source = scratch_source(type_name);
        fs::write(&self.scratch_path, source)
    }
}

// -- Workspace root --

/// Returns the root directory for all daemon workspaces.
#[must_use]
pub fn workspaces_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".cache")
        });
    base.join("rust-meth").join("workspaces")
}

/// Generates the per-query `scratch.rs` content with `type_name` injected.
fn scratch_source(type_name: &str) -> String {
    format!("fn main() {{\n    let _x: {type_name} = todo!();\n    _x.\n}}\n")
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}
