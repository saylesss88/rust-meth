//! # `builder_definitions`
//!
//! Demonstrates [`MethodQuery::run_with_definitions`], queries methods and
//! resolves their source locations in one chain.
//!
//! Each definition query spins up its own ephemeral `rust-analyzer` session.
//! For third-party crate types, Cargo must also resolve dependencies inside
//! each probe project, which adds significant time per method. Use a tight
//! [`filter`](rust_meth_lib::query::MethodQuery::filter) to limit the number
//! of definition queries. Prefer exact or short prefix matches over broad
//! substring patterns.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.4.0"
//! ```
//!
//! ## Returns
//!
//! - **Stdlib**: `Vec<u8>` methods matching `"drain"` with source locations
//! - **Third-party**: `serde_json::Value::as_str` source location
//!
//! > **Note:** The third-party query requires Cargo to resolve `serde_json`
//! > inside the probe project. Expect 5–10 seconds on a warm cache.

use rust_meth_lib::analyzer::find_rust_analyzer;
use rust_meth_lib::query::MethodQuery;

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // -- Stdlib type -- fast --

    let results = MethodQuery::new("Vec<u8>")
        .filter("drain")
        .run_with_definitions(&ra_path)?;

    println!("Vec<u8> methods matching \"drain\" with source locations:");
    for r in &results {
        match &r.definition {
            Some(def) => println!("  {}  →  {}:{}", r.method.name, def.path, def.line + 1),
            None => println!("  {}  →  (no source location)", r.method.name),
        }
    }

    println!();

    // -- Third-party type -- slow -- use an exact filter to avoid multiplying
    // Cargo resolution cost

    println!("Querying serde_json::Value::as_str (may be slow on first run)...");
    let results = MethodQuery::new("serde_json::Value")
        .deps(r#"serde_json = "1.0""#)
        .filter("as_str")
        .run_with_definitions(&ra_path)?;

    println!("serde_json::Value methods matching \"as_str\" with source locations:");
    for r in &results {
        match &r.definition {
            Some(def) => println!("  {}  →  {}:{}", r.method.name, def.path, def.line + 1),
            None => println!("  {}  →  (no source location)", r.method.name),
        }
    }

    Ok(())
}
