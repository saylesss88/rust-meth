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

// -- Workspace creation --

pub fn open_or_create(
    key: &SessionKey,
    deps: Option<&str>,
    initial_type: &str,
) -> Result<DaemonWorkspace, WorkspaceError> {
    let dir = workspaces_dir().join(key.dir_name());
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Write Cargo.toml only if it doesn't exist yet.
    let cargo_toml_path = dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        let cargo_toml = build_cargo_toml(deps);
        fs::write(&cargo_toml_path, cargo_toml)?;
    }

    // lib.rs is stable — preamble imports that keep the index warm.
    let lib_path = src_dir.join("lib.rs");
    if !lib_path.exists() {
        fs::write(&lib_path, lib_source(deps))?;
    }

    // scratch.rs is ephemeral, written fresh for each query type.
    let scratch_path = src_dir.join("scratch.rs");
    let source = scratch_source(initial_type);
    fs::write(&scratch_path, &source)?;

    // dot position: fn main() is line 0, let _x is line 1, _x. is line 2
    let dot_line = 2u32;
    let dot_col = u32::try_from("    _x.".len()).expect("literal length fits u32");

    Ok(DaemonWorkspace {
        dir,
        scratch_path,
        dot_line,
        dot_col,
    })
}

// -- Source generation --

/// Generates the stable `lib.rs` content, imports the declared crates to
/// force rust-analyzer to index them. This file never changes after creation.
fn lib_source(deps: Option<&str>) -> String {
    let mut lines = vec![
        "#![allow(unused_imports)]".to_string(),
        "use std::collections::*;".to_string(),
        "use std::sync::*;".to_string(),
        "use std::cell::*;".to_string(),
        "use std::rc::Rc;".to_string(),
        "use std::io::{self, Read, Write, BufRead};".to_string(),
        "use std::fmt;".to_string(),
        "use std::ops::*;".to_string(),
        "use std::path::{Path, PathBuf};".to_string(),
    ];

    // Add extern crate declarations for each dep so RA indexes them.
    if let Some(d) = deps {
        for line in d.lines() {
            let crate_name = line
                .split('=')
                .next()
                .map(str::trim)
                .unwrap_or("")
                .replace('-', "_");
            if !crate_name.is_empty() && !crate_name.starts_with('[') {
                lines.push(format!("extern crate {crate_name};"));
            }
        }
    }

    lines.join("\n") + "\n"
}

fn build_cargo_toml(deps: Option<&str>) -> String {
    let base = "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
    match deps {
        None => base.to_string(),
        Some(d) => format!("{base}\n[dependencies]\n{d}\n"),
    }
}

// -- Session key construction --

/// Builds a [`SessionKey`] for the given deps and ra binary.
///
/// Runs `cargo metadata` on a temporary workspace to resolve locked versions,
/// ensuring that `serde_json = "1.0"` and `serde_json = "1.0.219"` produce
/// the same key. For stdlib-only queries (no deps), skips metadata entirely.
///
/// # Errors
///
/// Returns an error if `cargo metadata` fails or produces unexpected output.
pub fn build_session_key(deps: Option<&str>, ra_path: &Path) -> Result<SessionKey, WorkspaceError> {
    let ra_version = ra_version_string(ra_path);
    let toolchain = active_toolchain();
    let locked_deps = resolve_locked_deps(deps)?;

    Ok(SessionKey {
        locked_deps,
        ra_version,
        toolchain,
    })
}

/// Resolves dependency versions via `cargo metadata`.
///
/// Returns an empty vec for stdlib-only queries.
fn resolve_locked_deps(deps: Option<&str>) -> Result<Vec<(String, String)>, WorkspaceError> {
    let Some(deps_str) = deps else {
        return Ok(Vec::new());
    };

    // Write a temporary Cargo.toml and run cargo metadata against it.
    let tmp = std::env::temp_dir().join(format!("rust-meth-resolve-{}", std::process::id()));
    let src = tmp.join("src");
    fs::create_dir_all(&src).map_err(WorkspaceError::Io)?;
    fs::write(
        tmp.join("Cargo.toml"),
        format!(
            "[package]\nname = \"resolve\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{deps_str}\n"
        ),
    )
    .map_err(WorkspaceError::Io)?;
    fs::write(src.join("lib.rs"), "").map_err(WorkspaceError::Io)?;

    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&tmp)
        .output()
        .map_err(|e| WorkspaceError::CargoMetadata(e.to_string()))?;

    let _ = fs::remove_dir_all(&tmp);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorkspaceError::CargoMetadata(stderr.to_string()));
    }

    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| WorkspaceError::MetadataParse)?;

    // Extract resolved package versions from metadata.
    let packages = meta["packages"]
        .as_array()
        .ok_or(WorkspaceError::MetadataParse)?;

    let mut locked: Vec<(String, String)> = packages
        .iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?;
            let version = p["version"].as_str()?;
            // Skip the resolve package itself.
            if name == "resolve" {
                return None;
            }
            Some((name.to_string(), version.to_string()))
        })
        .collect();

    // Sort for stable key ordering.
    locked.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(locked)
}

fn ra_version_string(ra_path: &Path) -> String {
    std::process::Command::new(ra_path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn active_toolchain() -> String {
    std::process::Command::new("rustup")
        .args(["show", "active-toolchain"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
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
