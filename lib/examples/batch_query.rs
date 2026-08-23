//! # `batch_query`
//!
//! Querying multiple types and aggregating the results.
//!
//! A realistic use case for tooling authors: generate a method inventory
//! across a family of related types, find methods common to all of them,
//! or produce a diff-style view showing what each type adds over a base.
//!
//! [`query_methods_batch`] runs each query in parallel: one thread per type,
//! each with its own independent `rust-analyzer` subprocess. Total wall time
//! is roughly the cost of the slowest single query rather than the sum of all.
//! Errors are per-type and do not abort the batch.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.3.0"
//! ```
//!
//! ## Returns
//!
//! For `Vec<u8>`, `VecDeque<u8>`, and `LinkedList<u8>`:
//!
//! - **Per-type method counts**: printed after all queries complete
//! - **Common methods**: the intersection across all three types
//! - **Unique methods**: what each type has that neither of the others does

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods_batch};
use rust_meth_lib::error::RustMethError;
use std::collections::{BTreeMap, BTreeSet};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    let queries: &[(&str, Option<&str>)] = &[
        ("Vec<u8>", None),
        ("VecDeque<u8>", None),
        ("LinkedList<u8>", None),
    ];

    // All three queries run in parallel. Results come back in input order.
    let batch_results = query_methods_batch(queries, &ra_path);

    // -- Partition into successes and failures --

    let mut results: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut failures: Vec<(&str, RustMethError)> = Vec::new();

    for (type_name, result) in batch_results {
        match result {
            Ok(methods) => {
                let names: BTreeSet<String> = methods.into_iter().map(|m| m.name).collect();
                println!("{type_name}: {} methods", names.len());
                results.insert(type_name, names);
            }
            Err(e) => {
                eprintln!("{type_name}: FAILED: {e}");
                failures.push((type_name, e));
            }
        }
    }

    println!();

    // -- Methods common to all successfully queried types --

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

    // -- Per-type unique methods --
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

    // -- Summary of failures --

    if !failures.is_empty() {
        println!("Failed queries:");
        for (type_name, err) in failures {
            println!("  {type_name}: {err}");
        }
    }

    Ok(())
}
