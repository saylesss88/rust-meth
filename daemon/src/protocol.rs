use serde::{Deserialize, Serialize};

// -- Requests --

/// A command sent from the client to the daemon
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonCommand {
    /// Query methods available on a Rust type
    Query(QueryResult),
    /// Shut the daemon down gracefully.
    Stop,
    /// Request current daemon status.
    Status,
}

/// Parameters for a method query
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    /// The Rust type expression to query (e.g. `"Vec<u8>"`, `"serde_json::Value"`).
    pub type_name: String,
    /// Optional TOML dependency string (e.g. `r#"serde_json = "1.0""#`).
    /// Multiple crates are newline-separated.
    pub deps: Option<String>,
}

// -- Responses --

/// A response sent from the daemon to the client.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonResponse {
    /// Result of a [`DaemonCommand::Query`].
    QueryResult(QueryResponse),
    /// Result of a [`DaemonCommand::Status`].
    Status(StatusResponse),
    /// Acknowledgement of a [`DaemonCommand::Stop`].
    Stopped,
    /// A fatal error occurred processing the command.
    Error { message: String },
}

/// Successful method query result.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    /// The methods available on the queried type.
    pub methods: Vec<MethodData>,
    /// Whether an existing rust-analyzer session was reused.
    pub session_reused: bool,
    /// Whether results came from the persistent results cache (no LSP involved).
    pub from_results_cache: bool,
    /// Wall time for the query in milliseconds.
    pub elapsed_ms: u64,
}

/// Serializable method data — mirrors [`rust_meth_lib::analyzer::Method`].
#[derive(Debug, Serialize, Deserialize)]
pub struct MethodData {
    /// The plain method name (e.g. `"len"`).
    pub name: String,
    /// The full signature hint from the LSP server, if available.
    pub detail: Option<String>,
    /// Rustdoc string, if available.
    pub documentation: Option<String>,
}

/// Current daemon status snapshot.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Number of active rust-analyzer sessions.
    pub active_sessions: usize,
    /// Daemon uptime in seconds.
    pub uptime_secs: u64,
    /// PID of the daemon process.
    pub pid: u32,
}

// -- Conversions --

impl From<rust_meth_lib::analyzer::Method> for MethodData {
    fn from(m: rust_meth_lib::analyzer::Method) -> Self {
        Self {
            name: m.name,
            detail: m.detail,
            documentation: m.documentation,
        }
    }
}
