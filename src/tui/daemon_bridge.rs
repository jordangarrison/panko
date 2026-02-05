//! Bridge between TUI and daemon for sharing functionality
//!
//! This module provides a synchronous interface for the TUI to communicate
//! with the daemon. It spawns background tasks that use the async DaemonClient
//! and communicates back via channels.

use crate::daemon::client::{ClientError, DaemonClient};
use crate::daemon::protocol::{ShareId as DaemonShareId, ShareInfo, ShareStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Messages sent from daemon bridge tasks to the TUI
#[derive(Debug)]
pub enum DaemonMessage {
    /// Daemon connected successfully
    Connected,
    /// Daemon connection failed
    ConnectionFailed { error: String },
    /// Share started successfully
    ShareStarted { info: ShareInfo },
    /// Share failed to start
    ShareFailed { error: String },
    /// Share stopped
    ShareStopped { share_id: DaemonShareId },
    /// Share list received
    ShareListReceived { shares: Vec<ShareInfo> },
    /// Error occurred during operation
    Error { message: String },
}

/// Handle for a pending share operation
#[derive(Debug)]
pub struct DaemonShareHandle {
    /// Receiver for messages from the daemon task
    pub message_rx: Receiver<DaemonMessage>,
    /// The daemon share ID (set after ShareStarted is received)
    pub daemon_share_id: Option<DaemonShareId>,
}

impl DaemonShareHandle {
    /// Try to receive a message from the daemon task (non-blocking)
    pub fn try_recv(&self) -> Option<DaemonMessage> {
        self.message_rx.try_recv().ok()
    }
}

/// Information about an active share from the daemon
#[derive(Debug, Clone)]
pub struct DaemonActiveShare {
    /// Daemon's share ID (UUID)
    pub daemon_id: DaemonShareId,
    /// Path to the session file being shared
    pub session_path: PathBuf,
    /// Public URL where the session is available
    pub public_url: String,
    /// Name of the tunnel provider being used
    pub provider_name: String,
    /// When this share was started (as observed by TUI)
    pub started_at: Instant,
    /// Current status
    pub status: ShareStatus,
}

impl DaemonActiveShare {
    /// Create from ShareInfo
    pub fn from_share_info(info: &ShareInfo) -> Self {
        Self {
            daemon_id: info.id,
            session_path: info.session_path.clone(),
            public_url: info.public_url.clone(),
            provider_name: info.provider_name.clone(),
            started_at: Instant::now(),
            status: info.status,
        }
    }

    /// Get the session filename (without path)
    pub fn session_name(&self) -> &str {
        self.session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    }

    /// Get the duration since this share started
    pub fn duration(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Format the duration as a human-readable string
    pub fn duration_string(&self) -> String {
        let secs = self.duration().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Check if the share is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, ShareStatus::Active | ShareStatus::Starting)
    }
}

/// Manager for daemon-based sharing
///
/// This replaces the old ShareManager when daemon mode is enabled.
/// It tracks active shares by querying the daemon.
#[derive(Debug, Default)]
pub struct DaemonShareManager {
    /// Active shares tracked by daemon ID
    pub shares: HashMap<DaemonShareId, DaemonActiveShare>,
    /// Pending share handles (before we get the daemon ID)
    pub pending_handles: Vec<DaemonShareHandle>,
    /// Maximum number of concurrent shares allowed
    pub max_shares: usize,
    /// Currently selected share index (for shares panel)
    pub selected_index: usize,
}

impl DaemonShareManager {
    /// Create a new daemon share manager
    pub fn new(max_shares: usize) -> Self {
        Self {
            shares: HashMap::new(),
            pending_handles: Vec::new(),
            max_shares,
            selected_index: 0,
        }
    }

    /// Check if we can start a new share
    pub fn can_add_share(&self) -> bool {
        self.active_count() < self.max_shares
    }

    /// Get the number of active shares
    pub fn active_count(&self) -> usize {
        self.shares.values().filter(|s| s.is_active()).count() + self.pending_handles.len()
    }

    /// Check if there are any active shares
    pub fn has_active_shares(&self) -> bool {
        self.active_count() > 0
    }

    /// Get all active shares as a sorted list
    pub fn active_shares(&self) -> Vec<&DaemonActiveShare> {
        let mut shares: Vec<_> = self.shares.values().filter(|s| s.is_active()).collect();
        shares.sort_by(|a, b| b.started_at.cmp(&a.started_at)); // newest first
        shares
    }

    /// Get a share by daemon ID
    pub fn get_share(&self, id: &DaemonShareId) -> Option<&DaemonActiveShare> {
        self.shares.get(id)
    }

    /// Check if a session path is currently being shared
    pub fn is_session_shared(&self, path: &std::path::Path) -> bool {
        self.shares
            .values()
            .any(|s| s.is_active() && s.session_path == path)
    }

    /// Add a pending share handle
    pub fn add_pending(&mut self, handle: DaemonShareHandle) {
        self.pending_handles.push(handle);
    }

    /// Update shares from daemon ShareInfo list
    pub fn update_from_daemon(&mut self, shares: Vec<ShareInfo>) {
        // Update or add shares
        for info in shares {
            if let Some(existing) = self.shares.get_mut(&info.id) {
                // Update status
                existing.status = info.status;
                existing.public_url = info.public_url.clone();
            } else {
                // Add new share
                self.shares
                    .insert(info.id, DaemonActiveShare::from_share_info(&info));
            }
        }

        // Remove shares that are no longer in the daemon list (and not active)
        // We keep stopped/error shares for a bit so the user can see them
    }

    /// Mark a share as active with info from daemon
    pub fn mark_started(&mut self, info: ShareInfo) {
        self.shares
            .insert(info.id, DaemonActiveShare::from_share_info(&info));
    }

    /// Mark a share as stopped
    pub fn mark_stopped(&mut self, share_id: DaemonShareId) {
        if let Some(share) = self.shares.get_mut(&share_id) {
            share.status = ShareStatus::Stopped;
        }
        // Also remove from shares map since it's no longer active
        self.shares.remove(&share_id);
    }

    /// Process messages from pending handles
    /// Returns a list of (daemon_share_id, message) for shares that started
    pub fn poll_messages(&mut self) -> Vec<DaemonMessage> {
        let mut messages = Vec::new();
        let mut completed_indices = Vec::new();

        for (idx, handle) in self.pending_handles.iter_mut().enumerate() {
            while let Some(msg) = handle.try_recv() {
                match &msg {
                    DaemonMessage::ShareStarted { info } => {
                        handle.daemon_share_id = Some(info.id);
                        self.shares
                            .insert(info.id, DaemonActiveShare::from_share_info(info));
                        messages.push(msg);
                        completed_indices.push(idx);
                    }
                    DaemonMessage::ShareFailed { .. } => {
                        messages.push(msg);
                        completed_indices.push(idx);
                    }
                    _ => {
                        messages.push(msg);
                    }
                }
            }
        }

        // Remove completed handles (in reverse order to preserve indices)
        for idx in completed_indices.into_iter().rev() {
            self.pending_handles.remove(idx);
        }

        messages
    }

    /// Get the currently selected share (for shares panel)
    pub fn selected_share(&self) -> Option<&DaemonActiveShare> {
        let shares = self.active_shares();
        shares.get(self.selected_index).copied()
    }

    /// Select next share in the list
    pub fn select_next(&mut self) {
        let count = self.active_shares().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Select previous share in the list
    pub fn select_previous(&mut self) {
        let count = self.active_shares().len();
        if count > 0 {
            if self.selected_index == 0 {
                self.selected_index = count - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Get the currently selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Clear all shares (for shutdown)
    pub fn clear(&mut self) {
        self.shares.clear();
        self.pending_handles.clear();
    }
}

/// Start a share via the daemon
///
/// This spawns a background thread that:
/// 1. Connects to the daemon (starting it if needed)
/// 2. Sends the StartShare request
/// 3. Returns the result via the message channel
pub fn start_share_via_daemon(session_path: PathBuf, provider_name: String) -> DaemonShareHandle {
    let (message_tx, message_rx) = mpsc::channel();

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = message_tx.send(DaemonMessage::ShareFailed {
                    error: format!("Failed to create runtime: {}", e),
                });
                return;
            }
        };

        rt.block_on(async {
            debug!("Connecting to daemon for share...");

            // Connect to daemon (auto-starting if needed)
            let mut client = match DaemonClient::connect_or_start().await {
                Ok(c) => {
                    info!("Connected to daemon");
                    c
                }
                Err(e) => {
                    error!("Failed to connect to daemon: {}", e);
                    let _ = message_tx.send(DaemonMessage::ShareFailed {
                        error: format!("Failed to connect to daemon: {}", e),
                    });
                    return;
                }
            };

            // Start the share
            debug!(
                "Requesting share for {} with provider {}",
                session_path.display(),
                provider_name
            );
            match client.start_share(session_path, provider_name).await {
                Ok(info) => {
                    info!(
                        "Share started: {} at {}",
                        info.session_name, info.public_url
                    );
                    let _ = message_tx.send(DaemonMessage::ShareStarted { info });
                }
                Err(e) => {
                    error!("Failed to start share: {}", e);
                    let _ = message_tx.send(DaemonMessage::ShareFailed {
                        error: format!("Failed to start share: {}", e),
                    });
                }
            }
        });
    });

    DaemonShareHandle {
        message_rx,
        daemon_share_id: None,
    }
}

/// Stop a share via the daemon
///
/// This spawns a background thread that sends the stop request.
pub fn stop_share_via_daemon(share_id: DaemonShareId, callback_tx: Sender<DaemonMessage>) {
    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = callback_tx.send(DaemonMessage::Error {
                    message: format!("Failed to create runtime: {}", e),
                });
                return;
            }
        };

        rt.block_on(async {
            debug!("Connecting to daemon to stop share...");

            let mut client = match DaemonClient::connect().await {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to connect to daemon to stop share: {}", e);
                    // Even if we can't connect, consider it stopped
                    let _ = callback_tx.send(DaemonMessage::ShareStopped { share_id });
                    return;
                }
            };

            match client.stop_share(share_id).await {
                Ok(()) => {
                    info!("Share {} stopped", share_id);
                    let _ = callback_tx.send(DaemonMessage::ShareStopped { share_id });
                }
                Err(e) => {
                    warn!("Failed to stop share: {}", e);
                    // Consider it stopped anyway (daemon may have already stopped it)
                    let _ = callback_tx.send(DaemonMessage::ShareStopped { share_id });
                }
            }
        });
    });
}

