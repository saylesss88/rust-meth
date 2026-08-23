//! # `builder_query`
//!
//! Demonstrates the [`MethodQuery`] builder API introduced in `0.4.0`.
//!
//! [`MethodQuery`] provides a chainable interface over [`query_methods`] and
//! [`query_definition`], keeping call sites readable when multiple options are
//! involved. Use [`run`] for methods only, or [`run_with_definitions`] to also
//! resolve source locations in parallel.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.4.0"
//! ```
//!
//! ## Returns
//!
//! - **Simple chain**: methods on `String` whose names match `"push"`, ranked
//!   by match quality
//! - **Full chain**: methods on `Vec<u8>` matching `"drain"`, each paired
//!   with its source location if resolvable

use rust_meth_lib::analyzer::find_rust_analyzer;
use rust_meth_lib::query::MethodQuery;

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // ── Simple chain: query + filter ─────────────────────────────────────────

    let methods = MethodQuery::new("String").filter("push").run(&ra_path)?;

    println!("String methods matching \"push\":");
    for m in &methods {
        let sig = m.detail.as_deref().unwrap_or("(no signature)");
        println!("  {}  →  {sig}", m.name);
    }

    println!();

    // ── Full chain: deps + filter + definitions ───────────────────────────────
    //
    // run_with_definitions fetches source locations for each filtered method
    // in parallel. Methods with no resolvable location are included with
    // definition: None rather than being dropped.
    // let results = MethodQuery::new("serde_json::Value")
    //     .deps(r#"serde_json = "1.0""#)
    //     .filter("as_")
    //     .run_with_definitions(&ra_path)?;
    //
    let results = MethodQuery::new("serde_json::Value")
        .deps(r#"serde_json = "1.0""#)
        .filter("as_str") // exact match, only one method
        .run_with_definitions(&ra_path)?;

    println!("serde_json::Value methods matching \"as_\" with source locations:");
    for r in &results {
        match &r.definition {
            Some(def) => println!("  {}  →  {}:{}", r.method.name, def.path, def.line + 1),
            None => println!("  {}  →  (no source location)", r.method.name),
        }
    }
    Ok(())
}
