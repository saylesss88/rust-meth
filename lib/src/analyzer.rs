// Orchestrates the full LSP session:
//   1. Spawn rust-analyzer
//   2. initialize / initialized handshake
//   3. textDocument/didOpen
//   4. Wait for indexing to complete
//   5. textDocument/completion (with retry)
//   6. Extract Method items from the response
//   7. shutdown / exit

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::error::{Result, RustMethError};
use serde_json::Value;

use crate::LspTransport;
use crate::probe::Probe;

/// LSP `CompletionItemKind` value corresponding to a Method.
const KIND_METHOD: u64 = 2;

static RA_PATH_CACHE: OnceLock<PathBuf> = OnceLock::new();

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

fn rustup_rust_analyzer() -> Option<PathBuf> {
    let out = Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then(|| path.into())
}

/// Locates the `rust-analyzer` binary.
///
/// It first searches the system `PATH` env variable using the system `which` utility.
/// If missing, it attempts to fall back to the active toolchain's binary directory
/// using `rustup which rust-analyzer`.
///
/// # Errors
///
/// Returns an error if `rust-analyzer` cannot be found via either mechanism,
/// providing user-friendly instructions on how to install it.
///
pub fn find_rust_analyzer() -> Result<PathBuf> {
    if let Some(path) = RA_PATH_CACHE.get() {
        return Ok(path.clone());
    }
    let path = if let Ok(path) = which("rust-analyzer") {
        path
    } else if let Some(path) = rustup_rust_analyzer() {
        path
    } else {
        return Err(RustMethError::RustAnalyzerNotFound);
    };
    Ok(RA_PATH_CACHE.get_or_init(|| path).clone())
}

