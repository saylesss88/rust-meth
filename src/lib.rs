//! Rust method explorer — queries LSP completions and definitions
//! for any Rust type, including third-party crates.

#![deny(missing_docs)]

/// Orchestrates the full LSP session
pub mod analyzer;
pub(crate) mod lsp;
pub(crate) mod probe;

pub use lsp::LspTransport;
pub use probe::Probe;
