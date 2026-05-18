use crate::ui::{self, Opts};
use rust_meth::analyzer;
use std::process;
use std::time::Instant;

/// The primary entry point for the application logic.
pub fn run() -> Result<(), String> {
    let opts = parse_or_exit()?;
    let ra_path = analyzer::find_rust_analyzer().map_err(|e| e.to_string())?;

    if handle_definition_mode(&opts, &ra_path)? {
        return Ok(());
    }

    let methods = query_methods_with_spinner(&opts, &ra_path)?;

    if opts.interactive {
        return ui::run_interactive(&opts, &methods);
    }

    let matched = filter_or_err(&opts, &methods)?;
    render_methods(&opts, &matched)
}

fn parse_or_exit() -> Result<Opts, String> {
    match ui::parse_args()? {
        ui::ParseResult::Opts(opts) => Ok(opts),
        ui::ParseResult::Help(text) => {
            eprint!("{text}");
            process::exit(0);
        }
        ui::ParseResult::Version(text) => {
            println!("{text}");
            process::exit(0);
        }
    }
}

fn handle_definition_mode(opts: &Opts, ra_path: &std::path::Path) -> Result<bool, String> {
    let Some(method_name) = &opts.goto_def else {
        return Ok(false);
    };

    let spinner = ui::definition(&opts.type_name, method_name);

    match analyzer::query_definition(&opts.type_name, method_name, ra_path, opts.deps.as_deref()) {
        Ok(Some(def)) => {
            spinner.finish_with_message("✓ Found definition");
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

            Ok(true)
        }
        Ok(None) => {
            spinner.finish_with_message("✗ Not found");
            Err(format!(
                "No definition found for `{}::{}` — is rust-src installed?\n\
                 Run: rustup component add rust-src",
                opts.type_name, method_name
            ))
        }
        Err(e) => {
            spinner.finish_with_message("✗ Query failed");
            Err(e.to_string())
        }
    }
}

fn query_methods_with_spinner(
    opts: &Opts,
    ra_path: &std::path::Path,
) -> Result<Vec<analyzer::Method>, String> {
    let start = Instant::now();
    let spinner = ui::indexing(&opts.type_name);

    match analyzer::query_methods(&opts.type_name, ra_path, opts.deps.as_deref()) {
        Ok(methods) => {
            let elapsed = start.elapsed();
            spinner.finish_with_message(format!(
                "✓ Found {} methods ({:.1}s)",
                methods.len(),
                elapsed.as_secs_f64()
            ));
            Ok(methods)
        }
        Err(e) => {
            spinner.finish_with_message("✗ Query failed");
            Err(e.to_string())
        }
    }
}

fn filter_or_err<'a>(
    opts: &Opts,
    methods: &'a [analyzer::Method],
) -> Result<Vec<&'a analyzer::Method>, String> {
    let matched = ui::filter_methods(methods, opts.filter.as_deref());

    if matched.is_empty() {
        let err_msg = opts.filter.as_deref().map_or_else(
            || format!("No methods found for type `{}`", opts.type_name),
            |pat| format!("No methods on `{}` matching {pat:?}", opts.type_name),
        );
        return Err(err_msg);
    }

    Ok(matched)
}

fn render_methods(opts: &Opts, matched: &[&analyzer::Method]) -> Result<(), String> {
    if opts.json {
        let out = serde_json::to_string_pretty(&matched).map_err(|e| e.to_string())?;
        println!("{out}");
        return Ok(());
    }

    if opts.snippet {
        for m in matched {
            ui::print_snippet(m);
        }
        return Ok(());
    }

    print_methods_header(opts);

    let name_width = matched.iter().map(|m| m.name.len()).max().unwrap_or(0);
    for m in matched {
        ui::print_method(m, name_width, opts.show_doc);
    }

    println!("\n{} method(s)", matched.len());
    Ok(())
}

fn print_methods_header(opts: &Opts) {
    match opts.filter.as_deref() {
        Some(pat) => println!(
            "\n{}: methods on `{}` matching {pat:?}\n",
            opts.bin, opts.type_name
        ),
        None => println!("\n{}: methods on `{}`\n", opts.bin, opts.type_name),
    }
}
