use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use rust_meth::analyzer;

use super::{args::Opts, display::print_method};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Method;

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
