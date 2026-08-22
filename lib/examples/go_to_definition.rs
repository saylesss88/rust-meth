//! # `go_to_definition`
//!
//! Resolves the source location of a specific method using
//! `query_definition`.
//!
//! This is the same operation as "go to definition" in your editor, it
//! returns the file path and line number where the method is declared in
//! the standard library or a third-party crate's source.
//!
//! `query_definition` returns `Ok(None)` when `rust-analyzer` can find the
//! type but has no source location for the method (e.g. compiler built-ins
//! or methods with no available source). It's not an error, just a miss.
//!
//! ```toml
//! [dependencies]
//! rust-meth-lib = "0.3.0"
//! ```
//! ## Returns
//!
//! Source location of each method, stdlib methods resolve to `rust-src`
//! (e.g. `library/alloc/src/vec/mod.rs`), third-party methods into the
//! Cargo registry cache (e.g. `~/.cargo/registry/src/.../serde_json-.../src/...`).

use rust_meth_lib::analyzer::{find_rust_analyzer, query_definition};

fn main() -> rust_meth_lib::error::Result<()> {
    let ra_path = find_rust_analyzer()?;

    // ── Stdlib method ────────────────────────────────────────────────────────

    print_definition("Vec<u8>", "push", &ra_path, None)?;
    print_definition("String", "push_str", &ra_path, None)?;
    print_definition("HashMap<String, u32>", "insert", &ra_path, None)?;

    // ── Third-party method ───────────────────────────────────────────────────

    let serde_deps = r#"serde_json = "1.0""#;
    print_definition("serde_json::Value", "as_str", &ra_path, Some(serde_deps))?;

    Ok(())
}

fn print_definition(
    type_name: &str,
    method_name: &str,
    ra_path: &std::path::Path,
    deps: Option<&str>,
) -> rust_meth_lib::error::Result<()> {
    print!("{type_name}::{method_name}  →  ");

    match query_definition(type_name, method_name, ra_path, deps)? {
        Some(def) => {
            // `path` is the display-friendly shortened form:
            //   "library/core/src/num/uint_macros.rs"
            //
            // `full_path` is the absolute path on disk, useful if you want
            // to open the file programmatically.
            //
            // `line` is 0-indexed (LSP convention). Add 1 for display.
            println!("{}:{}", def.path, def.line + 1);
            println!("       full: {}", def.full_path);
        }
        None => {
            // Not an error, `rust-analyzer` just doesn't have a source
            // location for this method. Common for compiler intrinsics.
            println!("(no source location found)");
        }
    }

    println!();
    Ok(())
}
