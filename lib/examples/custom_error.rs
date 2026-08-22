//! # `custom_error`
//!
//! Integrating `RustMethError` into your own error type.
//!
//! The README mentions you can convert `RustMethError` into your own error
//! type via `?` since it implements `std::error::Error`. This example shows
//! what that looks like in practice with `thiserror`.
//!
//! The key insight: you don't need to match on every `RustMethError` variant
//! in your application. Just decide which ones you want to handle explicitly
//! (e.g. `TypeNotFound` for a "did you mean?" suggestion) and catch the rest
//! as a generic source.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.2.4"
//! thiserror = "2"
//! ```
//!
//! ## Returns
//!
//! ```bash
//! Vec<u8>: 229 methods
//! align_to
//! align_to_mut
//! allocator
//! append
//! array_windows
//! ...and 224 more
//!
//! Type not found: `NonExistentType`
//! Tip: check spelling, or pass deps for third-party crates.
//! ```

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};
use rust_meth_lib::error::RustMethError;

// ── Your application's error type ───────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum AppError {
    /// rust-analyzer isn't installed, give a helpful message.
    #[error("rust-analyzer not found. Install it with: rustup component add rust-analyzer")]
    RustAnalyzerMissing,

    /// The type the user asked about doesn't exist.
    #[error(
        "Unknown type `{type_name}`. Check the spelling and that any required crate is listed in deps."
    )]
    UnknownType { type_name: String },

    /// Anything else from the library becomes a generic query failure.
    #[error("LSP query failed: {0}")]
    QueryFailed(RustMethError),
}

// ── Conversion ───────────────────────────────────────────────────────────────
//
// We can't use `#[from]` for everything because we want to intercept specific
// variants. Instead, write a manual `From` impl that matches the cases we care
// about and falls through to the generic wrapper for everything else.

impl From<RustMethError> for AppError {
    fn from(err: RustMethError) -> Self {
        match err {
            RustMethError::RustAnalyzerNotFound => Self::RustAnalyzerMissing,
            RustMethError::TypeNotFound { type_name, .. } => Self::UnknownType { type_name },
            other => Self::QueryFailed(other),
        }
    }
}

// ── Application logic ────────────────────────────────────────────────────────

fn lookup(type_name: &str) -> Result<(), AppError> {
    // find_rust_analyzer() returns RustMethError::RustAnalyzerNotFound on
    // failure, which our From impl converts to AppError::RustAnalyzerMissing.
    let ra_path = find_rust_analyzer()?;

    // query_methods() returns RustMethError::TypeNotFound for bad type names,
    // which our From impl converts to AppError::UnknownType.
    let methods = query_methods(type_name, &ra_path, None)?;

    println!("{type_name}: {} methods", methods.len());
    for m in methods.iter().take(5) {
        println!("  {}", m.name);
    }
    if methods.len() > 5 {
        println!("  ...and {} more", methods.len() - 5);
    }
    Ok(())
}

fn main() {
    // Use a type that will succeed.
    match lookup("Vec<u8>") {
        Ok(()) => {}
        Err(e) => eprintln!("error: {e}"),
    }

    println!();

    // Use a type that will fail with `TypeNotFound`, demonstrates the
    // custom error message path.
    match lookup("NonExistentType") {
        Ok(()) => {}
        Err(AppError::UnknownType { type_name }) => {
            eprintln!("Type not found: `{type_name}`");
            eprintln!("Tip: check spelling, or pass deps for third-party crates.");
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