/// Fetch the list of shares from the daemon
///
/// This is used to sync the TUI state with the daemon on startup.
pub fn fetch_shares_from_daemon(callback_tx: Sender<DaemonMessage>) {
    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = callback_tx.send(DaemonMessage::Error {
                    message: format!("Failed to create runtime: {}", e),
                });
                return;
            }
        };

        rt.block_on(async {
            debug!("Fetching shares from daemon...");

            let mut client = match DaemonClient::connect().await {
                Ok(c) => c,
                Err(ClientError::DaemonNotRunning) => {
                    debug!("Daemon not running, no shares to fetch");
                    let _ = callback_tx.send(DaemonMessage::ShareListReceived { shares: vec![] });
                    return;
                }
                Err(e) => {
                    warn!("Failed to connect to daemon: {}", e);
                    let _ = callback_tx.send(DaemonMessage::Error {
                        message: format!("Failed to connect to daemon: {}", e),
                    });
                    return;
                }
            };

            match client.list_shares().await {
                Ok(shares) => {
                    info!("Fetched {} shares from daemon", shares.len());
                    let _ = callback_tx.send(DaemonMessage::ShareListReceived { shares });
                }
                Err(e) => {
                    warn!("Failed to list shares: {}", e);
                    let _ = callback_tx.send(DaemonMessage::Error {
                        message: format!("Failed to list shares: {}", e),
                    });
                }
            }
        });
    });
}

