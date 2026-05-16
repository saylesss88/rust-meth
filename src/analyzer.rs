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

use serde_json::Value;

use crate::lsp::LspTransport;
use crate::probe::Probe;

/// LSP `CompletionItemKind` for Method is 2.
const KIND_METHOD: u64 = 2;

pub struct Method {
    pub name: String,
    pub detail: Option<String>, // e.g. "fn len(&self) -> usize"
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

/// Find `rust-analyzer` on PATH, or in the active rustup toolchain bin dir.
pub fn find_rust_analyzer() -> anyhow::Result<PathBuf> {
    if let Ok(path) = which("rust-analyzer") {
        return Ok(path);
    }

    if let Some(path) = rustup_rust_analyzer() {
        return Ok(path);
    }

    anyhow::bail!(
        "rust-analyzer not found.\n\
         Install it with: rustup component add rust-analyzer\n\
         or ensure it is on your PATH."
    )
}

fn which(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let out = Command::new("which").arg(name).output()?;
    anyhow::ensure!(out.status.success(), "not found");
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(s.into())
}

/// Run a full LSP session against `type_name` and return sorted method names.
pub fn query_methods(type_name: &str, ra_path: &std::path::Path) -> anyhow::Result<Vec<Method>> {
    let probe = Probe::new(type_name)?;

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
    lsp.send(&LspTransport::did_open(&probe.src_uri(), &probe.source()))?;

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
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
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
    lsp.recv_until(200, |msg| {
        let method = msg["method"].as_str().unwrap_or("");
        if debug {
            eprintln!("[debug] {method}");
            if method == "$/progress" {
                eprintln!(
                    "        token={} kind={}",
                    msg["params"]["token"], msg["params"]["value"]["kind"]
                );
            }
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
            "workspace/diagnostic/refresh" | "textDocument/publishDiagnostics" => Some(()),
            _ => None,
        }
    })
    .or(Ok(()))
}

fn parse_methods(response: &Value) -> anyhow::Result<Vec<Method>> {
    let items = match &response["result"] {
        Value::Array(arr) => arr.clone(),
        obj if obj["items"].is_array() => obj["items"].as_array().cloned().unwrap_or_default(),
        _ => anyhow::bail!("Unexpected completion response shape: {response}"),
    };

    let mut methods: Vec<Method> = items
        .iter()
        .filter(|item| item["kind"].as_u64() == Some(KIND_METHOD))
        .map(|item| Method {
            name: item["label"]
                .as_str()
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            detail: item["detail"].as_str().map(str::to_string),
            documentation: item["documentation"]["value"].as_str().map(str::to_string),
        })
        .filter(|m| !m.name.is_empty())
        .collect();

    methods.sort_by(|a, b| a.name.cmp(&b.name));
    methods.dedup_by(|a, b| a.name == b.name);

    Ok(methods)
}

pub struct Definition {
    pub path: String,      // short display path e.g. library/core/src/num/uint_macros.rs
    pub full_path: String, // absolute path for opening in editor
    pub line: u32,
}

/// Query go-to-definition for a specific method on a type.
pub fn query_definition(
    type_name: &str,
    method_name: &str,
    ra_path: &std::path::Path,
) -> anyhow::Result<Option<Definition>> {
    let probe = Probe::for_definition(type_name, method_name)?;

    let mut child = Command::new(ra_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut lsp = LspTransport::new(&mut child);
    let pid = std::process::id();

    lsp.send(&LspTransport::initialize(pid, &probe.root_uri()))?;
    lsp.recv_until(20, |msg| {
        (msg["id"] == 1 && msg["result"].is_object()).then_some(())
    })?;

    lsp.send(&LspTransport::initialized())?;
    lsp.send(&LspTransport::did_open(&probe.src_uri(), &probe.source()))?;

    wait_for_indexing(&mut lsp)?;

    // Retry on "content modified" — RA rejects requests while it's still
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

            // -32801 = content modified, -32800 = request cancelled — both mean retry.
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

// fn parse_definition(response: &Value) -> anyhow::Result<Option<Definition>> {
//     let location = match &response["result"] {
//         Value::Array(arr) if !arr.is_empty() => arr[0].clone(),
//         single if single.is_object() => single.clone(),
//         _ => return Ok(None),
//     };

//     let uri = location["uri"].as_str().unwrap_or("").to_string();
//     let line = location["range"]["start"]["line"].as_u64().unwrap_or(0) as u32;

//     if uri.is_empty() {
//         return Ok(None);
//     }

//     let full_path = uri.strip_prefix("file://").unwrap_or(&uri).to_string();

//     let path = if let Some(idx) = full_path.find("/library/") {
//         full_path[idx + 1..].to_string()
//     } else if let Some(idx) = full_path.find("/src/") {
//         full_path[idx + 1..].to_string()
//     } else {
//         full_path.clone()
//     };

//     Ok(Some(Definition {
//         path,
//         full_path,
//         line,
//     }))
// }
//
fn parse_definition(response: &Value) -> Option<Definition> {
    let location = match &response["result"] {
        Value::Array(arr) if !arr.is_empty() => arr[0].clone(),
        single if single.is_object() => single.clone(),
        _ => return None,
    };

    let uri = location["uri"].as_str().unwrap_or("").to_string();
    let line = u32::try_from(location["range"]["start"]["line"].as_u64().unwrap_or(0))
        .expect("LSP definition line should fit in u32");

    if uri.is_empty() {
        return None;
    }

    let full_path = uri.strip_prefix("file://").unwrap_or(&uri).to_string();

    let path = full_path
        .find("/library/")
        .or_else(|| full_path.find("/src/"))
        .map_or_else(|| full_path.clone(), |idx| full_path[idx + 1..].to_string());

    Some(Definition {
        path,
        full_path,
        line,
    })
}
