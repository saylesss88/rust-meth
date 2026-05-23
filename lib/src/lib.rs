// #![doc = include_str!("../README.md")]
//! Rust method explorer, queries LSP completions and definitions
//! for any Rust type, including third-party crates.

#![deny(missing_docs)]

/// Orchestrates the full LSP session
pub mod analyzer;
/// Custom errors with `thiserror`
pub mod error;
/// Minimal LSP transport
pub mod lsp;
pub mod probe;

pub use error::RustMethError;
pub use lsp::LspTransport;
pub use probe::Probe;
