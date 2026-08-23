//! # `probe_cache`
//!
//! Demonstrates the in-process probe cache introduced in `0.3.0`.
//!
//! [`query_methods_batch`] populates the cache as a side effect of running
//! queries. [`cache_entries`] lets you inspect what's currently cached.
//! Useful for debugging, logging, or displaying cache state in your own tool.
//! [`clear_probe_cache`] evicts all entries; each directory is deleted when
//! its [`Arc`] refcount reaches zero.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.3.0"
//! ```
//!
//! ## Returns
//!
//! - **Before queries**: empty cache
//! - **After batch query**: one entry per unique `(type_name, deps, probe_kind)`
//! - **After clear**: empty cache, probe directories deleted from disk

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods_batch};
use rust_meth_lib::probe::{cache_entries, clear_probe_cache};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // ── Before any queries ───────────────────────────────────────────────────

    println!("Cache before queries: {} entries", cache_entries().len());
    println!();

    // ── Run a batch query ────────────────────────────────────────────────────

    let queries: &[(&str, Option<&str>)] = &[
        ("Vec<u8>", None),
        ("String", None),
        ("HashMap<String, u32>", None),
    ];

    println!("Running batch query...");
    let results = query_methods_batch(queries, &ra_path);
    for (type_name, result) in &results {
        match result {
            Ok(methods) => println!("  {type_name}: {} methods", methods.len()),
            Err(e) => println!("  {type_name}: FAILED: {e}"),
        }
    }
    println!();

    // ── Inspect the cache ────────────────────────────────────────────────────

    let mut entries = cache_entries();
    entries.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    println!("Cache after batch ({} entries):", entries.len());
    println!("{:<30} {:<20} {:<12} dir", "type", "deps", "kind");
    println!("{}", "-".repeat(90));
    for entry in &entries {
        let deps = entry.deps.as_deref().unwrap_or("none");
        let kind = entry.method_name.as_deref().unwrap_or("completion");
        println!(
            "{:<30} {:<20} {:<12} {}",
            entry.type_name,
            deps,
            kind,
            entry.dir.display()
        );
    }
    println!();

    // ── Verify dirs exist on disk ────────────────────────────────────────────

    let all_exist = entries.iter().all(|e| e.dir.exists());
    println!("All probe dirs exist on disk: {all_exist}");
    println!();

    // ── Clear the cache ──────────────────────────────────────────────────────

    clear_probe_cache();
    println!("Cache cleared.");
    println!("Cache after clear: {} entries", cache_entries().len());

    // Dirs are deleted when the Arc refcount reaches zero. Since no Probe
    // instances are alive at this point, they should be gone immediately.
    let any_exist = entries.iter().any(|e| e.dir.exists());
    println!("Any probe dirs still on disk: {any_exist}");

    Ok(())
}