/// Check if the daemon is running and try to connect
///
/// Returns a channel receiver that will receive Connected or ConnectionFailed
pub fn check_daemon_connection() -> Receiver<DaemonMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(DaemonMessage::ConnectionFailed {
                    error: format!("Failed to create runtime: {}", e),
                });
                return;
            }
        };

        rt.block_on(async {
            debug!("Checking daemon connection...");

            match DaemonClient::connect_or_start().await {
                Ok(mut client) => {
                    // Verify connection with a ping
                    match client.ping().await {
                        Ok(()) => {
                            info!("Daemon connection verified");
                            let _ = tx.send(DaemonMessage::Connected);
                        }
                        Err(e) => {
                            let _ = tx.send(DaemonMessage::ConnectionFailed {
                                error: format!("Daemon ping failed: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(DaemonMessage::ConnectionFailed {
                        error: e.to_string(),
                    });
                }
            }
        });
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_share_manager_new() {
        let manager = DaemonShareManager::new(5);
        assert_eq!(manager.max_shares, 5);
        assert!(manager.shares.is_empty());
        assert!(manager.pending_handles.is_empty());
    }

    #[test]
    fn test_daemon_share_manager_can_add_share() {
        let manager = DaemonShareManager::new(2);
        assert!(manager.can_add_share());
    }

    #[test]
    fn test_daemon_share_manager_active_count() {
        let manager = DaemonShareManager::new(5);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_daemon_share_manager_has_active_shares() {
        let manager = DaemonShareManager::new(5);
        assert!(!manager.has_active_shares());
    }

    #[test]
    fn test_daemon_active_share_duration_string() {
        let share = DaemonActiveShare {
            daemon_id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session.jsonl"),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            started_at: Instant::now(),
            status: ShareStatus::Active,
        };

        // Just created, should show seconds
        let duration_str = share.duration_string();
        assert!(duration_str.ends_with('s'));
    }

    #[test]
    fn test_daemon_active_share_session_name() {
        let share = DaemonActiveShare {
            daemon_id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/my_session.jsonl"),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            started_at: Instant::now(),
            status: ShareStatus::Active,
        };

        assert_eq!(share.session_name(), "my_session");
    }

    #[test]
    fn test_daemon_active_share_is_active() {
        let mut share = DaemonActiveShare {
            daemon_id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session.jsonl"),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            started_at: Instant::now(),
            status: ShareStatus::Active,
        };

        assert!(share.is_active());

        share.status = ShareStatus::Starting;
        assert!(share.is_active());

        share.status = ShareStatus::Stopped;
        assert!(!share.is_active());

        share.status = ShareStatus::Error;
        assert!(!share.is_active());
    }

    #[test]
    fn test_daemon_share_manager_mark_started() {
        use chrono::Utc;

        let mut manager = DaemonShareManager::new(5);
        let info = ShareInfo {
            id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session.jsonl"),
            session_name: "session".to_string(),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };

        manager.mark_started(info.clone());

        assert_eq!(manager.active_count(), 1);
        assert!(manager.has_active_shares());
        assert!(manager.get_share(&info.id).is_some());
    }

    #[test]
    fn test_daemon_share_manager_mark_stopped() {
        use chrono::Utc;

        let mut manager = DaemonShareManager::new(5);
        let info = ShareInfo {
            id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session.jsonl"),
            session_name: "session".to_string(),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };

        manager.mark_started(info.clone());
        assert_eq!(manager.active_count(), 1);

        manager.mark_stopped(info.id);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_daemon_share_manager_is_session_shared() {
        use chrono::Utc;

        let mut manager = DaemonShareManager::new(5);
        let path = PathBuf::from("/test/session.jsonl");
        let info = ShareInfo {
            id: DaemonShareId::new(),
            session_path: path.clone(),
            session_name: "session".to_string(),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };

        assert!(!manager.is_session_shared(&path));

        manager.mark_started(info);
        assert!(manager.is_session_shared(&path));

        assert!(!manager.is_session_shared(&PathBuf::from("/other/session.jsonl")));
    }

    #[test]
    fn test_daemon_share_manager_clear() {
        use chrono::Utc;

        let mut manager = DaemonShareManager::new(5);
        let info = ShareInfo {
            id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session.jsonl"),
            session_name: "session".to_string(),
            public_url: "https://example.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };

        manager.mark_started(info);
        assert_eq!(manager.active_count(), 1);

        manager.clear();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_daemon_share_manager_navigation() {
        use chrono::Utc;

        let mut manager = DaemonShareManager::new(5);

        // Add two shares
        let info1 = ShareInfo {
            id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session1.jsonl"),
            session_name: "session1".to_string(),
            public_url: "https://example1.com".to_string(),
            provider_name: "cloudflare".to_string(),
            local_port: 8080,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };
        let info2 = ShareInfo {
            id: DaemonShareId::new(),
            session_path: PathBuf::from("/test/session2.jsonl"),
            session_name: "session2".to_string(),
            public_url: "https://example2.com".to_string(),
            provider_name: "ngrok".to_string(),
            local_port: 8081,
            started_at: Utc::now(),
            status: ShareStatus::Active,
        };

        manager.mark_started(info1);
        manager.mark_started(info2);

        assert_eq!(manager.selected_index, 0);

        manager.select_next();
        assert_eq!(manager.selected_index, 1);

        manager.select_next();
        assert_eq!(manager.selected_index, 0); // wraps

        manager.select_previous();
        assert_eq!(manager.selected_index, 1); // wraps

        manager.select_previous();
        assert_eq!(manager.selected_index, 0);
    }
}
