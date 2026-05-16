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
        // Check if this part looks like a version component:
        // - Starts with a digit (1, 2, 3...)
        // - Is a pre-release identifier (alpha, beta, rc)
        // - Is just digits and dots (1.0, 2.1.3)
        if part.chars().next().is_some_and(|c| c.is_ascii_digit())
            || matches!(*part, "alpha" | "beta" | "rc" | "dev" | "pre")
            || part.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
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
// fn strip_version_suffix(crate_dir: &str) -> &str {
//     let parts: Vec<&str> = crate_dir.rsplitn(10, '-').collect();
//     let mut drop = 0;

//     for part in &parts {
//         if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
//             drop += 1;
//         } else {
//             break;
//         }
//     }

//     if drop == 0 {
//         return crate_dir;
//     }

//     let total_len: usize = parts[drop..].iter().map(|s| s.len() + 1).sum();
//     &crate_dir[..total_len.saturating_sub(1)]
// }

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: strip_version_suffix
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_strip_version_suffix_with_version() {
        // Test typical semver patterns
        assert_eq!(strip_version_suffix("serde-1.0.197"), "serde");
        assert_eq!(strip_version_suffix("tokio-1.35.1"), "tokio");
        assert_eq!(strip_version_suffix("clap-4.5.0"), "clap");
    }

    #[test]
    fn test_strip_version_suffix_no_version() {
        // When there's no version, return as-is
        assert_eq!(strip_version_suffix("my-crate"), "my-crate");
        assert_eq!(strip_version_suffix("serde"), "serde");
    }

    #[test]
    fn test_strip_version_suffix_multi_part_name() {
        // Crates with hyphens in the name
        assert_eq!(strip_version_suffix("rust-analyzer-1.2.3"), "rust-analyzer");
        assert_eq!(strip_version_suffix("serde-json-1.0.0"), "serde-json");
    }

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: rust-meth
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_cargo_crate_name_typical_path() {
        let path = "/home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.197/src/lib.rs";
        assert_eq!(cargo_crate_name(path), Some("serde".to_string()));
    }

    #[test]
    fn test_cargo_crate_name_with_hyphens() {
        let path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/serde-json-1.0.115/src/lib.rs";
        assert_eq!(cargo_crate_name(path), Some("serde_json".to_string()));
    }

    #[test]
    fn test_cargo_crate_name_invalid_path() {
        // Not a cargo registry path
        assert_eq!(cargo_crate_name("/home/user/src/main.rs"), None);
        assert_eq!(cargo_crate_name(""), None);
    }

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: stdlib_type_info
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_stdlib_type_info_primitives() {
        assert_eq!(stdlib_type_info("u8"), ("primitive", "primitive"));
        assert_eq!(stdlib_type_info("i32"), ("primitive", "primitive"));
        assert_eq!(stdlib_type_info("f64"), ("primitive", "primitive"));
        assert_eq!(stdlib_type_info("bool"), ("primitive", "primitive"));
        assert_eq!(stdlib_type_info("char"), ("primitive", "primitive"));
        assert_eq!(stdlib_type_info("str"), ("primitive", "primitive"));
    }

    #[test]
    fn test_stdlib_type_info_common_types() {
        assert_eq!(stdlib_type_info("String"), ("string", "struct"));
        assert_eq!(stdlib_type_info("Vec"), ("vec", "struct"));
        assert_eq!(stdlib_type_info("Box"), ("boxed", "struct"));
        assert_eq!(stdlib_type_info("Option"), ("option", "enum"));
        assert_eq!(stdlib_type_info("Result"), ("result", "enum"));
    }

    #[test]
    fn test_stdlib_type_info_collections() {
        assert_eq!(
            stdlib_type_info("HashMap"),
            ("collections/hash_map", "struct")
        );
        assert_eq!(
            stdlib_type_info("BTreeSet"),
            ("collections/btree_set", "struct")
        );
    }

    #[test]
    fn test_stdlib_type_info_unknown() {
        // Unknown types default to ("", "struct")
        assert_eq!(stdlib_type_info("MyCustomType"), ("", "struct"));
    }

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: is_stdlib_path
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_is_stdlib_path_core() {
        assert!(is_stdlib_path(
            "/rustup/toolchains/stable/lib/rustlib/src/rust/library/core/src/option.rs"
        ));
    }

    #[test]
    fn test_is_stdlib_path_std() {
        assert!(is_stdlib_path(
            "/rustup/toolchains/stable/lib/rustlib/src/rust/library/std/src/vec.rs"
        ));
    }

    #[test]
    fn test_is_stdlib_path_alloc() {
        assert!(is_stdlib_path(
            "/rustup/toolchains/stable/lib/rustlib/src/rust/library/alloc/src/string.rs"
        ));
    }

    #[test]
    fn test_is_stdlib_path_cargo_crate() {
        assert!(!is_stdlib_path(
            "/home/user/.cargo/registry/src/serde-1.0.0/src/lib.rs"
        ));
    }

    #[test]
    fn test_is_stdlib_path_user_code() {
        assert!(!is_stdlib_path("/home/user/projects/my-app/src/main.rs"));
    }

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: build_doc_url
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_build_doc_url_stdlib_primitive() {
        let def = crate::analyzer::Definition {
            full_path: "/rustup/toolchains/stable/lib/rustlib/src/rust/library/core/src/num/mod.rs"
                .to_string(),
            path: "library/core/src/num/mod.rs".to_string(),
            line: 0,
        };

        let url = build_doc_url("u8", "wrapping_add", &def);
        assert_eq!(
            url,
            "https://doc.rust-lang.org/std/primitive.u8.html#method.wrapping_add"
        );
    }

    #[test]
    fn test_build_doc_url_stdlib_string() {
        let def = crate::analyzer::Definition {
            full_path: "/rustup/toolchains/stable/lib/rustlib/src/rust/library/alloc/src/string.rs"
                .to_string(),
            path: "library/alloc/src/string.rs".to_string(),
            line: 0,
        };

        let url = build_doc_url("String", "len", &def);
        assert_eq!(
            url,
            "https://doc.rust-lang.org/std/string/struct.String.html#method.len"
        );
    }

    #[test]
    fn test_build_doc_url_stdlib_option() {
        let def = crate::analyzer::Definition {
            full_path: "/rustup/toolchains/stable/lib/rustlib/src/rust/library/core/src/option.rs"
                .to_string(),
            path: "library/core/src/option.rs".to_string(),
            line: 0,
        };

        let url = build_doc_url("Option", "unwrap", &def);
        assert_eq!(
            url,
            "https://doc.rust-lang.org/std/option/enum.Option.html#method.unwrap"
        );
    }

    #[test]
    fn test_build_doc_url_third_party() {
        let def = crate::analyzer::Definition {
            full_path:
                "/home/user/.cargo/registry/src/index.crates.io-xxx/serde-1.0.197/src/lib.rs"
                    .to_string(),
            path: "serde-1.0.197/src/lib.rs".to_string(),
            line: 0,
        };

        let url = build_doc_url("Deserialize", "deserialize", &def);
        assert_eq!(
            url,
            "https://docs.rs/serde/latest/serde/struct.Deserialize.html#method.deserialize"
        );
    }

    #[test]
    fn test_build_doc_url_generic_type() {
        let def = crate::analyzer::Definition {
            full_path:
                "/rustup/toolchains/stable/lib/rustlib/src/rust/library/alloc/src/vec/mod.rs"
                    .to_string(),
            path: "library/alloc/src/vec/mod.rs".to_string(),
            line: 0,
        };

        // Should strip generics: Vec<i32> -> Vec
        let url = build_doc_url("Vec<i32>", "push", &def);
        assert_eq!(
            url,
            "https://doc.rust-lang.org/std/vec/struct.Vec.html#method.push"
        );
    }
}
