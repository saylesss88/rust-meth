//! Domain types and pure parsing of `rust-analyzer` LSP responses.
//!
//! Nothing in this module touches a process or a socket, it only turns
//! [`serde_json::Value`] payloads into [`Method`] / [`Definition`] values.
//! That makes it the easiest part of the analyzer to unit test, which is
//! why almost all of the test suite lives here.

use serde_json::Value;

use crate::error::{Result, RustMethError};

/// LSP `CompletionItemKind` value corresponding to a Method.
const KIND_METHOD: u64 = 2;

/// Represents a method extracted from a `rust-analyzer` completion list.
#[derive(serde::Serialize)]
pub struct Method {
    /// The plain name of the method (e.g., `"len"`).
    pub name: String,
    /// The full method signature hint provided by the LSP server (e.g., `"pub const fn len(&self) -> usize"`).
    pub detail: Option<String>,
    /// Markdown or plaintext documentation extracted from the item.
    pub documentation: Option<String>,
}

/// Contains source definition location mappings returned by an LSP `textDocument/definition` call.
#[must_use]
pub struct Definition {
    /// A shortened path string tailored for display terminals (e.g., `"library/core/src/num/uint_macros.rs"`).
    pub path: String,
    /// The unadulterated, absolute path prefix on the local filesystem.
    pub full_path: String,
    /// 0-indexed line number where the source item is declared.
    pub line: u32,
}

/// Filters, sanitizes, and deduplicates the raw JSON arrays returned by the LSP completion query.
///
/// # Errors
///
/// Returns an error if the provided JSON response does not conform to the expected LSP
/// completion shape (missing both a top-level `result` array and an `items` sub-array).
pub fn parse_methods(response: &Value) -> Result<Vec<Method>> {
    let result = &response["result"];
    let items: &[Value] = match result {
        Value::Array(arr) => arr.as_slice(),
        obj if obj["items"].is_array() => obj["items"].as_array().map_or(&[], Vec::as_slice),
        _ => return Err(RustMethError::UnexpectedResponseShape(response.to_string())),
    };
    let mut methods: Vec<Method> = Vec::with_capacity(items.len() / 2);
    for item in items {
        if item["kind"].as_u64() != Some(KIND_METHOD) {
            continue;
        }
        let name = item["label"]
            .as_str()
            .unwrap_or("")
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        methods.push(Method {
            name,
            detail: item["detail"].as_str().map(str::to_string),
            documentation: item["documentation"]["value"].as_str().map(str::to_string),
        });
    }
    methods.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    methods.dedup_by(|a, b| a.name == b.name);
    Ok(methods)
}

