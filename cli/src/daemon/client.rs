//! Client-side connection to a running `rust-meth` daemon.
//!
//! Used by the CLI query path to transparently delegate to the daemon when
//! it is running. If the daemon socket does not exist or the connection fails,
//! the caller falls back to the standard LSP session path without error.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// Protocol types (minimal subset needed by the client)

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum DaemonCommand {
    Query(QueryRequest),
    Status,
    Stop,
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    type_name: String,
    deps: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum DaemonResponse {
    QueryResult(QueryResponse),
    Status(StatusResponse),
    Stopped,
    Error { message: String },
}

#[derive(Debug, Deserialize)]
pub struct QueryResponse {
    pub methods: Vec<MethodData>,
    pub session_reused: bool,
    pub from_results_cache: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct MethodData {
    pub name: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    active_sessions: usize,
    uptime_secs: u64,
    pid: u32,
}

// Socket path

/// Returns the path to the daemon Unix socket.
#[must_use]
pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(
            || {
                std::env::var_os("HOME")
                    .map_or_else(std::env::temp_dir, PathBuf::from)
                    .join(".cache")
            },
            PathBuf::from,
        )
        .join("rust-meth")
        .join("daemon.sock")
}

/// Returns true if the daemon socket exists on disk.
#[must_use]
pub fn daemon_running() -> bool {
    socket_path().exists()
}

// Client

/// Attempts to query methods via the daemon socket.
///
/// Returns `None` if the daemon is not running or the connection fails —
/// the caller should fall back to the standard LSP session path.
/// Returns `Some(Err(...))` only for protocol-level errors where the daemon
/// responded but reported a failure.
pub fn try_query(type_name: &str, deps: Option<&str>) -> Option<Result<QueryResponse, String>> {
    if !daemon_running() {
        return None;
    }

    let cmd = DaemonCommand::Query(QueryRequest {
        type_name: type_name.to_string(),
        deps: deps.map(str::to_string),
    });

    match send_command(&cmd) {
        Ok(DaemonResponse::QueryResult(resp)) => Some(Ok(resp)),
        Ok(DaemonResponse::Error { message }) => Some(Err(message)),
        Ok(_) | Err(_) => None,
    }
}

/// Sends a stop command to the daemon.
///
/// Returns `true` if the daemon acknowledged the stop.
pub fn try_stop() -> bool {
    matches!(
        send_command(&DaemonCommand::Stop),
        Ok(DaemonResponse::Stopped)
    )
}

/// Queries the daemon for its current status.
///
/// Returns `None` if the daemon is not running.
pub fn try_status() -> Option<DaemonStatusInfo> {
    if !daemon_running() {
        return None;
    }
    match send_command(&DaemonCommand::Status) {
        Ok(DaemonResponse::Status(s)) => Some(DaemonStatusInfo {
            active_sessions: s.active_sessions,
            uptime_secs: s.uptime_secs,
            pid: s.pid,
        }),
        _ => None,
    }
}

/// Public status info returned to callers.
#[derive(Debug)]
pub struct DaemonStatusInfo {
    pub active_sessions: usize,
    pub uptime_secs: u64,
    pub pid: u32,
}

// Internal

fn send_command(command: &DaemonCommand) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(socket_path())?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut writer = stream.try_clone()?;
    let reader = BufReader::new(stream);

    let mut json = serde_json::to_string(&command)?;
    json.push('\n');
    writer.write_all(json.as_bytes())?;

    let mut line = String::new();
    let mut reader = reader;
    reader.read_line(&mut line)?;

    let response: DaemonResponse = serde_json::from_str(line.trim())?;
    Ok(response)
}
