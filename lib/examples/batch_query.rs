//! # batch_query
//!
//! Querying multiple types and aggregating the results.
//!
//! A realistic use case for tooling authors: generate a method inventory
//! across a family of related types, find methods common to all of them,
//! or produce a diff-style view showing what each type adds over a base.
//!
//! Note: each `query_methods` call spins up its own ephemeral `rust-analyzer`
//! session. Sessions are independent and don't share state. For a large batch
//! you'll want to handle errors per-type rather than letting one failure abort
//! the whole run, this example shows that pattern.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.2.4"
//! ```
//!
//! ## Returns
//!
//! For `Vec<u8>`, `VecDeque<u8>`, and `LinkedList<u8>`:
//!
//! - **Per-type method counts**: printed as each query completes
//! - **Common methods**: the intersection across all three types
//! - **Unique methods**: what each type has that neither of the others does

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};
use rust_meth_lib::error::RustMethError;
use std::collections::{BTreeMap, BTreeSet};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    let types = ["Vec<u8>", "VecDeque<u8>", "LinkedList<u8>"];

    // ── Per-type query with per-type error handling ──────────────────────────
    //
    // Collect into a BTreeMap so results are ordered and we can compare them.
    let mut results: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut failures: Vec<(&str, RustMethError)> = Vec::new();

    for type_name in &types {
        print!("Querying {type_name}... ");
        match query_methods(type_name, &ra_path, None) {
            Ok(methods) => {
                let names: BTreeSet<String> = methods.into_iter().map(|m| m.name).collect();
                println!("{} methods", names.len());
                results.insert(type_name, names);
            }
            Err(e) => {
                println!("FAILED: {e}");
                failures.push((type_name, e));
            }
        }
    }

    println!();

    // ── Methods common to all successfully queried types ─────────────────────

    if results.len() >= 2 {
        let mut common = results.values().next().cloned().unwrap_or_default();
        for set in results.values().skip(1) {
            common = common.intersection(set).cloned().collect();
        }

        println!(
            "Methods common to all {} types ({} total):",
            results.len(),
            common.len()
        );
        for name in &common {
            println!("  {name}");
        }
        println!();
    }

    // ── Per-type unique methods ──────────────────────────────────────────────
    //
    // What does each type have that none of the others do?

    for (type_name, methods) in &results {
        let others_union: BTreeSet<&String> = results
            .iter()
            .filter(|(t, _)| *t != type_name)
            .flat_map(|(_, s)| s.iter())
            .collect();

        let unique: Vec<&String> = methods
            .iter()
            .filter(|m| !others_union.contains(m))
            .collect();

        if unique.is_empty() {
            println!("{type_name}: no unique methods");
        } else {
            println!("{type_name} unique methods ({}):", unique.len());
            for name in unique {
                println!("  {name}");
            }
        }
        println!();
    }

    // ── Summary of failures ──────────────────────────────────────────────────

    if !failures.is_empty() {
        println!("Failed queries:");
        for (type_name, err) in failures {
            println!("  {type_name}: {err}");
        }
    }

    Ok(())
}
