//! `rust-meth`: A CLI utility to discover and filter methods available on Rust types.
//! It leverages `rust-analyzer` via the Language Server Protocol (LSP) to provide
//! accurate, context-aware method resolution.
pub mod app;
pub mod ui;

use std::process;

fn main() {
    if let Err(err) = app::run(env!("CARGO_PKG_VERSION")) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
