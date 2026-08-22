//! # `method_info`
//!
//! Shows how to work with the richer fields on [`Method`]: the full signature
//! (`detail`) and the rustdoc string (`documentation`).
//!
//! Both are `Option<String>`, rust-analyzer doesn't always return them,
//! especially for trait-provided methods on generic types. This example
//! shows a pattern for rendering them gracefully.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.2.4"
//! ```

use rust_meth_lib::analyzer::{Method, find_rust_analyzer, query_methods};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // String has good documentation coverage in the stdlib, making it a
    // useful type for demonstrating the `documentation` field.
    let methods = query_methods("String", &ra_path, None)?;

    // Only print methods that have at least a signature. Methods with
    // neither detail nor docs still exist in the list, they're just
    // less useful for display purposes.
    let documented: Vec<&Method> = methods
        .iter()
        .filter(|m| m.detail.is_some() || m.documentation.is_some())
        .collect();

    println!(
        "{} of {} String methods have signatures or docs:\n",
        documented.len(),
        methods.len()
    );

    for method in documented {
        // The signature line (e.g. "pub fn push_str(&mut self, string: &str)")
        if let Some(sig) = &method.detail {
            println!("fn {}  →  {sig}", method.name);
        } else {
            println!("fn {} (no signature)", method.name);
        }

        // Rustdoc, if present: often multi-line, indent it for readability.
        if let Some(doc) = &method.documentation {
            // Just the first sentence to keep output manageable.
            let first_line = doc.lines().next().unwrap_or("").trim();
            if !first_line.is_empty() {
                println!("  doc: {first_line}");
            }
        }

        println!();
    }

    Ok(())
}
