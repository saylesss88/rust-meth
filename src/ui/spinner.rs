//! Terminal spinner utilities for providing user feedback during long operations.

use indicatif::{ProgressBar, ProgressStyle};

/// Creates and starts a new spinner with a custom message.
///
/// The spinner automatically disappears when dropped, so wrap it in a scope
/// or explicitly call `finish()` when the operation completes.
///
/// # Examples
///
/// ```no_run
/// let spinner = start("Indexing project...");
/// // ... long operation ...
/// spinner.finish_with_message("✓ Indexed");
/// ```
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

/// Creates a spinner for rust-analyzer indexing operations.
pub fn indexing(type_name: &str) -> ProgressBar {
    start(&format!(
        "Querying rust-analyzer for methods on `{type_name}`..."
    ))
}

/// Creates a spinner for go-to-definition lookups.
pub fn definition(type_name: &str, method_name: &str) -> ProgressBar {
    start(&format!(
        "Finding definition of `{type_name}::{method_name}`..."
    ))
}