#[cfg(unix)]
fn which(name: &str) -> Result<std::path::PathBuf> {
    let out = Command::new("which").arg(name).output()?;
    if !out.status.success() {
        return Err(RustMethError::RustAnalyzerNotFound);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(s.into())
}

/// Queries `rust-analyzer` for all available methods on a given type type expression.
///
/// This spins up an ephemeral LSP session, generates a mock workspace via a [`Probe`],
/// triggers a completion request at the appropriate line/column location, and parses the results.
///
/// # Environment Variables
///
/// * `RUST_METH_DEBUG` - If set, logs raw LSP method lifecycle events to standard error.
///
/// # Errors
///
/// Returns an error if:
/// * Spawning the `rust-analyzer` subprocess fails.
/// * The LSP server communication channels break.
/// * The server returns an unexpectedly structured or malformed JSON payload.
// pub fn query_methods(type_name: &str, ra_path: &std::path::Path) -> anyhow::Result<Vec<Method>> {
//     let probe = Probe::new(type_name)?;
pub fn query_methods(
    type_name: &str,
    ra_path: &std::path::Path,
    deps: Option<&str>,
) -> Result<Vec<Method>> {
    let probe = Probe::new_with_deps(type_name, deps)?;
    let mut child = Command::new(ra_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut lsp = LspTransport::new(&mut child);
    let pid = std::process::id();

    // ── 1. initialize ────────────────────────────────────────────────────────
    lsp.send(&LspTransport::initialize(pid, &probe.root_uri()))?;
    lsp.recv_until(20, |msg| {
        (msg["id"] == 1 && msg["result"].is_object()).then_some(())
    })?;

    // ── 2. initialized notification ──────────────────────────────────────────
    lsp.send(&LspTransport::initialized())?;

    // ── 3. didOpen ───────────────────────────────────────────────────────────
    lsp.send(&LspTransport::did_open(&probe.src_uri(), &probe.source()?))?;

    // ── 4. Wait for RA to finish indexing ────────────────────────────────────
    wait_for_indexing(&mut lsp)?;

    // ── 5. completion — retry until RA returns items ──────────────────────────
    // RA may return isIncomplete+empty if it isn't fully ready yet.
    let completion_response = retry_lsp_request(
        &mut lsp,
        10,
        |req_id| LspTransport::completion(req_id, &probe.src_uri(), probe.dot_line, probe.dot_col),
        |msg| {
            msg["result"]["items"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
        },
    )?;

    // ── 6. shutdown / exit ────────────────────────────────────────────────────
    lsp.send(&LspTransport::shutdown(13))?;
    let _ = lsp.recv_until(10, |msg| (msg["id"] == 13).then_some(()));
    lsp.send(&LspTransport::exit())?;
    let _ = child.wait();

    // ── 7. Parse completion items ─────────────────────────────────────────────
    parse_methods(&completion_response)
}

/// Wait until rust-analyzer is ready to serve completions.
///
/// RA doesn't always send $/progress. On fast/warm projects it skips straight
/// to publishing diagnostics. We treat any of these as "ready":
///   - $/progress with value.kind == "end"
///   - experimental/serverStatus with quiescent == true
///   - workspace/diagnostic/refresh
///   - textDocument/publishDiagnostics
fn wait_for_indexing(lsp: &mut LspTransport) -> Result<()> {
    let debug = std::env::var("RUST_METH_DEBUG").is_ok();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(10); // Hard timeout

    lsp.recv_until(200, |msg| {
        // Timeout escape hatch
        if start.elapsed() > timeout {
            return Some(()); // Give up and try anyway
        }

        let method = msg["method"].as_str().unwrap_or("");
        if debug {
            eprintln!("[debug] {method}");
        }

        match method {
            "$/progress" => {
                if msg["params"]["value"]["kind"] == "end" {
                    Some(())
                } else {
                    None
                }
            }
            "experimental/serverStatus" => {
                if msg["params"]["quiescent"] == true {
                    Some(())
                } else {
                    None
                }
            }
            // These are strong signals that indexing is done
            "textDocument/publishDiagnostics" | "workspace/diagnostic/refresh" => Some(()),
            _ => None,
        }
    })
    .or(Ok(()))
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

fn retry_lsp_request<F, G>(
    lsp: &mut LspTransport,
    max_attempts: u64,
    mut make_request: F,
    mut is_success: G,
) -> Result<Value>
where
    F: FnMut(u64) -> Value,   // takes req_id, returns the request to send
    G: FnMut(&Value) -> bool, // returns true when response is good
{
    let mut result = Value::Null;
    for attempt in 1..=max_attempts {
        let req_id = attempt + 2;
        lsp.send(&make_request(req_id))?;
        let msg = lsp.recv_until(50, |msg| (msg["id"] == req_id).then(|| msg.clone()))?;
        if is_success(&msg) {
            result = msg;
            break;
        }
        if attempt < max_attempts {
            if std::env::var("RUST_METH_DEBUG").is_ok() {
                eprintln!("(attempt {attempt}: not ready, retrying…)");
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
    Ok(result)
}

/// Queries `rust-analyzer` for the precise upstream source file declaration layout of a specific method.
///
/// Under the hood, this sets up a mock environment containing an isolated invocation of your method,
/// queries `textDocument/definition`, and intercepts the target file location coordinates.
///
/// # Errors
///
/// Returns an error if the underlying LSP runtime breaks, or if `rust-analyzer` encounters structural errors.
/// If a method exists but has no discoverable source code location definitions, it evaluates cleanly into `Ok(None)`.
pub fn query_definition(
    type_name: &str,
    method_name: &str,
    ra_path: &std::path::Path,
    deps: Option<&str>,
) -> Result<Option<Definition>> {
    let probe = Probe::for_definition_with_deps(type_name, method_name, deps)?;

    let mut child = Command::new(ra_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut lsp = LspTransport::new(&mut child);
    let pid = std::process::id();

    // Send didOpen immediately after initialized, don't wait
    lsp.send(&LspTransport::initialize(pid, &probe.root_uri()))?;
    lsp.recv_until(20, |msg| {
        (msg["id"] == 1 && msg["result"].is_object()).then_some(())
    })?;

    // Send both notifications back-to-back (no wait needed)
    lsp.send(&LspTransport::initialized())?;
    lsp.send(&LspTransport::did_open(&probe.src_uri(), &probe.source()?))?;

    // Now wait for indexing
    wait_for_indexing(&mut lsp)?;

    // Retry on "content modified" - RA rejects requests while it's still
    // processing the file. Same pattern as the completion retry loop.
    let response = retry_lsp_request(
        &mut lsp,
        10,
        |req_id| LspTransport::definition(req_id, &probe.src_uri(), probe.dot_line, probe.dot_col),
        |msg| {
            let is_error = msg["error"]["code"].as_i64().is_some();
            let is_null = msg["result"].is_null();
            !is_error && !is_null
        },
    )?;

    lsp.send(&LspTransport::shutdown(13))?;
    let _ = lsp.recv_until(10, |msg| (msg["id"] == 13).then_some(()));
    lsp.send(&LspTransport::exit())?;
    let _ = child.wait();

    Ok(parse_definition(&response))
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
        // A third-party crate source — has /src/ but no /library/
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
