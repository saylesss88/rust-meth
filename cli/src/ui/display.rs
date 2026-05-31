use owo_colors::OwoColorize;
use rust_meth_lib::analyzer;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

// --- Syntax Highlighting setup ---
struct Highlighter {
    ps: SyntaxSet,
    ts: ThemeSet,
}

static HIGHLIGHTER: OnceLock<Highlighter> = OnceLock::new();

fn get_highlighter() -> &'static Highlighter {
    HIGHLIGHTER.get_or_init(|| Highlighter {
        ps: SyntaxSet::load_defaults_newlines(),
        ts: ThemeSet::load_defaults(),
    })
}

#[allow(clippy::doc_markdown)]
/// Prints text, applying syntect syntax highlighting to ```rust blocks.
fn print_doc_with_highlighting(doc: &str) {
    // Fall back to plain output when piped or NO_COLOR is set
    if !is_color_enabled() {
        for line in doc.lines() {
            println!("   {} {}", "|".dimmed(), line);
        }
        return;
    }
    let hl = get_highlighter();
    let syntax = hl
        .ps
        .find_syntax_by_extension("rs")
        .unwrap_or_else(|| hl.ps.find_syntax_plain_text());

    let mut in_rust_block = false;
    let mut highlighter: Option<HighlightLines<'_>> = None;

    for line in doc.lines() {
        if !in_rust_block {
            if line.trim_start().starts_with("```rust") {
                in_rust_block = true;
                highlighter = Some(HighlightLines::new(
                    syntax,
                    &hl.ts.themes["base16-ocean.dark"],
                ));
                // Print the opening fence dimmed
                println!("   {} {}", "|".dimmed(), "```rust".dimmed());
            } else {
                println!("   {} {}", "|".dimmed(), line);
            }
        } else if line.trim() == "```" {
            // Closing fence
            println!("   {} {}", "|".dimmed(), "```".dimmed());
            in_rust_block = false;
            highlighter = None;
        } else if let Some(ref mut h) = highlighter {
            // Highlight the code line
            let line_nl = format!("{line}\n");
            match h.highlight_line(&line_nl, &hl.ps) {
                Ok(ranges) => {
                    // false = don't include a reset at the end of every line
                    let colored = as_24_bit_terminal_escaped(&ranges[..], false);
                    // trim the trailing newline syntect adds so println! controls it
                    let colored = colored.trim_end_matches('\n');
                    println!("   {} {colored}", "|".dimmed());
                }
                Err(_) => println!("   {} {}", "|".dimmed(), line),
            }
        }
    }
}

/// Formats and prints a single method to stdout.
pub fn print_method(m: &analyzer::Method, name_width: usize, show_doc: bool) {
    let padded_name = if name_width > 0 {
        format!("{:<name_width$}", m.name)
    } else {
        m.name.clone()
    };

    let styled_name = padded_name.bold().green().to_string();

    match &m.detail {
        Some(detail) => println!("  {}  {}", styled_name, detail.dimmed()),
        None => println!("  {styled_name}"),
    }

    if show_doc && let Some(doc) = &m.documentation {
        println!();
        for line in doc.lines().take(6) {
            println!("    {} {}", "│".dimmed(), line.dimmed());
        }

        if doc.lines().count() > 6 {
            println!("    {} {}", "│".dimmed(), "…".dimmed());
        }

        if name_width > 0 {
            println!();
        }
    }
}
/// Formats a method signature into a call snippet.
///
/// For example, given a method with
///
/// ```text
/// detail = "pub fn checked_add(self, rhs: u8) -> Option<u8>"
/// ```
///
/// the output is:
///
/// ```text
///   checked_add(self, rhs: u8) -> Option<u8>
///   → x.checked_add(rhs)
/// ```
pub fn print_snippet(m: &analyzer::Method) {
    let Some(detail) = &m.detail else {
        println!("  {}", m.name);
        return;
    };

    // Print the full signature
    println!("  {detail}");

    // Build the call form by extracting param names (strip types)
    let call_args = parse_call_args(detail);
    println!("  → x.{}({})\n", m.name, call_args);
}

/// Strips "self" and type annotations from a signature's param list,
/// returning just the argument names for the call form.
/// e.g. "(self, rhs: u8, other: u8)" → "rhs, other"
fn parse_call_args(detail: &str) -> String {
    // Find the opening paren
    let Some(start) = detail.find('(') else {
        return String::new();
    };
    let end = detail.find(')').unwrap_or(detail.len());
    let params = &detail[start + 1..end];

    params
        .split(',')
        .map(str::trim)
        .filter(|p| *p != "self" && *p != "&self" && *p != "&mut self")
        .map(|p| {
            // Take only the name before the ':'
            p.split(':').next().unwrap_or(p).trim()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Respect `NO_COLOR` / piped output. When stdout isn't a TTY, syntect ANSI
/// codes are noise
fn is_color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::IsTerminal::is_terminal(&std::io::stdout())
}

#[allow(clippy::doc_markdown)]
/// Prints the full documentation for a single method, with syntax-highlighted ```rust blocks.
pub fn print_full_doc(m: &analyzer::Method) {
    println!("\n {} {}", "method:".dimmed(), m.name.bold().green());
    if let Some(detail) = &m.detail {
        println!("  {} {}", "sig:".dimmed(), detail.dimmed());
    }
    println!();
    match &m.documentation {
        Some(doc) if !doc.trim().is_empty() => {
            print_doc_with_highlighting(doc);
        }
        _ => {
            println!(
                "    {} {}",
                "│".dimmed(),
                "(no documentation available)".dimmed()
            );
        }
    }
    println!();
}
