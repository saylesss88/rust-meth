use std::collections::HashMap;
use std::time::Instant;

use rust_meth_lib::analyzer::Method;

use super::session::RaSession;
use super::workspace::{SessionKey, WorkspaceError, build_session_key, open_or_create};

// pub const DEFAULT_TTL_SECS: u64 = 600;

/// Error type for pool operations.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("LSP error: {0}")]
    Lsp(#[from] rust_meth_lib::error::RustMethError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Manages a pool of rust-analyzer sessions keyed by [`SessionKey`].
pub struct SessionPool {
    sessions: HashMap<SessionKey, RaSession>,
    ra_path: std::path::PathBuf,
    ttl_secs: u64,
    started_at: Instant,
}

impl SessionPool {
    /// Creates a new empty session pool.
    #[must_use]
    pub fn new(ra_path: std::path::PathBuf, ttl_secs: u64) -> Self {
        Self {
            sessions: HashMap::new(),
            ra_path,
            ttl_secs,
            started_at: Instant::now(),
        }
    }

    /// Queries methods for `type_name` using a pooled session.
    ///
    /// If a session for this dep set already exists and is healthy, it is
    /// reused. Otherwise a new session is spawned. Returns the methods and
    /// whether the session was reused.
    ///
    /// # Errors
    ///
    /// Returns an error if session creation or the LSP query fails.
    pub fn query(
        &mut self,
        type_name: &str,
        deps: Option<&str>,
    ) -> std::result::Result<(Vec<Method>, bool), PoolError> {
        // Evict expired sessions before each query.
        self.evict_expired();

        let key = build_session_key(deps, &self.ra_path)?;
        let session_reused = self.sessions.contains_key(&key);

        if !session_reused {
            let workspace = open_or_create(&key, deps, type_name)?;
            eprintln!("[daemon] spawning session...");
            let session = RaSession::spawn(&self.ra_path, workspace).map_err(PoolError::Lsp)?;
            eprintln!("[daemon] session spawned");
            self.sessions.insert(key.clone(), session);
        }

        let session = self.sessions.get_mut(&key).expect("session just inserted");
        let methods = session.query_methods(type_name).map_err(PoolError::Lsp)?;

        Ok((methods, session_reused))
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns how long the pool has been running in seconds.
    #[must_use]
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Shuts down all sessions gracefully.
    #[allow(dead_code)]
    pub fn shutdown_all(self) {
        for (_, session) in self.sessions {
            session.shutdown();
        }
    }

    /// Removes sessions that have been idle longer than the TTL.
    fn evict_expired(&mut self) {
        let ttl = self.ttl_secs;
        let expired: Vec<SessionKey> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_expired(ttl))
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            if let Some(session) = self.sessions.remove(&key) {
                session.shutdown();
                if std::env::var("RUST_METH_DEBUG").is_ok() {
                    eprintln!("[daemon] evicted expired session: {}", key.dir_name());
                }
            }
        }
    }
}
