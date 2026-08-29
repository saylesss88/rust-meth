use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use rust_meth_lib::LspTransport;
use rust_meth_lib::analyzer::Method;
use rust_meth_lib::error::{Result, RustMethError};

use crate::workspace::DaemonWorkspace;

/// A live rust-analyzer session attached to a daemon workspace.
pub struct RaSession {
    child: Child,
    lsp: LspTransport,
    workspace: DaemonWorkspace,
    /// When this session last served a query, used for TTL eviction.
    pub last_used: Instant,
}

impl RaSession {
    /// Spawns a new rust-analyzer process, performs the LSP handshake,
    /// opens `lib.rs` to trigger indexing, and waits until ready.
    ///
    /// # Errors
    ///
    /// Returns an error if spawning fails, the handshake times out, or
    /// indexing produces a fatal diagnostic.
    pub fn spawn(ra_path: &Path, workspace: DaemonWorkspace) -> Result<Self> {
        let mut child = Command::new(ra_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut lsp = LspTransport::new(&mut child)?;
        let pid = std::process::id();

        lsp.send(&LspTransport::initialize(pid, &workspace.root_uri()))?;
        lsp.recv_until(20, |msg| {
            (msg["id"] == 1 && msg["result"].is_object()).then_some(())
        })?;
        lsp.send(&LspTransport::initialized())?;

        //  Open lib.rs to trigger indexing
        let lib_path = workspace.dir.join("src").join("lib.rs");
        let lib_uri = path_to_uri(&lib_path);
        let lib_source = std::fs::read_to_string(&lib_path)?;
        lsp.send(&LspTransport::did_open(&lib_uri, &lib_source))?;

        // Also open scratch.rs so RA knows it exists
        let scratch_source = workspace.scratch_source()?;
        lsp.send(&LspTransport::did_open(
            &workspace.scratch_uri(),
            &scratch_source,
        ))?;

        // ── Wait for indexing to complete ─────────────────────────────────────
        wait_for_indexing(&mut lsp);

        Ok(Self {
            child,
            lsp,
            workspace,
            last_used: Instant::now(),
        })
    }

    /// Queries methods for `type_name` by sending `textDocument/didChange`
    /// on `scratch.rs` and firing a completion request.
    ///
    /// # Errors
    ///
    /// Returns an error if the LSP session is broken or times out.
    pub fn query_methods(&mut self, type_name: &str) -> Result<Vec<Method>> {
        self.last_used = Instant::now();

        // Rewrite scratch.rs with the new type.
        self.workspace.set_type(type_name)?;
        let new_source = self.workspace.scratch_source()?;

        // textDocument/didChange
        let change = did_change(&self.workspace.scratch_uri(), &new_source);
        self.lsp.send(&change)?;

        // Wait for diagnostics to settle
        // After didChange RA re-parses scratch.rs. We wait for a
        // publishDiagnostics notification for our file before firing
        // the completion request.
        let scratch_uri = self.workspace.scratch_uri();
        let diag_msg = wait_for_scratch_diagnostics(&mut self.lsp, &scratch_uri);

        // Check for type errors in the diagnostics.
        check_diagnostics(type_name, diag_msg.as_ref())?;

        // Completion
        let completion = retry_completion(
            &mut self.lsp,
            type_name,
            &self.workspace.scratch_uri(),
            self.workspace.dot_line,
            self.workspace.dot_col,
        )?;

        rust_meth_lib::analyzer::parse_methods(&completion)
    }

    /// Sends LSP shutdown and exit, then waits for the child to exit.
    pub fn shutdown(mut self) {
        let _ = self.lsp.send(&LspTransport::shutdown(13));
        let _ = self
            .lsp
            .recv_until(10, |msg| (msg["id"] == 13).then_some(()));
        let _ = self.lsp.send(&LspTransport::exit());
        let _ = self.child.wait();
    }

    /// Returns true if this session has been idle longer than `ttl_secs`.
    #[must_use]
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        self.last_used.elapsed().as_secs() > ttl_secs
    }
}

// LSP helpers

/// Constructs a `textDocument/didChange` notification.
fn did_change(uri: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": text }]
        }
    })
}

/// Waits for rust-analyzer to finish initial indexing.
/// Accepts any of the signals RA sends when it's ready.
fn wait_for_indexing(lsp: &mut LspTransport) {
    let start = std::time::Instant::now();
    let _ = lsp.recv_until(200, |msg| {
        if start.elapsed() > std::time::Duration::from_mins(3) {
            return Some(());
        }
        let method = msg["method"].as_str().unwrap_or("");
        match method {
            "$/progress" if msg["params"]["value"]["kind"] == "end" => Some(()),
            "experimental/serverStatus" if msg["params"]["quiescent"] == true => Some(()),
            "workspace/diagnostic/refresh" => Some(()),
            _ => None,
        }
    });
}

/// Waits for a `textDocument/publishDiagnostics` for `scratch_uri` after
/// a `didChange`. Returns the diagnostic message for error checking.
fn wait_for_scratch_diagnostics(
    lsp: &mut LspTransport,
    scratch_uri: &str,
) -> Option<serde_json::Value> {
    let mut last_diag: Option<serde_json::Value> = None;
    let start = std::time::Instant::now();

    let _ = lsp.recv_until(50, |msg| {
        if start.elapsed() > std::time::Duration::from_secs(30) {
            return Some(());
        }
        let method = msg["method"].as_str().unwrap_or("");
        if method == "textDocument/publishDiagnostics"
            && msg["params"]["uri"].as_str() == Some(scratch_uri)
        {
            last_diag = Some(msg.clone());
            Some(())
        } else {
            None
        }
    });

    last_diag
}

/// Checks diagnostics for type errors, returning an appropriate error variant.
fn check_diagnostics(type_name: &str, diag_msg: Option<&serde_json::Value>) -> Result<()> {
    let Some(msg) = diag_msg else { return Ok(()) };
    let diagnostics = msg["params"]["diagnostics"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);

    for diag in diagnostics {
        if diag["severity"].as_u64() != Some(1) {
            continue;
        }
        if let Some(rendered) = diag["data"]["rendered"].as_str()
            && (rendered.contains("configured out") || rendered.contains("gated behind"))
        {
            return Err(RustMethError::FeatureGated {
                type_name: type_name.to_string(),
                message: rendered.to_string(),
            });
        }
        let message = diag["message"].as_str().unwrap_or("");
        if message.contains("cannot find type") || message.contains("not found in") {
            return Err(RustMethError::TypeNotFound {
                type_name: type_name.to_string(),
                message: message.to_string(),
            });
        }
    }
    Ok(())
}

/// Retries completion requests until RA returns non-blanket results.
fn retry_completion(
    lsp: &mut LspTransport,
    type_name: &str,
    uri: &str,
    dot_line: u32,
    dot_col: u32,
) -> Result<serde_json::Value> {
    let start = std::time::Instant::now();
    let max_attempts = 20u64;

    for attempt in 1..=max_attempts {
        let req_id = attempt + 10;
        let req = LspTransport::completion(req_id, uri, dot_line, dot_col);
        lsp.send(&req)?;
        let msg = lsp.recv_until(50, |msg| (msg["id"] == req_id).then(|| msg.clone()))?;

        let items = msg["result"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        if !items.is_empty() {
            return Ok(msg);
        }

        if attempt < max_attempts {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    Err(RustMethError::Timeout {
        type_name: type_name.to_string(),
        waited_secs: start.elapsed().as_secs(),
    })
}

fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}
