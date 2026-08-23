//! # `filter_methods`
//!
//! Filters and ranks methods against a query string without re-querying
//! `rust-analyzer`. Useful when you already have a [`Vec<Method>`] and want
//! to narrow results. Exact matches score highest, then prefix, then substring.
//! Non-matching methods are excluded entirely.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.4.0"
//! ```
//!
//! ## Returns
//!
//! Methods on `HashMap<String, u32>` whose names match `"get"`, ranked by
//! match quality (exact > prefix > substring).

use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};
use rust_meth_lib::query::filter_methods;

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;
    let methods = query_methods("HashMap<String, u32>", &ra_path, None)?;
    let filtered = filter_methods(&methods, "get");
    println!("HashMap<String, u32> methods matching \"get\":\n");
    for m in filtered {
        println!("{}", m.name);
    }
    Ok(())
}
