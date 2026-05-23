// #![doc = include_str!("../README.md")]
//! Rust method explorer, queries LSP completions and definitions
//! for any Rust type, including third-party crates.

#![deny(missing_docs)]

/// Orchestrates the full LSP session
pub mod analyzer;
/// Primary entry point for application logic
/// Custom errors with `thiserror`
pub mod error;
pub(crate) mod lsp;
pub(crate) mod probe;

pub use error::RustMethError;
pub use lsp::LspTransport;
pub use probe::Probe;
