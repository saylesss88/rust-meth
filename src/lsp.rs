// Minimal LSP transport: write Content-Length-framed JSON over stdin,
// read framed responses from stdout.  No async, no external LSP crate
// just `BufReader` + `serde_json`.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout};

use serde_json::{Value, json};

/// A synchronous transport layer for communicating with a Language Server Protocol (LSP) server.
pub struct LspTransport {
    /// The standard input stream used to send messages to the LSP server child process.
    pub stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl LspTransport {
    /// Creates a new `LspTransport` by taking ownership of the `stdin` and `stdout`
    /// streams from the provided child process.
    ///
    /// # Panics
    ///
    /// Panics if the child process does not have a captured `stdin` or `stdout` stream
    /// (e.g., if they weren't configured with `Stdio::piped()`).
    pub fn new(child: &mut Child) -> Self {
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        Self {
            stdin,
            reader: BufReader::new(stdout),
        }
    }

    /// Send a JSON-RPC message.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if writing to or flushing the underlying `stdin` stream fails.
    pub fn send(&mut self, msg: &Value) -> std::io::Result<()> {
        let body = msg.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
        self.stdin.flush()
    }

    /// Read the next LSP message (blocks until one arrives).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - An I/O error occurs while reading from the stdout stream.
    /// - The stream ends before a valid `Content-Length` header is parsed.
    /// - The header value cannot be parsed into a valid size.
    /// - The message body cannot be parsed as valid JSON.
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

    /// Read messages until `predicate` returns `Some(T)`, discarding everything else.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The underlying [`Self::recv`] call fails.
    /// - The `limit` number of messages is exhausted without the `predicate` returning `Some(T)`.
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

    /// Constructs an LSP `initialize` request message.
    #[must_use]
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

    /// Constructs an LSP `initialized` notification message.
    #[must_use]
    pub fn initialized() -> Value {
        json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} })
    }

    /// Constructs an LSP `textDocument/didOpen` notification message.
    #[must_use]
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

    /// Constructs an LSP `textDocument/definition` request message.
    #[must_use]
    pub fn definition(id: u64, uri: &str, line: u32, character: u32) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        })
    }

    /// Constructs an LSP `textDocument/completion` request message.
    #[must_use]
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

    /// Constructs an LSP `shutdown` request message.
    #[must_use]
    pub fn shutdown(id: u64) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null })
    }

    /// Constructs an LSP `exit` notification message.
    #[must_use]
    pub fn exit() -> Value {
        json!({ "jsonrpc": "2.0", "method": "exit", "params": null })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // UNIT TESTS: LSP Message Construction
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_initialize_message_structure() {
        let msg = LspTransport::initialize(12345, "file:///test");

        assert_eq!(msg["jsonrpc"], "2.0");
        assert_eq!(msg["id"], 1);
        assert_eq!(msg["method"], "initialize");
        assert_eq!(msg["params"]["processId"], 12345);
        assert_eq!(msg["params"]["rootUri"], "file:///test");
    }

    #[test]
    fn test_completion_message_structure() {
        let msg = LspTransport::completion(42, "file:///test.rs", 10, 5);

        assert_eq!(msg["jsonrpc"], "2.0");
        assert_eq!(msg["id"], 42);
        assert_eq!(msg["method"], "textDocument/completion");
        assert_eq!(msg["params"]["position"]["line"], 10);
        assert_eq!(msg["params"]["position"]["character"], 5);
        assert_eq!(msg["params"]["context"]["triggerKind"], 2);
        assert_eq!(msg["params"]["context"]["triggerCharacter"], ".");
    }

    #[test]
    fn test_definition_message_structure() {
        let msg = LspTransport::definition(99, "file:///main.rs", 20, 15);

        assert_eq!(msg["jsonrpc"], "2.0");
        assert_eq!(msg["id"], 99);
        assert_eq!(msg["method"], "textDocument/definition");
        assert_eq!(msg["params"]["textDocument"]["uri"], "file:///main.rs");
        assert_eq!(msg["params"]["position"]["line"], 20);
        assert_eq!(msg["params"]["position"]["character"], 15);
    }

    #[test]
    fn test_shutdown_and_exit() {
        let shutdown = LspTransport::shutdown(100);
        assert_eq!(shutdown["method"], "shutdown");
        assert_eq!(shutdown["id"], 100);

        let exit = LspTransport::exit();
        assert_eq!(exit["method"], "exit");
        assert!(exit["id"].is_null());
    }
}
