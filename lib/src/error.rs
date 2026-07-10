use thiserror::Error;

/// The primary error tracking type for operations within the library.
///
/// Encapsulates platform errors (I/O, parsing) along with specific failure modes
/// encountered during synchronization or communication with the underlying Language Server Protocol (LSP).
#[derive(Debug, Error)]
pub enum RustMethError {
    /// Errors originating from direct operating system interaction or file system operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors resulting from failed JSON serialization or deserialization routines.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Errors resulting from failed string-to-integer conversions.
    #[error("Integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Indication that an unexpected byte payload was read from the language server stream
    /// due to a missing or improperly formed `Content-Length` header field.
    #[error("No Content-Length header in LSP response")]
    NoContentLength,

    /// Raised when looking for a specific LSP message payload, but the message loop bounds
    /// were reached before a match was resolved.
    #[error("recv_until: exhausted {limit} messages without a match")]
    RecvExhausted {
        /// The maximum number of distinct messages evaluated before halting the loop.
        limit: usize,
    },

    /// Raised when `rust-analyzer` returns a payload structurally valid as JSON, but lacking
    /// the fields or nested objects required by the expected API model contract.
    #[error("Unexpected completion response shape: {0}")]
    UnexpectedResponseShape(String),

    /// Indicates that the `rust-analyzer` executable engine could not be discovered
    /// via the host path environment or native component directory.
    #[error(
        "rust-analyzer not found.\nInstall it with: rustup component add rust-analyzer\nor ensure it is on your PATH."
    )]
    RustAnalyzerNotFound,

    /// Indicates that the child process's `stdin` handle was not captured, typically because
    /// it was not spawned with `Stdio::piped()`.
    #[error("child process stdin was not captured (spawn with Stdio::piped())")]
    MissingStdin,

    /// Indicates that the child process's `stdout` handle was not captured, typically because
    /// it was not spawned with `Stdio::piped()`.
    #[error("child process stdout was not captured (spawn with Stdio::piped())")]
    MissingStdout,

    /// Raised when rust-analyzer reports a diagnostic error for the probed type,
    /// indicating the type does not exist in scope (e.g. a typo or non-existent type).
    #[error("type not found: `{type_name}` — rust-analyzer reported: {message}")]
    TypeNotFound {
        /// The type name that was queried.
        type_name: String,
        /// The diagnostic message from rust-analyzer.
        message: String,
    },
}

/// A specialized type alias for results returning [`RustMethError`].
pub type Result<T> = std::result::Result<T, RustMethError>;
