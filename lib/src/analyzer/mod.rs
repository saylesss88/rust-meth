//! Orchestrates the full LSP session:
//!   1. Spawn rust-analyzer
//!   2. initialize / initialized handshake
//!   3. textDocument/didOpen
//!   4. Wait for indexing to complete
//!   5. textDocument/completion (with retry)
//!   6. Extract Method items from the response
//!   7. shutdown / exit
//!
//! Split into:
//! * `discovery` : locating the `rust-analyzer` binary
//! * `session` : driving an LSP session end to end
//! * `parse` : turning raw LSP JSON into `Method` / `Definition`

mod discovery;
mod parse;
mod session;

pub use discovery::find_rust_analyzer;
pub use parse::{Definition, Method, parse_definition, parse_methods};
pub use session::{query_definition, query_methods};
