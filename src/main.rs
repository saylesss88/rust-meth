//! `rust-meth`: A CLI utility to discover and filter methods available on Rust types.
//! It leverages `rust-analyzer` via the Language Server Protocol (LSP) to provide
//! accurate, context-aware method resolution.

mod analyzer;
mod lsp;
mod probe;

use std::process;

use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

/// Prints the CLI help menu with usage patterns and examples.
fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter] [-i] [--doc]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping           # fuzzy filter");
    eprintln!("  {bin} u8 -i                 # interactive picker");
    eprintln!("  {bin} u8 --doc              # show doc comments inline");
    eprintln!("  {bin} u8 checked --doc      # filter + docs");
}

/// Holds the parsed command-line configuration.
struct Opts {
    /// The name of the binary being executed.
    bin: String,
    /// The Rust type to inspect (e.g., "String" or "Vec<u8>")
    type_name: String,
    /// An optional fuzzy search pattern to filter results.
    filter: Option<String>,
    /// If true, launches a TUI picker for method selection.
    interactive: bool,
    /// If true, includes doc comments in the output
    show_doc: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

/// Orchestrates the tool's execution flow:
/// 1. Finds rust-analyzer.
/// 2. Queries for methods.
/// 3. Routes to interactive or batch output modes.
fn run() -> Result<(), String> {
    let opts = parse_args()?;

    let ra_path = analyzer::find_rust_analyzer().map_err(|e| e.to_string())?;
    let methods = analyzer::query_methods(&opts.type_name, &ra_path).map_err(|e| e.to_string())?;

    if opts.interactive {
        return run_interactive(&opts, &methods);
    }

    let matched = filter_methods(&methods, opts.filter.as_deref());

    if matched.is_empty() {
        return Err(match opts.filter.as_deref() {
            Some(pat) => format!("No methods on `{}` matching {pat:?}", opts.type_name),
            None => format!("No methods found for type `{}`", opts.type_name),
        });
    }

    match opts.filter.as_deref() {
        Some(pat) => println!(
            "{}: methods on `{}` matching {pat:?}\n",
            opts.bin, opts.type_name
        ),
        None => println!("{}: methods on `{}`\n", opts.bin, opts.type_name),
    }

    let name_width = matched.iter().map(|m| m.name.len()).max().unwrap_or(0);

    for m in &matched {
        print_method(m, name_width, opts.show_doc);
    }

    println!("\n{} method(s)", matched.len());
    Ok(())
}

/// Hand-rolls argument parsing to support positional arguments and flags.
/// Returns `Err` if required arguments are missing or invalid.
fn parse_args() -> Result<Opts, String> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        usage(&bin);
        process::exit(0);
    }

    if matches!(args[0].as_str(), "--version" | "-V") {
        println!("{} {}", bin, env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    if args[0].starts_with('-') {
        usage(&bin);
        return Err(format!("unexpected argument '{}'", args[0]));
    }

    let interactive = args
        .iter()
        .any(|a| matches!(a.as_str(), "-i" | "--interactive"));
    let show_doc = args.iter().any(|a| matches!(a.as_str(), "-d" | "--doc"));

    let filter = if interactive {
        None
    } else {
        args.iter().skip(1).find(|a| !a.starts_with('-')).cloned()
    };

    Ok(Opts {
        bin,
        type_name: args[0].clone(),
        filter,
        interactive,
        show_doc,
    })
}

/// Displays a fuzzy-searchable list in the terminal using `dialoguer`.
fn run_interactive(opts: &Opts, methods: &[analyzer::Method]) -> Result<(), String> {
    let items: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Methods on `{}`", opts.type_name))
        .items(&items)
        .interact_opt()
        .map_err(|e| e.to_string())?;

    if let Some(idx) = selection {
        print_method(&methods[idx], 0, opts.show_doc);
    }

    Ok(())
}

/// Applies fuzzy matching to the list of methods.
/// If `filter` is `None`, returns all methods. Results are sorted by match score.
fn filter_methods<'a>(
    methods: &'a [analyzer::Method],
    filter: Option<&str>,
) -> Vec<&'a analyzer::Method> {
    filter.map_or_else(
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
    )
}

/// Formats and prints a single method to stdout.
/// Optionally includes documentation snippets (truncated to 6 lines).
fn print_method(m: &analyzer::Method, name_width: usize, show_doc: bool) {
    match &m.detail {
        Some(detail) if name_width > 0 => println!("  {:<name_width$}  {detail}", m.name),
        Some(detail) => println!("  {}  {detail}", m.name),
        None => println!("  {}", m.name),
    }

    if show_doc && let Some(doc) = &m.documentation {
        println!();
        for line in doc.lines().take(6) {
            println!("    {line}");
        }

        if name_width > 0 {
            println!();
        }
    }
}
