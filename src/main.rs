//! `rust-meth`: A CLI utility to discover and filter methods available on Rust types.
//! It leverages `rust-analyzer` via the Language Server Protocol (LSP) to provide
//! accurate, context-aware method resolution.

mod analyzer;
mod lsp;
mod probe;

use std::process;

use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Prints the CLI help menu with usage patterns and examples.
fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter|-i] [--version]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping         # fuzzy filter");
    eprintln!("  {bin} u8 -i               # interactive picker");
}

fn main() {
    // Determine binary name for help text, defaulting to "rust-meth".
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Check for help or version flags
    if args.is_empty() {
        usage(&bin);
        process::exit(0);
    }

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        usage(&bin);
        process::exit(0);
    }

    if args[0] == "--version" || args[0] == "-V" {
        // env!("CARGO_PKG_VERSION") fetches the version from Cargo.toml
        println!("{} {}", bin, env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    if args[0].starts_with('-') {
        eprintln!("error: unexpected argument '{}'", args[0]);
        usage(&bin);
        process::exit(1);
    }

    let type_name = &args[0];
    let interactive = args.iter().any(|a| a == "-i" || a == "--interactive");
    let filter = if interactive {
        None
    } else {
        args.get(1).map(String::as_str)
    };

    // Locate the rust-analyzer binary in the system PATH.
    let ra_path = match analyzer::find_rust_analyzer() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Spin up an ephemeral LSP session to query available methods for the type.
    let methods = match analyzer::query_methods(type_name, &ra_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Interactive fuzzy selector.
    if interactive {
        let items: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Methods on `{type_name}`"))
            .items(&items)
            .interact_opt();

        match selection {
            Ok(Some(idx)) => {
                let m = &methods[idx];
                match &m.detail {
                    Some(detail) => println!("  {}  {detail}", m.name),
                    None => println!("  {}", m.name),
                }
            }
            Ok(None) => {} // user hit Esc
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Apply optional fuzzy filter, sorted by match quality.
    let matched: Vec<_> = filter.map_or_else(
        || methods.iter().collect(),
        |pat| {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<_> = methods
                .iter()
                .filter_map(|m| matcher.fuzzy_match(&m.name, pat).map(|score| (score, m)))
                .collect();
            scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            scored.into_iter().map(|(_, m)| m).collect()
        },
    );

    if matched.is_empty() {
        match filter {
            Some(pat) => eprintln!("No methods on `{type_name}` matching {pat:?}"),
            None => eprintln!("No methods found for type `{type_name}`"),
        }
        process::exit(1);
    }

    // Display results in a formatted table with aligned columns.
    match filter {
        Some(pat) => println!("{bin}: methods on `{type_name}` matching {pat:?}\n"),
        None => println!("{bin}: methods on `{type_name}`\n"),
    }

    // Compute column width for aligned output.
    let name_width = matched.iter().map(|m| m.name.len()).max().unwrap_or(0);

    for m in &matched {
        match &m.detail {
            Some(detail) => println!("  {:<name_width$}  {detail}", m.name),
            None => println!("  {}", m.name),
        }
    }

    println!("\n{} method(s)", matched.len());
}
