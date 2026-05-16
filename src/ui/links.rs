//! URL building and external application launching.
//!
//! Maps definitions to standard library or `docs.rs` URLs and opens
//! them in the system browser or preferred editor.
use std::process::Command;

use crate::analyzer;

/// Opens a definition in the user's preferred editor (`$EDITOR`, falling back to `$VISUAL`).
///
/// Passes line-jump arguments appropriate to the detected editor:
/// - `hx`/`helix`: `path:line`
/// - `code`/`code-insiders`: `--goto path:line`
/// - everything else: `+line path`
///
/// # Errors
///
/// Returns `Err` if neither env var is set or the editor fails to launch.
pub fn open_in_editor(def: &analyzer::Definition) -> Result<(), String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .map_err(|_| "$EDITOR and $VISUAL are not set".to_string())?;

    let path = &def.full_path;
    let line = def.line + 1;

    let status = match editor.as_str() {
        "hx" | "helix" => Command::new(&editor).arg(format!("{path}:{line}")).status(),
        "code" | "code-insiders" => Command::new(&editor)
            .args(["--goto", &format!("{path}:{line}")])
            .status(),
        _ => Command::new(&editor)
            .arg(format!("+{line}"))
            .arg(path)
            .status(),
    };

    status.map_err(|e| format!("Failed to launch {editor}: {e}"))?;
    Ok(())
}

/// Opens `url` in the system default browser (`open`, `xdg-open`, or `cmd /C start`).
///
/// # Errors
///
/// Returns `Err` if the browser command fails to start or exits non-zero.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");

    #[cfg(target_os = "linux")]
    let mut cmd = Command::new("xdg-open");

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
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

/// Builds a documentation URL for `type_name::method_name`.
///
/// Inspects `def.full_path` to choose the destination:
/// - stdlib path → `doc.rust-lang.org`
/// - Cargo registry path → `docs.rs/<crate>`
/// - fallback → `docs.rs` search query
///
/// # Examples
///
/// ```text
/// # use std::path::PathBuf;
/// # use your_crate::analyzer::Definition;
/// # use your_crate::urls::build_doc_url;
/// let def = Definition {
///     full_path: "/home/user/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/option.rs".into(),
///     line: 0,
///     // ..other fields
/// };
/// let url = build_doc_url("Option", "unwrap", &def);
/// assert_eq!(url, "https://doc.rust-lang.org/std/option/enum.Option.html#method.unwrap");
/// ```
pub fn build_doc_url(type_name: &str, method_name: &str, def: &analyzer::Definition) -> String {
    let bare_type = type_name.split('<').next().unwrap_or(type_name);
    let type_lower = bare_type.to_lowercase();

    if is_stdlib_path(&def.full_path) {
        let (base, kind) = stdlib_type_info(bare_type);
        if base == "primitive" {
            format!(
                "https://doc.rust-lang.org/std/primitive.{type_lower}.html#method.{method_name}"
            )
        } else if base.is_empty() {
            format!("https://doc.rust-lang.org/std/{kind}.{bare_type}.html#method.{method_name}")
        } else {
            format!(
                "https://doc.rust-lang.org/std/{base}/{kind}.{bare_type}.html#method.{method_name}"
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

// Checks if the file path belongs to the local Rust toolchain source code.
fn is_stdlib_path(full_path: &str) -> bool {
    full_path.contains("/library/core/")
        || full_path.contains("/library/std/")
        || full_path.contains("/library/alloc/")
}

// Maps common standard library types to their module path and item kind (struct/enum/primitive).
fn stdlib_type_info(bare_type: &str) -> (&'static str, &'static str) {
    match bare_type {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" | "bool" | "char" | "str" => ("primitive", "primitive"),
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
        _ => ("", "struct"),
    }
}

// Determines the item kind of third-party types. Defaults strictly to "struct" for now.
const fn third_party_kind(_bare_type: &str) -> &'static str {
    "struct"
}

// Attempts to extract the crate name out of a Cargo registry path.
// e.g., ".../registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.197/src/lib.rs" -> "serde"
fn cargo_crate_name(full_path: &str) -> Option<String> {
    let marker = "/registry/src/";
    let idx = full_path.find(marker)?;
    let after_registry = &full_path[idx + marker.len()..];
    let after_index = after_registry.split_once('/')?.1;
    let crate_dir = after_index.split('/').next()?;
    let name = strip_version_suffix(crate_dir);
    Some(name.replace('-', "_"))
}

// Strips semver or hash version tags from a directory name to leave just the pure crate name.
// e.g., "serde-1.0.197" -> "serde"
fn strip_version_suffix(crate_dir: &str) -> &str {
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
