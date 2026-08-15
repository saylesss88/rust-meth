use crate::ui::{self, Opts};
use owo_colors::OwoColorize;
use rust_meth_lib::analyzer;
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
pub fn run(version: &str) -> Result<(), String> {
    let opts = parse_or_exit(version)?;
    let ra_path = analyzer::find_rust_analyzer().map_err(|e| e.to_string())?;

    if handle_definition_mode(&opts, &ra_path)? {
        return Ok(());
    }

    if handle_explain_mode(&opts, &ra_path)? {
        return Ok(());
    }

    let methods = query_methods_with_spinner(&opts, &ra_path)?;

    if opts.interactive {
        return ui::run_interactive(&opts, &methods);
    }

    let matched = filter_or_err(&opts, &methods)?;
    render_methods(&opts, &matched)
}

/// Handles `--explain <method>`: finds the method in the queried list and prints its full docs.
///
/// If `--browser` is also set, builds a doc URL and opens it in the system browser instead.
/// Returns `Ok(true)` if explain mode was active, `Ok(false)` otherwise.
fn handle_explain_mode(opts: &Opts, ra_path: &std::path::Path) -> Result<bool, String> {
    let Some(method_name) = &opts.explain else {
        return Ok(false);
    };

    let methods = query_methods_with_spinner(opts, ra_path)?;

    let method = methods
        .iter()
        .find(|m| m.name == *method_name)
        .ok_or_else(|| {
            format!(
                "method `{}` not found on type `{}`",
                method_name, opts.type_name
            )
        })?;

    if opts.browser {
        // Build URL without a full definition lookup:
        // We construct a "best-guess" URL directly from type + method name.
        let url = build_explain_url(&opts.type_name, method_name);
        ui::open_in_browser(&url)?;
    } else {
        ui::print_full_doc(method);
    }

    Ok(true)
}

/// Builds a best-effort doc URL for `--explain --browser` without needing a definition lookup.
fn build_explain_url(type_name: &str, method_name: &str) -> String {
    // Reuse the same logic as `build_doc_url` but without a Definition.
    // We can build a reasonable URL for stdlib types using the type name alone.
    let bare_type = type_name.split('<').next().unwrap_or(type_name);

    // Hardcode stdlib types we know about; everything else falls back to docs.rs search.
    match bare_type {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" | "bool" | "char" | "str" => {
            format!(
                "https://doc.rust-lang.org/std/primitive.{}.html#method.{}",
                bare_type.to_lowercase(),
                method_name
            )
        }
        "String" => {
            format!("https://doc.rust-lang.org/std/string/struct.String.html#method.{method_name}")
        }
        "Vec" => format!("https://doc.rust-lang.org/std/vec/struct.Vec.html#method.{method_name}"),
        "Option" => {
            format!("https://doc.rust-lang.org/std/option/enum.Option.html#method.{method_name}")
        }
        "Result" => {
            format!("https://doc.rust-lang.org/std/result/enum.Result.html#method.{method_name}")
        }
        "HashMap" => format!(
            "https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html#method.{method_name}"
        ),
        "HashSet" => format!(
            "https://doc.rust-lang.org/std/collections/hash_set/struct.HashSet.html#method.{method_name}"
        ),
        _ => format!("https://docs.rs/releases/search?query={bare_type}+{method_name}"),
    }
}
/// Parses command-line arguments using the UI configuration module.
///
/// # Errors
///
/// Returns an `Err` if the command-line arguments are malformed or missing required keys.
///
/// # Panics
///
/// **Process Exit:** This function will directly terminate the current process with an exit
/// code of `0` if the user requests `--help` or `--version`.
fn parse_or_exit(version: &str) -> Result<Opts, String> {
    match ui::parse_args(version)? {
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
