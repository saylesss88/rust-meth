//! # `fuzzy_filter`
//!
//! Using `rust-meth-lib` as a data source for your own search/filter UI.
//!
//! `query_methods` returns a plain `Vec<Method>`: already sorted and
//! deduplicated, ready to filter however you like. This example shows a few
//! patterns: substring match, signature grep, and a simple ranked scorer.
//! If you're building a TUI or editor plugin on top of the library, this
//! is the pattern to follow.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.3.0"
//! ```
//!
//! ## Returns
//!
//! Three views over `HashMap<String, u32>` methods, all queried once and filtered in-process:
//!
//! - **Substring filter**: methods whose name contains `"get"`
//! - **Signature grep**: methods whose `detail` field contains `-> Option`
//! - **Ranked results**: the same substring matches scored by match quality
//!   (exact > prefix > substring) and printed with a label

use rust_meth_lib::analyzer::{Method, find_rust_analyzer, query_methods};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;
    let methods = query_methods("HashMap<String, u32>", &ra_path, None)?;

    let query = "get";

    println!("All methods containing \"{query}\":");
    println!("{}", "-".repeat(40));

    // -- 1. Simple substring filter --
    let matched: Vec<&Method> = methods.iter().filter(|m| m.name.contains(query)).collect();

    for m in &matched {
        println!("  {}", m.name);
    }

    println!();

    // -- 2. Signature grep --
    //
    // Filter on the `detail` field to find methods with a specific return type.
    // Useful when you want to narrow results by type, not just name.

    let returns_option: Vec<&Method> = methods
        .iter()
        .filter(|m| {
            m.detail
                .as_deref()
                .is_some_and(|sig| sig.contains("-> Option"))
        })
        .collect();

    println!("Methods returning Option<_>:");
    println!("{}", "-".repeat(40));
    for m in &returns_option {
        println!(
            "  {}  →  {}",
            m.name,
            m.detail.as_deref().unwrap_or("(no signature)")
        );
    }

    println!();

    // -- 3. Simple ranked scorer --
    //
    // Score each method by how closely it matches the query: prefix match
    // scores higher than substring match, exact match highest. This gives
    // you a starting point for building fuzzy ranking without pulling in
    // a whole fuzzy-matching crate.

    println!("Ranked results for \"{query}\":");
    println!("{}", "-".repeat(40));

    let mut scored: Vec<(u8, &Method)> = methods
        .iter()
        .filter_map(|m| {
            let score = if m.name == query {
                3 // exact
            } else if m.name.starts_with(query) {
                2 // prefix
            } else if m.name.contains(query) {
                1 // substring
            } else {
                return None;
            };
            Some((score, m))
        })
        .collect();

    // Sort descending by score; stable within each tier (already alpha-sorted
    // from query_methods).
    scored.sort_by_key(|item| std::cmp::Reverse(item.0));

    for (score, m) in scored {
        let label = match score {
            3 => "exact ",
            2 => "prefix",
            _ => "substr",
        };
        println!("  [{label}]  {}", m.name);
    }

    Ok(())
}
