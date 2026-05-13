// Orchestrates the full LSP session:
//   1. Spawn rust-analyzer
//   2. initialize / initialized handshake
//   3. textDocument/didOpen
//   4. Wait for indexing to complete ($/progress kind=end)
//   5. textDocument/completion
//   6. Extract Method items from the response
//   7. shutdown / exit

use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::lsp::LspTransport;
use crate::probe::Probe;

/// LSP CompletionItemKind for Method is 2.
const KIND_METHOD: u64 = 2;

pub struct MethodInfo {
    pub name: String,
    pub detail: Option<String>, // e.g. "fn len(&self) -> usize"
}

/// Find `rust-analyzer` on PATH, or in the active rustup toolchain bin dir.
pub fn find_rust_analyzer() -> anyhow::Result<std::path::PathBuf> {
    // Prefer whatever is on PATH first.
    if let Ok(path) = which("rust-analyzer") {
        return Ok(path);
    }

    // Fall back to `rustup which rust-analyzer` if rustup is available.
    if let Ok(out) = Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(s.into());
            }
        }
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
pub fn query_methods(
    type_name: &str,
    ra_path: &std::path::Path,
) -> anyhow::Result<Vec<MethodInfo>> {
    let probe = Probe::new(type_name)?;

    let mut child = Command::new(ra_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // suppress RA's own logging
        .spawn()?;

    let mut lsp = LspTransport::new(&mut child);
    let pid = std::process::id();

    // ── 1. initialize ────────────────────────────────────────────────────────
    lsp.send(&LspTransport::initialize(pid, &probe.root_uri()))?;
    // Wait for the initialize response (id=1).
    lsp.recv_until(20, |msg| {
        (msg["id"] == 1 && msg["result"].is_object()).then_some(())
    })?;

    // ── 2. initialized notification ──────────────────────────────────────────
    lsp.send(&LspTransport::initialized())?;

    // ── 3. didOpen ───────────────────────────────────────────────────────────
    lsp.send(&LspTransport::did_open(&probe.src_uri(), &probe.source()))?;

    // ── 4. Wait for RA to finish indexing ────────────────────────────────────
    // RA sends $/progress notifications. We wait for any progress token to
    // reach kind="end", or bail out after a generous message budget.
    eprintln!("Waiting for rust-analyzer to index… (this may take a moment on first run)");
    wait_for_indexing(&mut lsp)?;

    // ── 5. completion ─────────────────────────────────────────────────────────
    lsp.send(&LspTransport::completion(
        2,
        &probe.src_uri(),
        probe.dot_line,
        probe.dot_col,
    ))?;

    let completion_response = lsp.recv_until(50, |msg| (msg["id"] == 2).then(|| msg.clone()))?;

    // ── 6. shutdown / exit ────────────────────────────────────────────────────
    lsp.send(&LspTransport::shutdown(3))?;
    let _ = lsp.recv_until(10, |msg| (msg["id"] == 3).then_some(()));
    lsp.send(&LspTransport::exit())?;
    let _ = child.wait();

    // ── 7. Parse completion items ─────────────────────────────────────────────
    parse_methods(completion_response)
}

/// Block until we see a $/progress with value.kind == "end", indicating RA
/// has finished its initial indexing pass.  We give up after 200 messages
/// (typically takes 5-30 on a warm project).
fn wait_for_indexing(lsp: &mut LspTransport) -> anyhow::Result<()> {
    lsp.recv_until(200, |msg| {
        if msg["method"] == "$/progress" {
            let kind = &msg["params"]["value"]["kind"];
            if kind == "end" {
                return Some(());
            }
        }
        // Also accept if RA sends a "rust-analyzer/ready" notification.
        if msg["method"] == "experimental/serverStatus" {
            if msg["params"]["quiescent"] == true {
                return Some(());
            }
        }
        None
    })
    // If we never see the signal, proceed anyway — completion may still work.
    .or(Ok(()))
}

fn parse_methods(response: Value) -> anyhow::Result<Vec<MethodInfo>> {
    // The result is either a CompletionList { items: [...] } or an array directly.
    let items = match &response["result"] {
        Value::Array(arr) => arr.clone(),
        obj if obj["items"].is_array() => obj["items"].as_array().cloned().unwrap_or_default(),
        _ => anyhow::bail!("Unexpected completion response shape: {response}"),
    };

    let mut methods: Vec<MethodInfo> = items
        .iter()
        .filter(|item| item["kind"].as_u64() == Some(KIND_METHOD))
        .map(|item| MethodInfo {
            name: item["label"]
                .as_str()
                .unwrap_or("")
                .split('(') // labels often include sig: "len()"
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            detail: item["detail"].as_str().map(str::to_string),
        })
        .filter(|m| !m.name.is_empty())
        .collect();

    methods.sort_by(|a, b| a.name.cmp(&b.name));
    methods.dedup_by(|a, b| a.name == b.name);

    Ok(methods)
}
