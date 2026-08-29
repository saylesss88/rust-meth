//! `rust-meth-daemon`: persistent rust-analyzer session manager.
//!
//! Keeps one rust-analyzer process alive per unique dependency set,
//! serving method queries over a Unix socket without the per-query
//! spawn/handshake/indexing overhead.
//!
//! ## Usage
//!
//! ```sh
//! rust-meth-daemon start [--ttl <secs>]
//! rust-meth-daemon stop
//! rust-meth-daemon status
//! rust-meth-daemon query --type 'Vec<u8>'
//! rust-meth-daemon query --type 'serde_json::Value' --deps 'serde_json = "1.0"'
//! ```

mod client;
mod pool;
mod protocol;
mod server;
mod session;
mod workspace;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use pool::{DEFAULT_TTL_SECS, SessionPool};
use protocol::{DaemonCommand, DaemonResponse, QueryRequest};
use server::{pid_path, socket_path};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("help");

    let result = match subcommand {
        "start" => cmd_start(&args[2..]),
        "stop" => cmd_stop(),
        "status" => cmd_status(),
        "query" => cmd_query(&args[2..]),
        _ => {
            print_help();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

// start

fn cmd_start(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Parse --ttl <secs>
    let ttl = parse_flag(args, "--ttl")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TTL_SECS);

    // Check if already running.
    if socket_path().exists() {
        if let Ok(resp) = send_command(DaemonCommand::Status)
            && let DaemonResponse::Status(s) = resp
        {
            eprintln!(
                "daemon already running (pid {}, {} sessions)",
                s.pid, s.active_sessions
            );
            return Ok(());
        }

        // Socket exists but daemon isn't responding — clean up.
        let _ = std::fs::remove_file(socket_path());
    }

    let ra_path = rust_meth_lib::analyzer::find_rust_analyzer()?;
    eprintln!(
        "[daemon] starting with TTL={ttl}s, ra={}",
        ra_path.display()
    );

    let pool = SessionPool::new(ra_path.clone(), ttl);
    server::run(pool, ra_path)?;
    Ok(())
}

// stop

fn cmd_stop() -> Result<(), Box<dyn std::error::Error>> {
    match send_command(DaemonCommand::Stop)? {
        DaemonResponse::Stopped => {
            println!("daemon stopped.");
            // Clean up PID file if it exists.
            let _ = std::fs::remove_file(pid_path());
        }
        DaemonResponse::Error { message } => {
            eprintln!("error stopping daemon: {message}");
        }
        _ => eprintln!("unexpected response from daemon"),
    }
    Ok(())
}

// status

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    if !socket_path().exists() {
        println!("daemon: not running");
        return Ok(());
    }

    match send_command(DaemonCommand::Status)? {
        DaemonResponse::Status(s) => {
            println!("daemon: running");
            println!("  pid:      {}", s.pid);
            println!("  uptime:   {}s", s.uptime_secs);
            println!("  sessions: {}", s.active_sessions);
        }
        DaemonResponse::Error { message } => {
            eprintln!("error: {message}");
        }
        _ => eprintln!("unexpected response"),
    }
    Ok(())
}

//  query

fn cmd_query(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let type_name = parse_flag(args, "--type").ok_or("--type is required")?;
    let deps = parse_flag(args, "--deps");

    if !socket_path().exists() {
        return Err("daemon is not running. Start it with: rust-meth-daemon start".into());
    }

    let cmd = DaemonCommand::Query(QueryRequest {
        type_name: type_name.to_string(),
        deps: deps.map(str::to_string),
    });

    match send_command(cmd)? {
        DaemonResponse::QueryResult(result) => {
            let source = if result.from_results_cache {
                "results cache"
            } else if result.session_reused {
                "warm session"
            } else {
                "new session"
            };
            println!(
                "{} methods on `{type_name}` ({}ms, {source}):",
                result.methods.len(),
                result.elapsed_ms,
            );
            for m in &result.methods {
                if let Some(sig) = &m.detail {
                    println!("  {}  →  {sig}", m.name);
                } else {
                    println!("  {}", m.name);
                }
            }
        }
        DaemonResponse::Error { message } => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
        _ => eprintln!("unexpected response"),
    }
    Ok(())
}

// IPC helpers

/// Sends a command to the daemon and returns the response.
fn send_command(command: DaemonCommand) -> Result<DaemonResponse, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path())?;
    let mut json = serde_json::to_string(&command)?;
    json.push('\n');
    stream.write_all(json.as_bytes())?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let response: DaemonResponse = serde_json::from_str(line.trim())?;
    Ok(response)
}

/// Parses `--flag value` from an args slice.
fn parse_flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

fn print_help() {
    eprintln!(
        "rust-meth-daemon — persistent rust-analyzer session manager
 
USAGE:
    rust-meth-daemon start [--ttl <secs>]   Start the daemon (default TTL: 600s)
    rust-meth-daemon stop                   Stop the daemon
    rust-meth-daemon status                 Show daemon status
    rust-meth-daemon query --type <TYPE> [--deps <TOML>]
                                            Query methods via the daemon
 
EXAMPLES:
    rust-meth-daemon start
    rust-meth-daemon query --type 'Vec<u8>'
    rust-meth-daemon query --type 'serde_json::Value' --deps 'serde_json = \"1.0\"'
    rust-meth-daemon stop
"
    );
}
