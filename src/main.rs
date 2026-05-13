mod analyzer;
mod lsp;
mod probe;

use std::process;

fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping");
}

fn main() {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        usage(&bin);
        process::exit(0);
    }

    let type_name = &args[0];
    let filter = args.get(1).map(String::as_str);

    // Locate rust-analyzer.
    let ra_path = match analyzer::find_rust_analyzer() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Run the LSP session.
    let methods = match analyzer::query_methods(type_name, &ra_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    // Apply optional filter.
    let matched: Vec<_> = filter.map_or_else(
        || methods.iter().collect(),
        |pat| methods.iter().filter(|m| m.name.contains(pat)).collect(),
    );

    if matched.is_empty() {
        match filter {
            Some(pat) => eprintln!("No methods on `{type_name}` matching {pat:?}"),
            None => eprintln!("No methods found for type `{type_name}`"),
        }
        process::exit(1);
    }

    // Header.
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
