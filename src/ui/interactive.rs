use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::analyzer;

use super::{args::Opts, display::print_method};

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
