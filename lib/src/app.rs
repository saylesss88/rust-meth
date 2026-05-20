use crate::analyzer;
use crate::ui::{self, Opts};
use owo_colors::OwoColorize;
use std::process;
use std::time::Instant;

/// The primary entry point for the application logic.
///
/// This function coordinates the entire lifecycle of the tool: parsing arguments,
/// locating `rust-analyzer`, handling specialized execution paths like "go-to-definition",
/// querying for methods, and dispatching to either interactive UI or standard rendering modes.
///
/// # Errors
///
/// Returns an `Err` containing a descriptive message if:
/// * CLI arguments fail to parse.
/// * The `rust-analyzer` binary cannot be located on the system.
/// * A requested definition or method query fails or returns no results.
/// * Rendering or writing JSON output encounters an unrecoverable serialization error.
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

/// Parses command-line arguments using the UI configuration module.
///
/// If the arguments match standard configuration parameters, it extracts and returns `Opts`.
/// If the arguments contain explicit user requests for help or version information, this
/// function handles printing that metadata directly to standard output or standard error
/// and terminates the active process.
///
/// # Errors
///
/// Returns an `Err` if the command-line arguments are malformed or missing required keys.
///
/// # Panics
///
/// **Process Exit:** This function will directly terminate the current process with an exit
/// code of `0` if the user requests `--help` or `--version`.
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

/// Evaluates whether the application is running in "go-to-definition" mode and executes it.
///
/// If `opts.goto_def` is populated, this function intercepts the execution flow to trace down
/// where a specific method is defined using `rust-analyzer`. It handles rendering progress animations
/// and optionally opening the file location in an external text editor or browser documentation.
///
/// # Returns
///
/// * `Ok(true)` if definition mode was active and processed successfully.
/// * `Ok(false)` if definition mode was not requested, indicating the caller should continue execution.
///
/// # Errors
///
/// Returns an `Err` if the definition query fails, or if a requested file/browser-open operation
/// fails via the operating system host hook.
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

/// Dispatches a query to `rust-analyzer` to parse and collect all methods belonging to a target type.
///
/// Implements a terminal spinner animation to indicate indexing latency and logs performance metrics
/// to standard output upon successful extraction.
///
/// # Errors
///
/// Returns an `Err` containing the underlying analysis layer failure if parsing or background
/// indexing crashes.
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

/// Filters a slice of methods using user-provided query patterns.
///
/// # Errors
///
/// Returns a descriptive `Err` if the filtering pass yields zero matches, helping prevent
/// confusing blank outputs in non-interactive pipeline contexts.
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

/// Handles formatting and rendering the filtered method set to standard output.
///
/// Supports three discrete visualization variants derived from application flags:
/// 1. **JSON:** Serializes matches into pretty-printed structured payloads.
/// 2. **Snippets:** Loops over and renders code blocks or implementation footprints.
/// 3. **Tabular/Standard:** Computes target terminal alignment margins and prints colorized output summaries.
///
/// # Errors
///
/// Returns an `Err` if JSON payload serialization breaks down due to systemic memory allocation or internal typing faults.
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

    println!(
        "\n{} {}",
        matched.len().to_string().bold().yellow(),
        "method(s)".dimmed()
    );
    Ok(())
    // println!("\n{} method(s)", matched.len());
    //     Ok(())
}

/// Prints a styled contextual header to standard output matching current filtering settings.
fn print_methods_header(opts: &Opts) {
    let bin = opts.bin.as_str().dimmed().to_string();
    let type_name = format!("`{}`", opts.type_name).bold().cyan().to_string();

    match opts.filter.as_deref() {
        Some(pat) => println!(
            "\n{}: methods on {} matching {}\n",
            bin,
            type_name,
            format!("{pat:?}").yellow()
        ),
        None => println!("\n{bin}: methods on {type_name}\n"),
    }
}
