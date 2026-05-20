//! User Interface (UI) and Interaction Capabilities.
//!
//! This module serves as the central hub for the application's user interface,
//! splitting concerns into CLI argument parsing, console display rendering,
//! interactive terminal menus, and external link handling.

/// Command-line argument parsing and validation.
pub mod args;

/// Terminal output rendering and text formatting utilities.
pub mod display;

/// Interactive terminal user interface (TUI) components and menus.
pub mod interactive;

/// URL construction and external browser/editor integration.
pub mod links;

/// Terminal spinner utilities for progress indication.
pub mod spinner;

// --- Re-exports ---

pub use args::{Opts, ParseResult, parse_args};
pub use display::{print_method, print_snippet};
pub use interactive::{filter_methods, run_interactive};
pub use links::{build_doc_url, open_in_browser, open_in_editor};
pub use spinner::{definition, indexing};
