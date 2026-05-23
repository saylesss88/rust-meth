//! Terminal spinner utilities for providing user feedback during long operations.

use indicatif::{ProgressBar, ProgressStyle};

/// Creates and starts a new spinner with a custom message.
///
/// The spinner automatically disappears when dropped, so wrap it in a scope
/// or explicitly call `finish()` when the operation completes.
///
/// # Panics
///
/// Panics if the internal hardcoded formatting template string fails to parse.
/// Because this template layout is immutable and validated during development tests,
/// this operation is considered safe under normal runtime conditions.
///
/// # Examples
///
/// ```no_run
/// use rust_meth_lib::ui::spinner;
///
/// let spinner = spinner::start("Loading metadata...");
/// // ... perform action ...
/// spinner.finish_with_message("✓ Done!");
///
#[must_use]
pub fn start(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();

    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .expect("Invalid template"),
    );

    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    spinner
}

/// Creates and starts a tailored spinner representing a `rust-analyzer` source indexing query.
///
/// Informatively tracks latency while fetching structural implementation blocks matching `type_name`.
///
/// # Examples
///
/// ```no_run
/// use rust_meth_lib::ui::spinner;
///
/// let progress = spinner::indexing("std::collections::HashMap");
/// // ... raw query logic ...
/// progress.finish_and_clear();
///
#[must_use]
pub fn indexing(type_name: &str) -> ProgressBar {
    start(&format!(
        "Querying rust-analyzer for methods on `{type_name}`..."
    ))
}

/// Creates and starts a tailored spinner representing a definition lookup query.
///
/// Informatively tracks latency while resolving file pathways and source offsets matching
/// `type_name` and `method_name`.
///
/// # Examples
///
/// ```no_run
/// use rust_meth_lib::ui::spinner;
///
/// let progress = spinner::definition("Vec", "push");
/// // ... definition trace ...
/// progress.finish_with_message("✓ Found source link");
///
#[must_use]
pub fn definition(type_name: &str, method_name: &str) -> ProgressBar {
    start(&format!(
        "Finding definition of `{type_name}::{method_name}`..."
    ))
}
