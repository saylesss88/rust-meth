use crate::analyzer;

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
