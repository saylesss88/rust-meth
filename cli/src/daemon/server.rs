use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rust_meth_lib::probe::ra_version;
use rust_meth_lib::results_cache::{load_results, save_results};

use crate::daemon::pool::SessionPool;
use crate::daemon::protocol::{
    DaemonCommand, DaemonResponse, MethodData, QueryResponse, StatusResponse,
};

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

/// Returns the path to the daemon PID file.
#[must_use]
pub fn pid_path() -> PathBuf {
    socket_path().with_file_name("daemon.pid")
}

/// Runs the daemon server loop.
///
/// Binds the Unix socket, writes the PID file, and accepts connections
/// until a `Stop` command is received.
///
/// # Errors
///
/// Returns an error if the socket cannot be bound or the PID file cannot
/// be written.
#[allow(clippy::needless_pass_by_value)]
pub fn run(pool: SessionPool, ra_path: std::path::PathBuf) -> std::io::Result<()> {
    let sock = socket_path();
    if let Some(parent) = sock.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove stale socket from a previous run.
    let _ = std::fs::remove_file(&sock);

    let listener = UnixListener::bind(&sock)?;

    // Write PID file.
    let pid_file = pid_path();
    std::fs::write(&pid_file, std::process::id().to_string())?;

    eprintln!("[daemon] listening on {}", sock.display());

    let pool = Arc::new(Mutex::new(pool));
    let ra_version = ra_version(&ra_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let pool = Arc::clone(&pool);
                let ra_version = ra_version.clone();
                let ra_path = ra_path.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &pool, &ra_version, &ra_path) {
                        eprintln!("[daemon] connection error: {e}");
                    }
                });
            }
            Err(e) => {
                eprintln!("[daemon] accept error: {e}");
            }
        }
    }

    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&pid_file);
    Ok(())
}

/// Handles a single client connection.
///
/// Reads one JSON-lines command, dispatches it, writes the response.
/// Returns `true` if the daemon should stop.
fn handle_connection(
    stream: UnixStream,
    pool: &Arc<Mutex<SessionPool>>,
    ra_version: &str,
    ra_path: &std::path::Path,
) -> std::io::Result<bool> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    let mut line = String::new();
    reader.read_line(&mut line)?;

    let line = line.trim();

    if line.is_empty() {
        return Ok(false);
    }

    let command: DaemonCommand = match serde_json::from_str(line) {
        Ok(cmd) => cmd,
        Err(e) => {
            let resp = DaemonResponse::Error {
                message: format!("failed to parse command: {e}"),
            };
            write_response(&mut writer, &resp)?;
            return Ok(false);
        }
    };

    let stop = matches!(command, DaemonCommand::Stop);
    let response = dispatch(command, pool, ra_version, ra_path);
    write_response(&mut writer, &response)?;

    Ok(stop)
}

/// Dispatches a command to the appropriate handler.
fn dispatch(
    command: DaemonCommand,
    pool: &Arc<Mutex<SessionPool>>,
    ra_version: &str,
    _ra_path: &std::path::Path,
) -> DaemonResponse {
    match command {
        DaemonCommand::Query(req) => {
            let start = std::time::Instant::now();
            let type_name = &req.type_name;
            let deps = req.deps.as_deref();

            // Results cache check
            // Check the persistent results cache before touching the pool.
            // If we have cached results, return immediately. No LSP needed.
            if let Some(methods) = load_results(type_name, deps, ra_version) {
                // let elapsed_ms = start.elapsed().as_millis() as u64;
                let elapsed_ms =
                    u64::try_from(start.elapsed().as_millis()).expect("should succeed");
                return DaemonResponse::QueryResult(QueryResponse {
                    methods: methods.into_iter().map(MethodData::from).collect(),
                    session_reused: false,
                    from_results_cache: true,
                    elapsed_ms,
                });
            }

            let result = {
                let mut pool = pool.lock().expect("pool lock poisoned");
                pool.query(type_name, deps)
            };

            match result {
                Ok((methods, session_reused)) => {
                    // Save to results cache for future process invocations.
                    save_results(type_name, deps, ra_version, &methods);

                    let elapsed_ms =
                        u64::try_from(start.elapsed().as_millis()).expect("should succeed");
                    DaemonResponse::QueryResult(QueryResponse {
                        methods: methods.into_iter().map(MethodData::from).collect(),
                        session_reused,
                        from_results_cache: false,
                        elapsed_ms,
                    })
                }
                Err(e) => DaemonResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        DaemonCommand::Status => {
            let pool = pool.lock().expect("pool lock poisoned");
            DaemonResponse::Status(StatusResponse {
                active_sessions: pool.session_count(),
                uptime_secs: pool.uptime_secs(),
                pid: std::process::id(),
            })
        }

        DaemonCommand::Stop => DaemonResponse::Stopped,
    }
}

fn write_response(writer: &mut UnixStream, response: &DaemonResponse) -> std::io::Result<()> {
    let mut json = serde_json::to_string(response)
        .unwrap_or_else(|_| r#"{"type":"Error","message":"serialization failed"}"#.to_string());
    json.push('\n');
    writer.write_all(json.as_bytes())
}
