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
// Cache results
pub mod results_cache;

pub use error::RustMethError;
pub use lsp::LspTransport;
pub use probe::{
    CacheEntry, PersistentCacheEntry, Probe, cache_entries, clear_persistent_cache,
    clear_probe_cache, persistent_cache_dir, persistent_cache_entries,
};
pub use query::{MethodQuery, MethodResult, filter_methods, query_definition_for_methods};
pub use results_cache::{
    ResultsCacheEntry, clear_results_cache, results_cache_dir, results_cache_entries,
};
