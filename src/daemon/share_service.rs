//! Share service for managing active shares in the daemon
//!
//! This module handles the lifecycle of shares, including starting/stopping
//! the local server and tunnel processes, and persisting state to SQLite.

use crate::config::Config;
use crate::daemon::db::{Database, DatabaseError};
use crate::daemon::protocol::{ShareId, ShareInfo, ShareStatus};
use crate::parser::{ClaudeParser, SessionParser};
use crate::server::{start_server_with_source, ServerConfig, ServerHandle};
use crate::tunnel::{get_provider_with_config, TunnelError, TunnelHandle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Errors that can occur in the share service
#[derive(Debug, Error)]
pub enum ShareServiceError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Failed to parse session: {0}")]
    ParseError(String),

    #[error("Unknown tunnel provider: {0}")]
    UnknownProvider(String),

    #[error("Tunnel provider not available: {0}")]
    ProviderNotAvailable(String),

    #[error("Failed to start server: {0}")]
    ServerError(String),

    #[error("Failed to start tunnel: {0}")]
    TunnelError(#[from] TunnelError),

    #[error("Share not found: {0}")]
    ShareNotFound(ShareId),

    #[error("Failed to acquire lock")]
    LockError,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for share service operations
pub type ShareServiceResult<T> = Result<T, ShareServiceError>;

/// Running share state - holds the handles to the server and tunnel processes
struct RunningShare {
    /// Handle to the local HTTP server
    server_handle: ServerHandle,
    /// Handle to the tunnel process
    tunnel_handle: TunnelHandle,
}

/// Service for managing shares in the daemon
///
/// The ShareService is responsible for:
/// - Starting shares (server + tunnel)
/// - Stopping shares gracefully
/// - Persisting share state to SQLite
/// - Managing configuration
pub struct ShareService {
    /// Database handle for persistence
    db: Arc<Mutex<Database>>,
    /// Running shares indexed by ShareId
    running_shares: RwLock<HashMap<ShareId, RunningShare>>,
    /// Application configuration
    config: Config,
}

impl ShareService {
    /// Create a new share service with the given database and configuration
    pub fn new(db: Arc<Mutex<Database>>, config: Config) -> Self {
        Self {
            db,
            running_shares: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create a new share service with default configuration
    pub fn with_db(db: Arc<Mutex<Database>>) -> Self {
        let config = Config::load().unwrap_or_default();
        Self::new(db, config)
    }

    /// Get the ngrok token from configuration
    fn ngrok_token(&self) -> Option<&str> {
        self.config.ngrok_token.as_deref()
    }

    /// Get the effective port for the server
    fn effective_port(&self) -> u16 {
        self.config.effective_port(3000)
    }

    /// Start a new share for the given session
    ///
    /// This will:
    /// 1. Parse the session file
    /// 2. Create a ShareInfo record in "Starting" status
    /// 3. Start the local HTTP server
    /// 4. Start the tunnel to expose the server
    /// 5. Update the ShareInfo with the public URL and "Active" status
    ///
    /// Returns the ShareInfo with the public URL on success.
    pub async fn start_share(
        &self,
        session_path: PathBuf,
        provider_name: String,
    ) -> ShareServiceResult<ShareInfo> {
        info!(
            session = %session_path.display(),
            provider = %provider_name,
            "Starting share"
        );

        // Phase 1: Parse the session file
        debug!("Parsing session file");
        let parser = ClaudeParser::new();
        let session = parser
            .parse(&session_path)
            .map_err(|e| ShareServiceError::ParseError(e.to_string()))?;

        let session_name = session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Phase 2: Create initial share record
        let share_id = ShareId::new();
        let share_info = ShareInfo {
            id: share_id,
            session_path: session_path.clone(),
            session_name: session_name.clone(),
            public_url: String::new(), // Will be updated after tunnel starts
            provider_name: provider_name.clone(),
            local_port: 0, // Will be updated after server starts
            started_at: chrono::Utc::now(),
            status: ShareStatus::Starting,
        };

        // Persist initial state
        {
            let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
            db.insert_share(&share_info)?;
        }

        // Phase 3: Get tunnel provider
        debug!("Getting tunnel provider: {}", provider_name);
        let provider = get_provider_with_config(&provider_name, self.ngrok_token())
            .ok_or_else(|| ShareServiceError::UnknownProvider(provider_name.clone()))?;

        if !provider.is_available() {
            // Update status to error in database
            self.update_share_status(share_id, ShareStatus::Error)?;
            return Err(ShareServiceError::ProviderNotAvailable(provider_name));
        }

        // Phase 4: Start the local server
        debug!("Starting local server");
        let server_config = ServerConfig {
            base_port: self.effective_port(),
            open_browser: false,
        };

        let server_handle = match start_server_with_source(
            session,
            server_config,
            Some(session_path.clone()),
        )
        .await
        {
            Ok(h) => {
                info!(port = h.port(), "Local server started");
                h
            }
            Err(e) => {
                error!("Failed to start server: {}", e);
                self.update_share_status(share_id, ShareStatus::Error)?;
                return Err(ShareServiceError::ServerError(e.to_string()));
            }
        };

        let local_port = server_handle.port();

        // Phase 5: Start the tunnel
        debug!("Spawning tunnel");
        let tunnel_handle = match provider.spawn(local_port) {
            Ok(h) => {
                info!(url = %h.url(), "Tunnel started");
                h
            }
            Err(e) => {
                error!("Failed to start tunnel: {}", e);
                // Clean up server
                server_handle.stop().await;
                self.update_share_status(share_id, ShareStatus::Error)?;
                return Err(ShareServiceError::TunnelError(e));
            }
        };

        let public_url = tunnel_handle.url().to_string();

        // Phase 6: Update database with final state
        {
            let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
            db.update_share_url(share_id, &public_url)?;
            db.update_share_status(share_id, ShareStatus::Active)?;
        }

        // Phase 7: Store running share handles
        let running_share = RunningShare {
            server_handle,
            tunnel_handle,
        };

        {
            let mut running = self.running_shares.write().await;
            running.insert(share_id, running_share);
        }

        // Return the final share info
        let final_info = ShareInfo {
            id: share_id,
            session_path,
            session_name,
            public_url,
            provider_name,
            local_port,
            started_at: share_info.started_at,
            status: ShareStatus::Active,
        };

        info!(share_id = %share_id, url = %final_info.public_url, "Share started successfully");
        Ok(final_info)
    }

    /// Stop an existing share
    ///
    /// This will:
    /// 1. Stop the tunnel process
    /// 2. Stop the local server
    /// 3. Update the share status in the database to "Stopped"
    pub async fn stop_share(&self, share_id: ShareId) -> ShareServiceResult<()> {
        info!(share_id = %share_id, "Stopping share");

        // Get and remove the running share
        let running_share = {
            let mut running = self.running_shares.write().await;
            running.remove(&share_id)
        };

        if let Some(mut share) = running_share {
            // Stop tunnel first
            if let Err(e) = share.tunnel_handle.stop() {
                warn!(share_id = %share_id, error = %e, "Error stopping tunnel");
            }

            // Then stop server
            share.server_handle.stop().await;
            debug!(share_id = %share_id, "Server and tunnel stopped");
        } else {
            debug!(share_id = %share_id, "Share not running, only updating database");
        }

        // Update database status
        self.update_share_status(share_id, ShareStatus::Stopped)?;

        info!(share_id = %share_id, "Share stopped");
        Ok(())
    }

    /// List all shares from the database
    pub fn list_shares(&self) -> ShareServiceResult<Vec<ShareInfo>> {
        let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
        let shares = db.list_shares(None)?;
        Ok(shares)
    }

    /// List only active shares
    pub fn list_active_shares(&self) -> ShareServiceResult<Vec<ShareInfo>> {
        let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
        let shares = db.list_active_shares()?;
        Ok(shares)
    }

    /// Get a specific share by ID
    pub fn get_share(&self, share_id: ShareId) -> ShareServiceResult<Option<ShareInfo>> {
        let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
        let share = db.get_share(share_id)?;
        Ok(share)
    }

    /// Check if a share is currently running (has active handles)
    pub async fn is_share_running(&self, share_id: ShareId) -> bool {
        let running = self.running_shares.read().await;
        running.contains_key(&share_id)
    }

    /// Get the number of running shares
    pub async fn running_share_count(&self) -> usize {
        let running = self.running_shares.read().await;
        running.len()
    }

    /// Update share status in the database
    fn update_share_status(
        &self,
        share_id: ShareId,
        status: ShareStatus,
    ) -> ShareServiceResult<()> {
        let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
        db.update_share_status(share_id, status)?;
        Ok(())
    }

    /// Stop all running shares
    ///
    /// This is typically called during daemon shutdown.
    pub async fn stop_all_shares(&self) {
        info!("Stopping all running shares");

        let share_ids: Vec<ShareId> = {
            let running = self.running_shares.read().await;
            running.keys().copied().collect()
        };

        for share_id in share_ids {
            if let Err(e) = self.stop_share(share_id).await {
                error!(share_id = %share_id, error = %e, "Error stopping share during shutdown");
            }
        }

        info!("All shares stopped");
    }

    /// Restore shares from database on daemon startup
    ///
    /// This marks any shares that were in "Active" or "Starting" status as "Error"
    /// since the daemon was restarted and the actual processes are no longer running.
    ///
    /// In the future, this could be enhanced to actually restart the shares.
    pub fn restore_on_startup(&self) -> ShareServiceResult<()> {
        info!("Restoring share state on daemon startup");

        let shares = self.list_shares()?;

        for share in shares {
            match share.status {
                ShareStatus::Active | ShareStatus::Starting => {
                    // These shares were running but the daemon was restarted
                    // Mark them as error since we can't restore the tunnel URL
                    warn!(
                        share_id = %share.id,
                        session = %share.session_name,
                        "Share was active but daemon restarted, marking as error"
                    );
                    self.update_share_status(share.id, ShareStatus::Error)?;
                }
                ShareStatus::Error | ShareStatus::Stopped => {
                    // These are already in a terminal state
                    debug!(share_id = %share.id, status = ?share.status, "Share already in terminal state");
                }
            }
        }

        Ok(())
    }

    /// Clean up old stopped/error shares from the database
    ///
    /// This removes shares older than the specified number of hours.
    pub fn cleanup_old_shares(&self, max_age_hours: u64) -> ShareServiceResult<usize> {
        let db = self.db.lock().map_err(|_| ShareServiceError::LockError)?;
        let shares = db.list_shares(None)?;

        let cutoff = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let mut removed_count = 0;

        for share in shares {
            if matches!(share.status, ShareStatus::Stopped | ShareStatus::Error)
                && share.started_at < cutoff
            {
                db.delete_share(share.id)?;
                removed_count += 1;
                debug!(share_id = %share.id, "Cleaned up old share");
            }
        }

        if removed_count > 0 {
            info!(count = removed_count, "Cleaned up old shares");
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::db::Database;

    fn create_test_db() -> Arc<Mutex<Database>> {
        Arc::new(Mutex::new(
            Database::open_in_memory().expect("Failed to create test database"),
        ))
    }

    fn create_test_service() -> ShareService {
        let db = create_test_db();
        let config = Config::default();
        ShareService::new(db, config)
    }

    #[test]
    fn test_share_service_creation() {
        let service = create_test_service();
        assert!(service.list_shares().unwrap().is_empty());
    }

    #[test]
    fn test_share_service_with_config() {
        let db = create_test_db();
        let mut config = Config::default();
        config.set_ngrok_token(Some("test_token".to_string()));
        config.set_default_port(Some(4000));

        let service = ShareService::new(db, config);
        assert_eq!(service.ngrok_token(), Some("test_token"));
        assert_eq!(service.effective_port(), 4000);
    }

    #[test]
    fn test_share_service_default_port() {
        let service = create_test_service();
        assert_eq!(service.effective_port(), 3000);
    }

    #[tokio::test]
    async fn test_running_share_count_empty() {
        let service = create_test_service();
        assert_eq!(service.running_share_count().await, 0);
    }

    #[tokio::test]
    async fn test_is_share_running_false() {
        let service = create_test_service();
        let share_id = ShareId::new();
        assert!(!service.is_share_running(share_id).await);
    }

    #[test]
    fn test_update_share_status() {
        let db = create_test_db();
        let service = ShareService::new(db.clone(), Config::default());

        // Insert a test share
        let share_id = ShareId::new();
        let share = ShareInfo {
            id: share_id,
            session_path: PathBuf::from("/test/session.jsonl"),
            session_name: "test".to_string(),
            public_url: "https://example.com".to_string(),
            provider_name: "mock".to_string(),
            local_port: 8080,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Active,
        };

        {
            let db = db.lock().unwrap();
            db.insert_share(&share).unwrap();
        }

        // Update status
        service
            .update_share_status(share_id, ShareStatus::Stopped)
            .unwrap();

        // Verify
        let retrieved = service.get_share(share_id).unwrap().unwrap();
        assert_eq!(retrieved.status, ShareStatus::Stopped);
    }

    #[test]
    fn test_list_shares() {
        let db = create_test_db();
        let service = ShareService::new(db.clone(), Config::default());

        // Insert test shares
        for i in 0..3 {
            let share = ShareInfo {
                id: ShareId::new(),
                session_path: PathBuf::from(format!("/test/session{}.jsonl", i)),
                session_name: format!("test{}", i),
                public_url: format!("https://example{}.com", i),
                provider_name: "mock".to_string(),
                local_port: 8080 + i as u16,
                started_at: chrono::Utc::now(),
                status: ShareStatus::Active,
            };
            let db = db.lock().unwrap();
            db.insert_share(&share).unwrap();
        }

        let shares = service.list_shares().unwrap();
        assert_eq!(shares.len(), 3);
    }

    #[test]
    fn test_list_active_shares() {
        let db = create_test_db();
        let service = ShareService::new(db.clone(), Config::default());

        // Insert active share
        let active_share = ShareInfo {
            id: ShareId::new(),
            session_path: PathBuf::from("/test/active.jsonl"),
            session_name: "active".to_string(),
            public_url: "https://active.com".to_string(),
            provider_name: "mock".to_string(),
            local_port: 8080,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Active,
        };

        // Insert stopped share
        let stopped_share = ShareInfo {
            id: ShareId::new(),
            session_path: PathBuf::from("/test/stopped.jsonl"),
            session_name: "stopped".to_string(),
            public_url: "https://stopped.com".to_string(),
            provider_name: "mock".to_string(),
            local_port: 8081,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Stopped,
        };

        {
            let db = db.lock().unwrap();
            db.insert_share(&active_share).unwrap();
            db.insert_share(&stopped_share).unwrap();
        }

        let active = service.list_active_shares().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].session_name, "active");
    }

    #[test]
    fn test_restore_on_startup() {
        let db = create_test_db();
        let service = ShareService::new(db.clone(), Config::default());

        // Insert shares in various states
        let active_id = ShareId::new();
        let starting_id = ShareId::new();
        let stopped_id = ShareId::new();

        {
            let db = db.lock().unwrap();

            db.insert_share(&ShareInfo {
                id: active_id,
                session_path: PathBuf::from("/test/active.jsonl"),
                session_name: "active".to_string(),
                public_url: "https://active.com".to_string(),
                provider_name: "mock".to_string(),
                local_port: 8080,
                started_at: chrono::Utc::now(),
                status: ShareStatus::Active,
            })
            .unwrap();

            db.insert_share(&ShareInfo {
                id: starting_id,
                session_path: PathBuf::from("/test/starting.jsonl"),
                session_name: "starting".to_string(),
                public_url: "".to_string(),
                provider_name: "mock".to_string(),
                local_port: 0,
                started_at: chrono::Utc::now(),
                status: ShareStatus::Starting,
            })
            .unwrap();

            db.insert_share(&ShareInfo {
                id: stopped_id,
                session_path: PathBuf::from("/test/stopped.jsonl"),
                session_name: "stopped".to_string(),
                public_url: "https://stopped.com".to_string(),
                provider_name: "mock".to_string(),
                local_port: 8082,
                started_at: chrono::Utc::now(),
                status: ShareStatus::Stopped,
            })
            .unwrap();
        }

        // Restore on startup
        service.restore_on_startup().unwrap();

        // Verify active and starting are now error, stopped is unchanged
        let active = service.get_share(active_id).unwrap().unwrap();
        assert_eq!(active.status, ShareStatus::Error);

        let starting = service.get_share(starting_id).unwrap().unwrap();
        assert_eq!(starting.status, ShareStatus::Error);

        let stopped = service.get_share(stopped_id).unwrap().unwrap();
        assert_eq!(stopped.status, ShareStatus::Stopped);
    }

    #[test]
    fn test_cleanup_old_shares() {
        let db = create_test_db();
        let service = ShareService::new(db.clone(), Config::default());

        // Insert old stopped share (simulate by manually adjusting the database)
        let old_share = ShareInfo {
            id: ShareId::new(),
            session_path: PathBuf::from("/test/old.jsonl"),
            session_name: "old".to_string(),
            public_url: "https://old.com".to_string(),
            provider_name: "mock".to_string(),
            local_port: 8080,
            started_at: chrono::Utc::now() - chrono::Duration::hours(48),
            status: ShareStatus::Stopped,
        };

        // Insert recent active share
        let recent_share = ShareInfo {
            id: ShareId::new(),
            session_path: PathBuf::from("/test/recent.jsonl"),
            session_name: "recent".to_string(),
            public_url: "https://recent.com".to_string(),
            provider_name: "mock".to_string(),
            local_port: 8081,
            started_at: chrono::Utc::now(),
            status: ShareStatus::Active,
        };

        {
            let db = db.lock().unwrap();
            db.insert_share(&old_share).unwrap();
            db.insert_share(&recent_share).unwrap();
        }

        // Cleanup shares older than 24 hours
        let removed = service.cleanup_old_shares(24).unwrap();
        assert_eq!(removed, 1);

        // Verify only recent share remains
        let shares = service.list_shares().unwrap();
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].session_name, "recent");
    }

    #[test]
    fn test_share_service_error_display() {
        let err = ShareServiceError::UnknownProvider("test".to_string());
        assert!(err.to_string().contains("Unknown tunnel provider"));

        let err = ShareServiceError::ShareNotFound(ShareId::new());
        assert!(err.to_string().contains("Share not found"));

        let err = ShareServiceError::ParseError("parse failed".to_string());
        assert!(err.to_string().contains("parse failed"));
    }
}