/// Normalizes the location array or object mapping payload returned by the LSP server into a [`Definition`].
///
/// # Panics
///
/// Panics if the line position value returned by the LSP protocol fails to map cleanly into a `u32`.
#[must_use]
pub fn parse_definition(response: &Value) -> Option<Definition> {
    let result = &response["result"];
    let location: &Value = match result {
        Value::Array(arr) if !arr.is_empty() => &arr[0],
        single if single.is_object() => single,
        _ => return None,
    };
    let uri = location["uri"].as_str().unwrap_or("");
    if uri.is_empty() {
        return None;
    }
    let line = u32::try_from(location["range"]["start"]["line"].as_u64().unwrap_or(0))
        .expect("LSP definition line should fit in u32");
    let full_path_str = uri.strip_prefix("file://").unwrap_or(uri);
    let path = full_path_str
        .find("/library/")
        .or_else(|| full_path_str.find("/src/"))
        .map_or_else(
            || full_path_str.to_string(),
            |idx| full_path_str[idx + 1..].to_string(),
        );
    let full_path = full_path_str.to_string();
    Some(Definition {
        path,
        full_path,
        line,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── parse_methods ────────────────────────────────────────────────────────
    #[test]
    fn parse_methods_empty_items_returns_empty_vec() {
        let resp = json!({ "result": { "items": [], "isIncomplete": false } });
        let methods = parse_methods(&resp).unwrap();
        assert!(methods.is_empty());
    }

    #[test]
    fn parse_methods_filters_non_method_kinds() {
        // kind 2 = Method, kind 5 = Field, kind 9 = Module
        let resp = json!({
            "result": {
                "items": [
                    { "kind": 2, "label": "len(…)" },
                    { "kind": 5, "label": "capacity" },
                    { "kind": 9, "label": "Clone" }
                ]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "len");
    }

    #[test]
    fn parse_methods_deduplicates_same_name() {
        let resp = json!({
            "result": {
                "items": [
                    { "kind": 2, "label": "clone(…)" },
                    { "kind": 2, "label": "clone(…)" }
                ]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "clone");
    }

    #[test]
    fn parse_methods_returns_sorted_names() {
        let resp = json!({
            "result": {
                "items": [
                    { "kind": 2, "label": "zip(…)" },
                    { "kind": 2, "label": "map(…)" },
                    { "kind": 2, "label": "filter(…)" }
                ]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        let names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["filter", "map", "zip"]);
    }

    #[test]
    fn parse_methods_preserves_detail_and_documentation() {
        let resp = json!({
            "result": {
                "items": [{
                    "kind": 2,
                    "label": "len(…)",
                    "detail": "pub fn len(&self) -> usize",
                    "documentation": { "value": "Returns the number of elements." }
                }]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(
            methods[0].detail.as_deref(),
            Some("pub fn len(&self) -> usize")
        );
        assert_eq!(
            methods[0].documentation.as_deref(),
            Some("Returns the number of elements.")
        );
    }

    #[test]
    fn parse_methods_no_detail_or_docs_is_none() {
        let resp = json!({
            "result": { "items": [{ "kind": 2, "label": "len(…)" }] }
        });
        let methods = parse_methods(&resp).unwrap();
        assert!(methods[0].detail.is_none());
        assert!(methods[0].documentation.is_none());
    }

    #[test]
    fn parse_methods_array_result_form() {
        // Some LSP servers return `result` as a plain array
        let resp = json!({
            "result": [
                { "kind": 2, "label": "len(…)" },
                { "kind": 2, "label": "is_empty(…)" }
            ]
        });
        let methods = parse_methods(&resp).unwrap();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn parse_methods_skips_empty_label() {
        let resp = json!({
            "result": {
                "items": [
                    { "kind": 2, "label": "" },
                    { "kind": 2, "label": "len(…)" }
                ]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "len");
    }

    #[test]
    fn parse_methods_unexpected_shape_returns_error() {
        let resp = json!({ "result": "this_is_not_valid" });
        assert!(parse_methods(&resp).is_err());
    }

    // These simulate what rust-analyzer returns for third-party crate types:
    // the label contains the full signature e.g. `"as_str(…)"`.
    #[test]
    fn parse_methods_third_party_label_stripped_at_paren() {
        let resp = json!({
            "result": {
                "items": [
                    { "kind": 2, "label": "as_str(…)", "detail": "pub fn as_str(&self) -> &str" },
                    { "kind": 2, "label": "as_object(…)" }
                ]
            }
        });
        let methods = parse_methods(&resp).unwrap();
        let names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"as_str"));
        assert!(names.contains(&"as_object"));
    }

    // ── parse_definition ─────────────────────────────────────────────────────
    #[test]
    fn parse_definition_array_form() {
        let resp = json!({
            "result": [{
                "uri": "file:///home/user/.rustup/toolchains/stable/library/core/src/num/mod.rs",
                "range": {
                    "start": { "line": 42, "character": 0 },
                    "end":   { "line": 42, "character": 10 }
                }
            }]
        });
        let def = parse_definition(&resp).unwrap();
        assert_eq!(def.line, 42);
        assert!(def.path.starts_with("library/"));
        assert!(!def.full_path.starts_with("file://"));
    }

    #[test]
    fn parse_definition_object_form() {
        let resp = json!({
            "result": {
                "uri": "file:///home/user/.rustup/toolchains/stable/library/core/src/str/mod.rs",
                "range": {
                    "start": { "line": 99, "character": 4 },
                    "end":   { "line": 99, "character": 20 }
                }
            }
        });
        let def = parse_definition(&resp).unwrap();
        assert_eq!(def.line, 99);
        assert!(def.path.starts_with("library/"));
    }

    #[test]
    fn parse_definition_null_result_returns_none() {
        let resp = json!({ "result": null });
        assert!(parse_definition(&resp).is_none());
    }

    #[test]
    fn parse_definition_empty_array_returns_none() {
        let resp = json!({ "result": [] });
        assert!(parse_definition(&resp).is_none());
    }

    #[test]
    fn parse_definition_empty_uri_returns_none() {
        let resp = json!({
            "result": [{
                "uri": "",
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }
            }]
        });
        assert!(parse_definition(&resp).is_none());
    }

    #[test]
    fn parse_definition_strips_library_prefix_from_path() {
        let resp = json!({
            "result": [{
                "uri": "file:///home/user/.rustup/toolchains/stable/library/core/src/num/mod.rs",
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }
            }]
        });
        let def = parse_definition(&resp).unwrap();
        // path should start at "library/" not "/"
        assert!(def.path.starts_with("library/"));
        assert!(!def.path.starts_with('/'));
    }

    #[test]
    fn parse_definition_src_path_fallback() {
        // A third-party crate source, has /src/ but no /library/
        let resp = json!({
            "result": [{
                "uri": "file:///home/user/myproject/src/main.rs",
                "range": { "start": { "line": 5, "character": 0 }, "end": { "line": 5, "character": 0 } }
            }]
        });
        let def = parse_definition(&resp).unwrap();
        assert!(def.path.starts_with("src/"));
        assert_eq!(def.line, 5);
    }

    #[test]
    fn parse_definition_full_path_does_not_start_with_file_scheme() {
        let resp = json!({
            "result": [{
                "uri": "file:///home/user/project/src/lib.rs",
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 0 } }
            }]
        });
        let def = parse_definition(&resp).unwrap();
        assert!(!def.full_path.starts_with("file://"));
        assert!(def.full_path.starts_with('/'));
    }
}
