//! Orchestrates `rust-analyzer` LSP sessions.
//!
//! Each public function here spins up an ephemeral `rust-analyzer` subprocess,
//! drives it through the handshake, asks one question (completion or
//! definition), and tears it back down. The two query functions are
//! intentionally similar in shape, see the module-level note below.

use std::process::{Command, Stdio};

use serde_json::Value;

use crate::LspTransport;
use crate::error::Result;
use crate::probe::Probe;

use super::parse::{self, Definition, Method};

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
    let mut lsp = LspTransport::new(&mut child)?;
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
    let diag_msg = wait_for_indexing(&mut lsp);
    check_diagnostics_for_type_error(type_name, diag_msg.as_ref())?;

    // ── 5. completion — retry until RA returns items ──────────────────────────
    // RA may return isIncomplete+empty if it isn't fully ready yet.
    let completion_response = retry_lsp_request(
        &mut lsp,
        10,
        |req_id| LspTransport::completion(req_id, &probe.src_uri(), probe.dot_line, probe.dot_col),
        |msg| {
            msg["result"]["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty() && !is_blanket_fallback(items))
        },
    )?;

    // ── 6. shutdown / exit ────────────────────────────────────────────────────
    lsp.send(&LspTransport::shutdown(13))?;
    let _ = lsp.recv_until(10, |msg| (msg["id"] == 13).then_some(()));
    lsp.send(&LspTransport::exit())?;
    let _ = child.wait();

    // ── 7. Parse completion items ─────────────────────────────────────────────
    parse::parse_methods(&completion_response)
}

/// Use diagnostics to find the error type
fn check_diagnostics_for_type_error(type_name: &str, diag_msg: Option<&Value>) -> Result<()> {
    let Some(msg) = diag_msg else { return Ok(()) };

    let diagnostics = msg["params"]["diagnostics"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);

    for diag in diagnostics {
        // severity 1 = Error in LSP
        if diag["severity"].as_u64() != Some(1) {
            continue;
        }
        let message = diag["message"].as_str().unwrap_or("");
        // RA reports unknown types as "cannot find type `X` in this scope"
        if message.contains("cannot find type") || message.contains("not found in") {
            return Err(crate::error::RustMethError::TypeNotFound {
                type_name: type_name.to_string(),
                message: message.to_string(),
            });
        }
    }
    Ok(())
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
    let mut lsp = LspTransport::new(&mut child)?;
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
    wait_for_indexing(&mut lsp);

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

    Ok(parse::parse_definition(&response))
}

/// Wait until rust-analyzer is ready to serve completions.
///
/// RA doesn't always send $/progress. On fast/warm projects it skips straight
/// to publishing diagnostics. We treat any of these as "ready":
///   - $/progress with value.kind == "end"
///   - experimental/serverStatus with quiescent == true
///   - workspace/diagnostic/refresh
///   - textDocument/publishDiagnostics
fn wait_for_indexing(lsp: &mut LspTransport) -> Option<Value> {
    let debug = std::env::var("RUST_METH_DEBUG").is_ok();
    // Accumulate the last publishDiagnostics message seen.
    // Wait untill RA signals it's truly done.
    let mut last_diag: Option<Value> = None;
    let mut done = false;

    let drain_start = std::time::Instant::now();
    let _ = lsp.recv_until(200, |msg| {
        // Timeout escape hatch
        if drain_start.elapsed() > std::time::Duration::from_secs(5) {
            return Some(());
        }
        let method = msg["method"].as_str().unwrap_or("");
        if debug {
            eprintln!("[debug] {method}");
        }
        match method {
            "$/progress" => {
                if msg["params"]["value"]["kind"] == "end" {
                    done = true;
                    Some(())
                } else {
                    None
                }
            }
            "experimental/serverStatus" => {
                if msg["params"]["quiescent"] == true {
                    done = true;
                    Some(())
                } else {
                    None
                }
            }

            "workspace/diagnostic/refresh" => {
                done = true;
                Some(())
            }
            "textDocument/publishDiagnostics" => {
                last_diag = Some(msg.clone());
                None
            }
            _ => None,
        }
    });
    let _ = done;

    last_diag
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

/// The generic blanket-impl completions RA returns for any type it can't
/// fully resolve yet (still indexing) or can't resolve at all (unknown type).
/// If completion results are *exactly* this set, treat it as "not ready" —
/// never treat it as a legitimate answer.
const BLANKET_FALLBACK_METHODS: &[&str] = &[
    "clamp",
    "clone",
    "clone_from",
    "clone_into",
    "cmp",
    "eq",
    "ge",
    "gt",
    "into",
    "le",
    "lt",
    "max",
    "min",
    "ne",
    "not",
    "partial_cmp",
    "to_owned",
    "to_string",
    "try_into",
];

fn is_blanket_fallback(items: &[Value]) -> bool {
    let names: std::collections::BTreeSet<&str> =
        items.iter().filter_map(|i| i["label"].as_str()).collect();
    let fallback: std::collections::BTreeSet<&str> =
        BLANKET_FALLBACK_METHODS.iter().copied().collect();
    !names.is_empty() && names == fallback
}
