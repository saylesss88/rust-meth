//! # `third_party_crate`
//!
//! Querying methods on types from external crates.
//!
//! The `deps` argument to `query_methods` accepts a raw TOML snippet: the
//! same text that would appear under `[dependencies]` in a Cargo.toml.
//! For multiple crates, separate entries with newlines.
//!
//! Note: the first time you query a third-party type, Cargo must download
//! and compile the crate inside the ephemeral probe project. This can take
//! 10–30 seconds on a cold cache. Subsequent queries against the same crate
//! version are fast because Cargo reuses the registry cache.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.2.4"
//! ```
//!
//! ## Returns
//!
//! Methods available for `serde::json::Value`.
//! Compare against <https://docs.rs/serde_json/latest/serde_json/enum.Value.html>

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // ── Single external crate ────────────────────────────────────────────────

    let serde_deps = r#"serde_json = "1.0""#;

    println!("Querying serde_json::Value methods (may be slow on first run)...");
    let methods = query_methods("serde_json::Value", &ra_path, Some(serde_deps))?;

    println!("serde_json::Value has {} methods:", methods.len());
    for m in &methods {
        println!("  {}", m.name);
    }

    println!();

    // ── Multiple dependencies ────────────────────────────────────────────────
    //
    // If your type requires more than one crate (e.g. a type that uses serde
    // traits), pass all deps as a single newline-separated string. This maps
    // directly to what Cargo expects under [dependencies].

    let multi_deps = "serde = { version = \"1.0\", features = [\"derive\"] }\nserde_json = \"1.0\"";

    println!("Querying serde_json::Map with serde + serde_json...");
    let map_methods = query_methods(
        "serde_json::Map<String, serde_json::Value>",
        &ra_path,
        Some(multi_deps),
    )?;

    println!("serde_json::Map has {} methods:", map_methods.len());
    for m in map_methods.iter().take(10) {
        println!("  {}", m.name);
    }
    if map_methods.len() > 10 {
        println!("  ... and {} more", map_methods.len() - 10);
    }

    Ok(())
}
