//! `rust-meth`: A CLI utility to discover and filter methods available on Rust types.
//! It leverages `rust-analyzer` via the Language Server Protocol (LSP) to provide
//! accurate, context-aware method resolution.

mod analyzer;
mod lsp;
mod probe;
mod ui;

use std::process;

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
    let opts = ui::parse_args()?;

    let ra_path = analyzer::find_rust_analyzer().map_err(|e| e.to_string())?;

    // Handle --gd before querying methods. Avoids spinning up RA twice.
    if let Some(method_name) = &opts.goto_def {
        match analyzer::query_definition(&opts.type_name, method_name, &ra_path)
            .map_err(|e| e.to_string())?
        {
            Some(def) => {
                println!(
                    "{}::{}  {}:{}",
                    opts.type_name,
                    method_name,
                    def.path,
                    def.line + 1
                );
                if opts.open_def {
                    ui::open_in_editor(&def)?;
                }

                if opts.open_doc {
                    let url = ui::build_doc_url(&opts.type_name, method_name, &def);
                    ui::open_in_browser(&url)?;
                }
            }
            None => {
                return Err(format!(
                    "No definition found for `{}::{}` — is rust-src installed?\n\
                     Run: rustup component add rust-src",
                    opts.type_name, method_name
                ));
            }
        }
        return Ok(());
    }

    let methods = analyzer::query_methods(&opts.type_name, &ra_path).map_err(|e| e.to_string())?;

    if opts.interactive {
        return ui::run_interactive(&opts, &methods);
    }

    let matched = ui::filter_methods(&methods, opts.filter.as_deref());

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
        ui::print_method(m, name_width, opts.show_doc);
    }

    println!("\n{} method(s)", matched.len());
    Ok(())
}
