//! # `basic_query`
//!
//! The minimal end-to-end example: discover `rust-analyzer`, query all methods
//! on a stdlib type, and print their names.
//!
//! This is the "hello world" of `rust-meth-lib`
//!
//! ## Running the Example
//!
//! You can run this example using Cargo:
//!
//! ```bash
//! cargo run --example basic_query
//! ```
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.3.0"
//! ```
//!
//! ## Returns
//!
//! Returns the methods available on a standard library `Vec<u8>` type, printing
//! their names and the total count to standard output.

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};

fn main() -> rust_meth_lib::error::Result<()> {
    // Step 1: locate the rust-analyzer binary.
    //
    // find_rust_analyzer checks `PATH` first, then falls back to the rustup
    // component directory (~/.rustup/toolchains/<active>/bin/rust-analyzer).
    // Returns `RustMethError::RustAnalyzerNotFound` if neither exists.
    let ra_path = find_rust_analyzer()?;

    // Step 2: query methods.
    //
    // query_methods spins up an ephemeral LSP session, synthesises a
    // temporary Cargo project with `let _x: Vec<u8> = todo!();`, triggers
    // a completion request at the dot position, and tears everything down.
    //
    // The third argument is `deps: Option<&str>`: `None` means stdlib only.
    let methods = query_methods("Vec<u8>", &ra_path, None)?;

    println!("Methods on Vec<u8> ({} total):", methods.len());
    for method in &methods {
        println!("  {}", method.name);
    }

    Ok(())
}
