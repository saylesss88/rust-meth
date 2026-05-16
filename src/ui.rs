// src/ui.rs

use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::process;

use crate::analyzer;

/// Holds the parsed command-line configuration.
pub struct Opts {
    pub bin: String,
    pub type_name: String,
    pub filter: Option<String>,
    pub interactive: bool,
    pub show_doc: bool,
    pub goto_def: Option<String>,
    pub open_def: bool,
}

/// Prints the CLI help menu with usage patterns and examples.
pub fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter] [-i] [--doc]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping            # fuzzy filter");
    eprintln!("  {bin} u8 -i                  # interactive picker");
    eprintln!("  {bin} u8 --doc               # show doc comments inline");
    eprintln!("  {bin} u8 checked --doc       # filter + docs");
    eprintln!("Usage: {bin} <type> [filter] [-i] [--doc] [--gd <method>]");
    eprintln!("  {bin} String --gd len       # go to method definition");
    eprintln!("  {bin} u8 --gd checked_add        # go to definition");
    eprintln!("  {bin} u8 --gd checked_add --open  # open in $EDITOR");
}

/// Hand-rolls argument parsing to support positional arguments and flags.
pub fn parse_args() -> Result<Opts, String> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let mut args = std::env::args().skip(1);

    let Some(first) = args.next() else {
        usage(&bin);
        process::exit(0);
    };

    if matches!(first.as_str(), "--help" | "-h") {
        usage(&bin);
        process::exit(0);
    }

    if matches!(first.as_str(), "--version" | "-V") {
        println!("{} {}", bin, env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    if first.starts_with('-') {
        usage(&bin);
        return Err(format!("unexpected argument '{first}'"));
    }

    let type_name = first;
    let mut filter = None;
    let mut interactive = false;
    let mut show_doc = false;
    let mut goto_def = None;
    let mut open_def = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--interactive" => interactive = true,
            "-d" | "--doc" => show_doc = true,
            "--gd" => {
                let method = args
                    .next()
                    .ok_or_else(|| "--gd requires a method name".to_string())?;
                goto_def = Some(method);
            }
            "--open" | "-o" => open_def = true,
            _ if arg.starts_with('-') => {
                return Err(format!("unexpected flag '{arg}'"));
            }
            _ => {
                if filter.is_none() {
                    filter = Some(arg);
                } else {
                    return Err(format!("unexpected argument '{arg}'"));
                }
            }
        }
    }

    if interactive {
        filter = None;
    }

    Ok(Opts {
        bin,
        type_name,
        filter,
        interactive,
        show_doc,
        goto_def,
        open_def,
    })
}

/// Displays a fuzzy-searchable list in the terminal using `dialoguer`.
pub fn run_interactive(opts: &Opts, methods: &[analyzer::Method]) -> Result<(), String> {
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
pub fn filter_methods<'a>(
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
pub fn print_method(m: &analyzer::Method, name_width: usize, show_doc: bool) {
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

pub fn open_in_editor(def: &analyzer::Definition) -> Result<(), String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .map_err(|_| "$EDITOR and $VISUAL are not set".to_string())?;

    let path = &def.full_path;
    let line = def.line + 1;

    let status = match editor.as_str() {
        "hx" | "helix" => std::process::Command::new(&editor)
            .arg(format!("{path}:{line}"))
            .status(),
        "code" | "code-insiders" => std::process::Command::new(&editor)
            .args(["--goto", &format!("{path}:{line}")])
            .status(),
        _ => std::process::Command::new(&editor)
            .arg(format!("+{line}"))
            .arg(path)
            .status(),
    };

    status.map_err(|e| format!("Failed to launch {editor}: {e}"))?;
    Ok(())
}
