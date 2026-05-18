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

use serde_json::Value;

use crate::lsp::LspTransport;
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
pub fn find_rust_analyzer() -> anyhow::Result<PathBuf> {
    if let Some(path) = RA_PATH_CACHE.get() {
        return Ok(path.clone());
    }
    let path = if let Ok(path) = which("rust-analyzer") {
        path
    } else if let Some(path) = rustup_rust_analyzer() {
        path
    } else {
        anyhow::bail!(
            "rust-analyzer not found.\n\
             Install it with: rustup component add rust-analyzer\n\
             or ensure it is on your PATH."
        )
    };
    Ok(RA_PATH_CACHE.get_or_init(|| path).clone())
}

#[cfg(unix)]
fn which(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let out = Command::new("which").arg(name).output()?;
    anyhow::ensure!(out.status.success(), "not found");
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
) -> anyhow::Result<Vec<Method>> {
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
    let completion_response = {
        let mut response = Value::Null;
        for attempt in 1..=10u64 {
            let req_id = attempt + 2;
            lsp.send(&LspTransport::completion(
                req_id,
                &probe.src_uri(),
                probe.dot_line,
                probe.dot_col,
            ))?;

            let msg = lsp.recv_until(50, |msg| (msg["id"] == req_id).then(|| msg.clone()))?;

            let has_items = msg["result"]["items"]
                .as_array()
                .is_some_and(|a| !a.is_empty());

            if has_items {
                response = msg;
                break;
            }

            if attempt < 10 {
                let delay = match attempt {
                    1 => 50,  // 50ms - RA might be ready immediately
                    2 => 100, // 100ms
                    3 => 200, // 200ms
                    _ => 300, // 300ms for later attempts
                };
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            // if attempt < 10 {
            //     std::thread::sleep(std::time::Duration::from_millis(500));
            // }
        }
        response
    };

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
fn wait_for_indexing(lsp: &mut LspTransport) -> anyhow::Result<()> {
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
pub fn parse_methods(response: &Value) -> anyhow::Result<Vec<Method>> {
    let result = &response["result"];
    let items: &[Value] = match result {
        Value::Array(arr) => arr.as_slice(),
        obj if obj["items"].is_array() => obj["items"].as_array().map_or(&[], Vec::as_slice),
        _ => anyhow::bail!("Unexpected completion response shape: {response}"),
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
) -> anyhow::Result<Option<Definition>> {
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
    let response = {
        let mut result = Value::Null;
        for attempt in 1..=10u64 {
            let req_id = attempt + 2;
            lsp.send(&LspTransport::definition(
                req_id,
                &probe.src_uri(),
                probe.dot_line,
                probe.dot_col,
            ))?;

            let msg = lsp.recv_until(50, |msg| (msg["id"] == req_id).then(|| msg.clone()))?;

            // -32801 = content modified, -32800 = request cancelled. Both mean retry.
            let is_error = msg["error"]["code"].as_i64().is_some();
            let is_null = msg["result"].is_null();

            if !is_error && !is_null {
                result = msg;
                break;
            }

            if attempt < 10 {
                if std::env::var("RUST_METH_DEBUG").is_ok() {
                    eprintln!("(attempt {attempt}: not ready, retrying…)");
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        result
    };

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
