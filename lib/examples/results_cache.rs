//! # `results_cache`
//!
//! Demonstrates the results cache introduced in `0.4.0`.
//!
//! The first call to `query_methods` for a given type runs a full
//! `rust-analyzer` LSP session and saves the results to
//! `$XDG_CACHE_HOME/rust-meth/results/`. Subsequent calls, in the same
//! process or a later one, return immediately from the JSON cache with
//! zero LSP cost.
//!
//! This example queries `Vec<u8>` twice and times both calls to show the
//! difference. Run it a second time to see the persistent cache in action.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.4.0"
//! ```
//!
//! ## Returns
//!
//! Two timed queries against `Vec<u8>`:
//! - **First call**: full LSP session (~3-4s)
//! - **Second call**: results cache hit (~0ms)

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};
use rust_meth_lib::results_cache::results_cache_entries;
use std::time::Instant;

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // Uncomment to clear the cache on every run
    // rust_meth_lib::results_cache::clear_results_cache().ok();

    // -- First call -- cold results cache --
    println!("First call (cold cache)...");
    let t1 = Instant::now();
    let methods = query_methods("Vec<u8>", &ra_path, None)?;
    let elapsed1 = t1.elapsed();

    println!("  {} methods found in {:.2?}\n", methods.len(), elapsed1);

    // Inspect what was saved
    let entries = results_cache_entries();
    println!("Results cache now has {} entry:", entries.len());
    for e in &entries {
        println!(
            "  {} : {} methods (ra: {})",
            e.type_name, e.method_count, e.ra_version
        );
    }
    println!();

    // -- Second call -- results cache hit --
    println!("Second call (warm cache)...");
    let t2 = Instant::now();
    let methods2 = query_methods("Vec<u8>", &ra_path, None)?;
    let elapsed2 = t2.elapsed();

    println!("  {} methods found in {:.2?}\n", methods2.len(), elapsed2);

    // Summary
    let speedup = elapsed1.as_secs_f64() / elapsed2.as_secs_f64();
    println!("Speedup: {speedup:.0}x faster on cache hit");
    println!("\nRun this example again to see the persistent cache kick in");
    println!(
        "on the very first call (~0ms instead of ~{}s).",
        elapsed1.as_secs()
    );

    Ok(())
}
