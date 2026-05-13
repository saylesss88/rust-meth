// Minimal LSP transport: write Content-Length-framed JSON over stdin,
// read framed responses from stdout.  No async, no external LSP crate —
// just BufReader + serde_json.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout};

use serde_json::{json, Value};

pub struct LspTransport {
    pub stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl LspTransport {
    pub fn new(child: &mut Child) -> Self {
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        Self {
            stdin,
            reader: BufReader::new(stdout),
        }
    }

    /// Send a JSON-RPC message.
    pub fn send(&mut self, msg: &Value) -> std::io::Result<()> {
        let body = msg.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
        self.stdin.flush()
    }

    /// Read the next LSP message (blocks until one arrives).
    pub fn recv(&mut self) -> anyhow::Result<Value> {
        // Read headers until blank line.
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(val) = line.strip_prefix("Content-Length: ") {
                content_length = Some(val.trim().parse()?);
            }
        }

        let length = content_length.ok_or_else(|| anyhow::anyhow!("No Content-Length header"))?;
        let mut body = vec![0u8; length];
        self.reader.read_exact(&mut body)?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// Read messages until `predicate` returns Some(T), discarding everything else.
    /// Returns an error if more than `limit` messages are consumed without a match.
    pub fn recv_until<T>(
        &mut self,
        limit: usize,
        mut predicate: impl FnMut(&Value) -> Option<T>,
    ) -> anyhow::Result<T> {
        for _ in 0..limit {
            let msg = self.recv()?;
            if let Some(result) = predicate(&msg) {
                return Ok(result);
            }
        }
        anyhow::bail!("recv_until: exhausted {limit} messages without a match")
    }

    // ── Convenience constructors for common messages ─────────────────────────

    pub fn initialize(process_id: u32, root_uri: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": process_id,
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "completion": {
                            "completionItem": { "snippetSupport": false }
                        }
                    }
                },
                "initializationOptions": {
                    // Ask RA not to load proc-macros — speeds up cold indexing.
                    "procMacro": { "enable": false }
                }
            }
        })
    }

    pub fn initialized() -> Value {
        json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} })
    }

    pub fn did_open(uri: &str, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": text
                }
            }
        })
    }

    pub fn completion(id: u64, uri: &str, line: u32, character: u32) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/completion",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "triggerKind": 2, "triggerCharacter": "." }
            }
        })
    }

    pub fn shutdown(id: u64) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null })
    }

    pub fn exit() -> Value {
        json!({ "jsonrpc": "2.0", "method": "exit", "params": null })
    }
}
