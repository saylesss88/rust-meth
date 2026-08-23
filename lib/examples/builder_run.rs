//! # `builder_run`
//!
//! Demonstrates the [`MethodQuery`] builder API for querying and filtering
//! methods in a single chain.
//!
//! [`MethodQuery::new`] → [`filter`](rust_meth_lib::query::MethodQuery::filter)
//! → [`run`](rust_meth_lib::query::MethodQuery::run) is the fast path: one
//! `rust-analyzer` session, results filtered in-process. No Cargo resolution
//! required for stdlib types.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.4.0"
//! ```
//!
//! ## Returns
//!
//! Methods on `String` whose names match `"push"`, ranked by match quality
//! (exact > prefix > substring), with full signatures.

use rust_meth_lib::analyzer::find_rust_analyzer;
use rust_meth_lib::query::MethodQuery;

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    let methods = MethodQuery::new("String").filter("push").run(&ra_path)?;

    println!("String methods matching \"push\":");
    for m in &methods {
        let sig = m.detail.as_deref().unwrap_or("(no signature)");
        println!("  {}  →  {sig}", m.name);
    }

    Ok(())
}
