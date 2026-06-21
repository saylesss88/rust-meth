//! Locating the `rust-analyzer` binary on the host system.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::error::{Result, RustMethError};

static RA_PATH_CACHE: OnceLock<PathBuf> = OnceLock::new();

fn rustup_rust_analyzer() -> Option<PathBuf> {
    let out = Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then(|| path.into())
}

#[cfg(unix)]
fn which(name: &str) -> Result<std::path::PathBuf> {
    let out = Command::new("which").arg(name).output()?;
    if !out.status.success() {
        return Err(RustMethError::RustAnalyzerNotFound);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(s.into())
}

/// Locates the `rust-analyzer` binary.
///
/// It first searches the system `PATH` env variable using the system `which` utility.
/// If missing, it attempts to fall back to the active toolchain's binary directory
/// using `rustup which rust-analyzer`.
///
/// # Errors
///
/// Returns an error if `rust-analyzer` cannot be found via either mechanism,
/// providing user-friendly instructions on how to install it.
pub fn find_rust_analyzer() -> Result<PathBuf> {
    if let Some(path) = RA_PATH_CACHE.get() {
        return Ok(path.clone());
    }
    let path = which("rust-analyzer")
        .ok()
        .or_else(rustup_rust_analyzer)
        .ok_or(RustMethError::RustAnalyzerNotFound)?;
    Ok(RA_PATH_CACHE.get_or_init(|| path).clone())
}
