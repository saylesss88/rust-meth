pub(crate) mod client;
mod pool;
mod protocol;
pub(crate) mod server;
mod session;
mod workspace;

/// Starts the daemon server loop with the given TTL.
///
/// Blocks until the daemon receives a Stop command.
///
/// # Errors
///
/// Returns an error if the socket cannot be bound or the pool fails to start.
pub fn start(ttl_secs: u64) -> std::io::Result<()> {
    use pool::SessionPool;

    let ra_path = rust_meth_lib::analyzer::find_rust_analyzer()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;

    eprintln!(
        "[daemon] starting with TTL={ttl_secs}s, ra={}",
        ra_path.display()
    );

    let pool = SessionPool::new(ra_path.clone(), ttl_secs);
    server::run(pool, ra_path)
}
