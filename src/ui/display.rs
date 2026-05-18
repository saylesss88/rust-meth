use rust_meth::analyzer;

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

/// Formats a method signature into a call snippet.
/// e.g. detail = "pub fn checked_add(self, rhs: u8) -> Option<u8>"
/// outputs:
///   checked_add(self, rhs: u8) -> Option<u8>
///   → x.checked_add(rhs)
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
