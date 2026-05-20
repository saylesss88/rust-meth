use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::analyzer;

use super::{args::Opts, display::print_method};

/// Displays an interactive, fuzzy-searchable list in the terminal using `dialoguer`.
///
/// This block brings up an alternate interactive view loop directly on the standard streams.
/// If the user makes a choice, the selected method's details are printed to standard output.
/// If the user cancels the prompt (e.g., via hitting `Esc`), the function returns cleanly
/// without throwing an error or printing details.
///
/// # Errors
///
/// Returns an `Err` if the underlying terminal rendering environment fails to initialize,
/// or if capturing standard input from the host terminal encounters an I/O fault.
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

/// Applies fuzzy matching scores to a list of methods using a `SkimMatcherV2` engine.
///
/// If a filter pattern is provided, this function evaluates method names against the pattern,
/// discards non-matching instances, and returns elements sorted descending by their structural
/// match confidence score (highest quality matches first).
///
/// If `filter` is `None`, it skips the scoring engine completely and returns references to
/// all provided input methods in their original sequential order.
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;
    use analyzer::Method;

    fn make_method(name: &str) -> Method {
        Method {
            name: name.to_string(),
            detail: None,
            documentation: None,
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: filter_methods
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_filter_methods_no_filter() {
        let methods = vec![make_method("len"), make_method("push"), make_method("pop")];

        // No filter = return all
        let result = filter_methods(&methods, None);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_methods_exact_match() {
        let methods = vec![make_method("len"), make_method("push"), make_method("pop")];

        let result = filter_methods(&methods, Some("len"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "len");
    }

    #[test]
    fn test_filter_methods_fuzzy_match() {
        let methods = vec![
            make_method("wrapping_add"),
            make_method("wrapping_sub"),
            make_method("checked_add"),
            make_method("saturating_add"),
            make_method("overflowing_add"),
        ];

        // "wrap" should match all "wrapping_*" methods
        let result = filter_methods(&methods, Some("wrap"));
        assert!(result.len() >= 2);
        assert!(result.iter().any(|m| m.name == "wrapping_add"));
        assert!(result.iter().any(|m| m.name == "wrapping_sub"));
    }

    #[test]
    fn test_filter_methods_fuzzy_ordering() {
        let methods = vec![
            make_method("checked_add"),
            make_method("add"),
            make_method("wrapping_add"),
        ];

        // "add" should rank "add" highest, then others
        let result = filter_methods(&methods, Some("add"));
        assert!(!result.is_empty());
        // Exact match should be first
        assert_eq!(result[0].name, "add");
    }

    #[test]
    fn test_filter_methods_no_matches() {
        let methods = vec![make_method("len"), make_method("push")];

        let result = filter_methods(&methods, Some("xyz_nonexistent"));
        assert!(result.is_empty());
    }
}
