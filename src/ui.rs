// src/ui.rs

use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::process;

use crate::analyzer;

/// Holds the parsed command-line configuration.
#[allow(clippy::struct_excessive_bools)]
pub struct Opts {
    pub bin: String,
    pub type_name: String,
    pub filter: Option<String>,
    pub interactive: bool,
    pub show_doc: bool,
    pub goto_def: Option<String>,
    pub open_def: bool,
    pub open_doc: bool,
}

/// Prints the CLI help menu with usage patterns and examples.
pub fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter] [-i] [--doc] [--gd <method>] [--open] [--open-doc]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping                 # fuzzy filter");
    eprintln!("  {bin} u8 -i                       # interactive picker");
    eprintln!("  {bin} u8 --doc                    # show doc comments inline");
    eprintln!("  {bin} u8 checked --doc            # filter + docs");
    eprintln!("  {bin} String --gd len             # print definition location");
    eprintln!("  {bin} u8 --gd checked_add         # go to definition");
    eprintln!("  {bin} u8 --gd checked_add --open  # open in $EDITOR");
    eprintln!("  {bin} u8 --gd checked_add --open-doc  # open in browser");
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
    let mut open_doc = false;

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
            "--open-doc" => open_doc = true,
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

    if open_def && goto_def.is_none() {
        return Err("--open requires --gd <method>".to_string());
    }

    if open_doc && goto_def.is_none() {
        return Err("--open-doc requires --gd <method>".to_string());
    }
    if open_def && open_doc {
        return Err("choose only one of --open or --open-doc".to_string());
    }
    Ok(Opts {
        bin,
        type_name,
        filter,
        interactive,
        show_doc,
        goto_def,
        open_def,
        open_doc,
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

pub fn open_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");

    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        return c
            .status()
            .map_err(|e| format!("failed to open browser: {e}"))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("browser command exited with status {status}"))
                }
            });
    };

    #[cfg(not(target_os = "windows"))]
    {
        let status = cmd
            .arg(url)
            .status()
            .map_err(|e| format!("failed to open browser: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("browser command exited with status {status}"))
        }
    }
}

pub fn build_doc_url(type_name: &str, method_name: &str, def: &analyzer::Definition) -> String {
    // Strip generics: "Vec<i32>" → "Vec"
    let bare_type = type_name.split('<').next().unwrap_or(type_name);
    let type_lower = bare_type.to_lowercase();

    if is_stdlib_path(&def.full_path) {
        let (base, kind) = stdlib_type_info(bare_type);
        if base.is_empty() {
            format!("https://doc.rust-lang.org/std/{kind}.{type_lower}.html#method.{method_name}")
        } else if base == "primitive" {
            format!(
                "https://doc.rust-lang.org/std/primitive.{type_lower}.html#method.{method_name}"
            )
        } else {
            format!(
                "https://doc.rust-lang.org/std/{base}/{kind}.{type_lower}.html#method.{method_name}"
            )
        }
    } else if let Some(crate_name) = cargo_crate_name(&def.full_path) {
        let kind = third_party_kind(bare_type);
        format!(
            "https://docs.rs/{crate_name}/latest/{crate_name}/{kind}.{bare_type}.html#method.{method_name}"
        )
    } else {
        format!("https://docs.rs/releases/search?query={type_name}+{method_name}")
    }
}

fn is_stdlib_path(full_path: &str) -> bool {
    full_path.contains("/library/core/")
        || full_path.contains("/library/std/")
        || full_path.contains("/library/alloc/")
}

/// Returns (`module_path`, `kind`) for a stdlib type.
/// Primitives use "primitive", alloc/std structs use their module + "struct".
fn stdlib_type_info(bare_type: &str) -> (&'static str, &'static str) {
    match bare_type {
        // primitives
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" | "bool" | "char" | "str" => ("primitive", "primitive"), // alloc structs re-exported via std
        "String" => ("string", "struct"),
        "Vec" => ("vec", "struct"),
        "Box" => ("boxed", "struct"),
        "Rc" => ("rc", "struct"),
        "Arc" => ("sync", "struct"),
        "HashMap" => ("collections/hash_map", "struct"),
        "HashSet" => ("collections/hash_set", "struct"),
        "BTreeMap" => ("collections/btree_map", "struct"),
        "BTreeSet" => ("collections/btree_set", "struct"),
        "VecDeque" => ("collections/vec_deque", "struct"),
        "LinkedList" => ("collections/linked_list", "struct"),
        "BinaryHeap" => ("collections/binary_heap", "struct"),
        "Option" => ("option", "enum"),
        "Result" => ("result", "enum"),
        // fallback: guess struct under std
        _ => ("", "struct"),
    }
}

/// Best-effort kind for third-party types. Defaults to "struct".
/// Extend this if you start hitting enums or traits in practice.
const fn third_party_kind(_bare_type: &str) -> &'static str {
    "struct"
}

/// Extracts the crate name from a Cargo registry path.
/// e.g. ~/.cargo/registry/src/index.crates.io-xxx/serde-1.0.197/src/lib.rs → "serde"
fn cargo_crate_name(full_path: &str) -> Option<String> {
    let marker = "/registry/src/";
    let idx = full_path.find(marker)?;
    let after_registry = &full_path[idx + marker.len()..];

    // skip index hash dir (e.g. "index.crates.io-6f17d22bba15001f/")
    let after_index = after_registry.split_once('/')?.1;

    // 1. Get the next segment string slice by calling .next() on the split iterator
    let crate_dir = after_index.split('/').next()?;

    // 2. Now crate_dir is a valid &str (e.g., "crate-name-1.2.3")
    let name = strip_version_suffix(crate_dir);

    Some(name.replace('-', "_"))
}

fn strip_version_suffix(crate_dir: &str) -> &str {
    // "serde-1.0.197" → "serde", "my-crate-0.1.0" → "my_crate"
    let parts: Vec<&str> = crate_dir.rsplitn(10, '-').collect();
    let mut drop = 0;
    for part in &parts {
        if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            drop += 1;
        } else {
            break;
        }
    }
    if drop == 0 {
        return crate_dir;
    }
    let total_len: usize = parts[drop..].iter().map(|s| s.len() + 1).sum();
    &crate_dir[..total_len.saturating_sub(1)]
}
