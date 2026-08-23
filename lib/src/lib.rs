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
/// Builder API and filter_methods standalone function
pub mod query;

pub use error::RustMethError;
pub use lsp::LspTransport;
pub use probe::{CacheEntry, Probe, cache_entries, clear_probe_cache};
pub use query::{MethodQuery, MethodResult, filter_methods, query_definition_for_methods};
