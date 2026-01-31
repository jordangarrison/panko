//! Background sharing management for TUI.
//!
//! This module provides functionality for managing sharing sessions in the background
//! while the TUI continues to run. Supports multiple concurrent shares.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::config::Config;
use crate::parser::{ClaudeParser, SessionParser};
use crate::server::{start_server, ServerConfig};
use crate::tunnel::get_provider_with_config;

// Global counter for generating unique share IDs
static SHARE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a sharing session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShareId(u64);

impl ShareId {
    /// Generate a new unique share ID.
    pub fn new() -> Self {
        Self(SHARE_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    /// Get the numeric value of this ID.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for ShareId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ShareId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "share-{}", self.0)
    }
}

/// Information about an active share.
#[derive(Debug, Clone)]
pub struct ActiveShare {
    /// Unique identifier for this share.
    pub id: ShareId,
    /// Path to the session file being shared.
    pub session_path: PathBuf,
    /// Public URL where the session is available.
    pub public_url: String,
    /// Name of the tunnel provider being used.
    pub provider_name: String,
    /// When this share was started.
    pub started_at: Instant,
}

impl ActiveShare {
    /// Create a new active share.
    pub fn new(
        id: ShareId,
        session_path: PathBuf,
        public_url: String,
        provider_name: String,
    ) -> Self {
        Self {
            id,
            session_path,
            public_url,
            provider_name,
            started_at: Instant::now(),
        }
    }

    /// Get the session filename (without path).
    pub fn session_name(&self) -> &str {
        self.session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    }

    /// Get the duration since this share started.
    pub fn duration(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Format the duration as a human-readable string.
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
}

/// Container for managing multiple concurrent shares.
#[derive(Debug, Default)]
pub struct ShareManager {
    /// Active shares indexed by ID.
    pub active_shares: Vec<ActiveShare>,
    /// Handles for background sharing threads, indexed by share ID.
    pub handles: HashMap<ShareId, SharingHandle>,
    /// Maximum number of concurrent shares allowed.
    pub max_shares: usize,
}

impl ShareManager {
    /// Create a new share manager with the given maximum concurrent shares.
    pub fn new(max_shares: usize) -> Self {
        Self {
            active_shares: Vec::new(),
            handles: HashMap::new(),
            max_shares,
        }
    }

    /// Check if we can start a new share.
    pub fn can_add_share(&self) -> bool {
        self.active_shares.len() < self.max_shares
    }

    /// Get the number of active shares.
    pub fn active_count(&self) -> usize {
        self.active_shares.len()
    }

    /// Check if there are any active shares.
    pub fn has_active_shares(&self) -> bool {
        !self.active_shares.is_empty()
    }

    /// Add a new share. Returns the share ID.
    pub fn add_share(
        &mut self,
        session_path: PathBuf,
        public_url: String,
        provider_name: String,
        handle: SharingHandle,
    ) -> ShareId {
        let id = ShareId::new();
        let share = ActiveShare::new(id, session_path, public_url, provider_name);
        self.active_shares.push(share);
        self.handles.insert(id, handle);
        id
    }

    /// Add a pending share (before URL is known). Returns the share ID.
    /// The share will be updated with URL when Started message is received.
    pub fn add_pending_share(
        &mut self,
        id: ShareId,
        _session_path: PathBuf,
        _provider_name: String,
        handle: SharingHandle,
    ) {
        // Don't add to active_shares yet - wait for Started message
        self.handles.insert(id, handle);
    }

    /// Mark a pending share as started with the given URL.
    pub fn mark_started(
        &mut self,
        id: ShareId,
        session_path: PathBuf,
        public_url: String,
        provider_name: String,
    ) {
        let share = ActiveShare::new(id, session_path, public_url, provider_name);
        self.active_shares.push(share);
    }

    /// Stop a share by ID.
    pub fn stop_share(&mut self, id: ShareId) {
        if let Some(handle) = self.handles.remove(&id) {
            handle.stop();
        }
        self.active_shares.retain(|s| s.id != id);
    }

    /// Stop all shares.
    pub fn stop_all(&mut self) {
        for (_, handle) in self.handles.drain() {
            handle.stop();
        }
        self.active_shares.clear();
    }

    /// Get a share by ID.
    pub fn get_share(&self, id: ShareId) -> Option<&ActiveShare> {
        self.active_shares.iter().find(|s| s.id == id)
    }

    /// Get all active shares.
    pub fn shares(&self) -> &[ActiveShare] {
        &self.active_shares
    }

    /// Get a handle by ID.
    pub fn get_handle(&self, id: ShareId) -> Option<&SharingHandle> {
        self.handles.get(&id)
    }

    /// Try to receive messages from all handles.
    /// Returns a list of (ShareId, SharingMessage) pairs.
    pub fn poll_messages(&self) -> Vec<(ShareId, SharingMessage)> {
        let mut messages = Vec::new();
        for (&id, handle) in &self.handles {
            while let Some(msg) = handle.try_recv() {
                messages.push((id, msg));
            }
        }
        messages
    }

    /// Remove handle for a share (when it has stopped).
    pub fn remove_handle(&mut self, id: ShareId) {
        self.handles.remove(&id);
        self.active_shares.retain(|s| s.id != id);
    }
}

/// Messages sent from the sharing background thread to the TUI.
#[derive(Debug)]
pub enum SharingMessage {
    /// Sharing started successfully with the given public URL.
    Started { url: String },
    /// An error occurred while starting sharing.
    Error { message: String },
    /// Sharing has stopped.
    Stopped,
}

/// Extended sharing message that includes the share ID.
/// Used when polling messages from the ShareManager.
#[derive(Debug)]
pub struct ShareMessage {
    /// The share ID this message relates to.
    pub share_id: ShareId,
    /// The actual message content.
    pub message: SharingMessage,
}

/// Messages sent from the TUI to the sharing background thread.
#[derive(Debug)]
pub enum SharingCommand {
    /// Stop sharing.
    Stop,
}

/// Handle for managing a background sharing session.
#[derive(Debug)]
pub struct SharingHandle {
    /// Channel to send commands to the sharing thread.
    command_tx: Sender<SharingCommand>,
    /// Channel to receive messages from the sharing thread.
    message_rx: Receiver<SharingMessage>,
    /// Join handle for the sharing thread.
    _thread_handle: thread::JoinHandle<()>,
}

impl SharingHandle {
    /// Start sharing a session in the background.
    pub fn start(session_path: PathBuf, provider_name: String) -> Self {
        let (message_tx, message_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            sharing_thread(session_path, provider_name, message_tx, command_rx);
        });

        Self {
            command_tx,
            message_rx,
            _thread_handle: thread_handle,
        }
    }

    /// Try to receive a message from the sharing thread (non-blocking).
    pub fn try_recv(&self) -> Option<SharingMessage> {
        self.message_rx.try_recv().ok()
    }

    /// Stop the sharing session.
    pub fn stop(&self) {
        let _ = self.command_tx.send(SharingCommand::Stop);
    }
}

/// The background thread function that manages sharing.
fn sharing_thread(
    session_path: PathBuf,
    provider_name: String,
    message_tx: Sender<SharingMessage>,
    command_rx: Receiver<SharingCommand>,
) {
    // Create a tokio runtime for async operations
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = message_tx.send(SharingMessage::Error {
                message: format!("Failed to create runtime: {}", e),
            });
            return;
        }
    };

    rt.block_on(async {
        // Parse the session file
        let parser = ClaudeParser::new();
        let session = match parser.parse(&session_path) {
            Ok(s) => s,
            Err(e) => {
                let _ = message_tx.send(SharingMessage::Error {
                    message: format!("Failed to parse session: {}", e),
                });
                return;
            }
        };

        // Load config for ngrok token and port
        let config = Config::load().unwrap_or_default();
        let ngrok_token = config.ngrok_token.as_deref();
        let port = config.effective_port(3000);

        // Get the tunnel provider
        let provider = match get_provider_with_config(&provider_name, ngrok_token) {
            Some(p) => p,
            None => {
                let _ = message_tx.send(SharingMessage::Error {
                    message: format!("Unknown provider: {}", provider_name),
                });
                return;
            }
        };

        // Start the local server
        let server_config = ServerConfig {
            base_port: port,
            open_browser: false,
        };

        let server_handle = match start_server(session, server_config).await {
            Ok(h) => h,
            Err(e) => {
                let _ = message_tx.send(SharingMessage::Error {
                    message: format!("Failed to start server: {}", e),
                });
                return;
            }
        };

        let actual_port = server_handle.port();

        // Spawn the tunnel
        let mut tunnel_handle = match provider.spawn(actual_port) {
            Ok(h) => h,
            Err(e) => {
                server_handle.stop().await;
                let _ = message_tx.send(SharingMessage::Error {
                    message: format!("Failed to start tunnel: {}", e),
                });
                return;
            }
        };

        let public_url = tunnel_handle.url().to_string();

        // Send the started message
        let _ = message_tx.send(SharingMessage::Started { url: public_url });

        // Wait for stop command (blocking wait)
        // This will block until we receive Stop command or the channel is closed
        let _ = command_rx.recv();

        // Cleanup
        let _ = tunnel_handle.stop();
        server_handle.stop().await;

        let _ = message_tx.send(SharingMessage::Stopped);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_sharing_message_debug() {
        let msg = SharingMessage::Started {
            url: "https://example.com".to_string(),
        };
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Started"));
    }

    #[test]
    fn test_sharing_command_debug() {
        let cmd = SharingCommand::Stop;
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("Stop"));
    }

    #[test]
    fn test_try_recv_returns_none_on_empty_channel() {
        let (_tx, rx) = mpsc::channel::<SharingMessage>();
        // try_recv should return Err immediately (non-blocking)
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_stop_command_sends_correctly() {
        let (tx, rx) = mpsc::channel::<SharingCommand>();
        tx.send(SharingCommand::Stop).unwrap();

        let received = rx.recv_timeout(Duration::from_millis(10));
        assert!(matches!(received, Ok(SharingCommand::Stop)));
    }

    #[test]
    fn test_message_started_round_trip() {
        let (tx, rx) = mpsc::channel();
        let url = "https://test.example.com".to_string();
        tx.send(SharingMessage::Started { url: url.clone() })
            .unwrap();

        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(SharingMessage::Started { url: received_url }) => {
                assert_eq!(received_url, url);
            }
            _ => panic!("Expected Started message"),
        }
    }

    #[test]
    fn test_message_error_round_trip() {
        let (tx, rx) = mpsc::channel();
        tx.send(SharingMessage::Error {
            message: "test error".into(),
        })
        .unwrap();

        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(SharingMessage::Error { message }) => {
                assert_eq!(message, "test error");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_message_stopped_round_trip() {
        let (tx, rx) = mpsc::channel();
        tx.send(SharingMessage::Stopped).unwrap();

        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(10)),
            Ok(SharingMessage::Stopped)
        ));
    }

    #[test]
    fn test_channel_operations_are_fast() {
        let (_tx, rx) = mpsc::channel::<SharingMessage>();

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = rx.try_recv();
        }
        // 1000 try_recv calls should complete in under 10ms
        assert!(
            start.elapsed().as_millis() < 10,
            "Channel operations too slow: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn test_sharing_message_variants() {
        // Test all message types can be created and matched
        let messages = vec![
            SharingMessage::Started {
                url: "https://example.com".to_string(),
            },
            SharingMessage::Error {
                message: "error".to_string(),
            },
            SharingMessage::Stopped,
        ];

        for msg in messages {
            match msg {
                SharingMessage::Started { url } => assert!(!url.is_empty()),
                SharingMessage::Error { message } => assert!(!message.is_empty()),
                SharingMessage::Stopped => {}
            }
        }
    }

    #[test]
    fn test_sharing_command_variants() {
        // Test command can be created and matched
        let cmd = SharingCommand::Stop;
        assert!(matches!(cmd, SharingCommand::Stop));
    }

    // ShareId tests

    #[test]
    fn test_share_id_new_unique() {
        let id1 = ShareId::new();
        let id2 = ShareId::new();
        let id3 = ShareId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_share_id_display() {
        let id = ShareId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("share-"));
    }

    #[test]
    fn test_share_id_as_u64() {
        let id = ShareId::new();
        let value = id.as_u64();
        assert!(value > 0);
    }

    #[test]
    fn test_share_id_eq_and_hash() {
        use std::collections::HashSet;

        let id1 = ShareId::new();
        let id2 = ShareId::new();

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&id1));
        assert!(set.contains(&id2));
    }

    #[test]
    fn test_share_id_copy_clone() {
        let id1 = ShareId::new();
        let id2 = id1; // Copy
        let id3 = id1.clone();

        assert_eq!(id1, id2);
        assert_eq!(id1, id3);
    }

    // ActiveShare tests

    #[test]
    fn test_active_share_new() {
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/session.jsonl");
        let url = "https://example.com".to_string();
        let provider = "cloudflare".to_string();

        let share = ActiveShare::new(id, path.clone(), url.clone(), provider.clone());

        assert_eq!(share.id, id);
        assert_eq!(share.session_path, path);
        assert_eq!(share.public_url, url);
        assert_eq!(share.provider_name, provider);
    }

    #[test]
    fn test_active_share_session_name() {
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/my_session.jsonl");
        let share = ActiveShare::new(id, path, "https://example.com".into(), "ngrok".into());

        assert_eq!(share.session_name(), "my_session");
    }

    #[test]
    fn test_active_share_session_name_no_extension() {
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/session");
        let share = ActiveShare::new(id, path, "https://example.com".into(), "ngrok".into());

        assert_eq!(share.session_name(), "session");
    }

    #[test]
    fn test_active_share_duration() {
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/session.jsonl");
        let share = ActiveShare::new(id, path, "https://example.com".into(), "ngrok".into());

        // Duration should be very small (just created)
        assert!(share.duration().as_secs() < 1);
    }

    #[test]
    fn test_active_share_duration_string() {
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/session.jsonl");
        let share = ActiveShare::new(id, path, "https://example.com".into(), "ngrok".into());

        // Just created, should show seconds
        let duration_str = share.duration_string();
        assert!(duration_str.ends_with('s'));
    }

    // ShareManager tests

    #[test]
    fn test_share_manager_new() {
        let manager = ShareManager::new(5);
        assert_eq!(manager.max_shares, 5);
        assert!(manager.active_shares.is_empty());
        assert!(manager.handles.is_empty());
    }

    #[test]
    fn test_share_manager_default() {
        let manager = ShareManager::default();
        assert!(manager.active_shares.is_empty());
        assert!(manager.handles.is_empty());
    }

    #[test]
    fn test_share_manager_can_add_share() {
        let manager = ShareManager::new(2);
        assert!(manager.can_add_share());
    }

    #[test]
    fn test_share_manager_active_count() {
        let manager = ShareManager::new(5);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_share_manager_has_active_shares() {
        let manager = ShareManager::new(5);
        assert!(!manager.has_active_shares());
    }

    #[test]
    fn test_share_manager_shares_empty() {
        let manager = ShareManager::new(5);
        assert!(manager.shares().is_empty());
    }

    #[test]
    fn test_share_manager_get_share_none() {
        let manager = ShareManager::new(5);
        let id = ShareId::new();
        assert!(manager.get_share(id).is_none());
    }

    #[test]
    fn test_share_manager_get_handle_none() {
        let manager = ShareManager::new(5);
        let id = ShareId::new();
        assert!(manager.get_handle(id).is_none());
    }

    #[test]
    fn test_share_manager_poll_messages_empty() {
        let manager = ShareManager::new(5);
        let messages = manager.poll_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_share_manager_mark_started() {
        let mut manager = ShareManager::new(5);
        let id = ShareId::new();
        let path = PathBuf::from("/path/to/session.jsonl");
        let url = "https://example.com".to_string();
        let provider = "cloudflare".to_string();

        manager.mark_started(id, path.clone(), url.clone(), provider.clone());

        assert_eq!(manager.active_count(), 1);
        assert!(manager.has_active_shares());

        let share = manager.get_share(id).unwrap();
        assert_eq!(share.id, id);
        assert_eq!(share.public_url, url);
    }

    #[test]
    fn test_share_manager_stop_all() {
        let mut manager = ShareManager::new(5);

        // Add some shares manually (without handles for this test)
        let id1 = ShareId::new();
        let id2 = ShareId::new();
        manager.mark_started(
            id1,
            PathBuf::from("/a.jsonl"),
            "https://a.com".into(),
            "ngrok".into(),
        );
        manager.mark_started(
            id2,
            PathBuf::from("/b.jsonl"),
            "https://b.com".into(),
            "cloudflare".into(),
        );

        assert_eq!(manager.active_count(), 2);

        manager.stop_all();

        assert_eq!(manager.active_count(), 0);
        assert!(!manager.has_active_shares());
    }

    // ShareMessage test

    #[test]
    fn test_share_message_debug() {
        let msg = ShareMessage {
            share_id: ShareId::new(),
            message: SharingMessage::Started {
                url: "https://example.com".to_string(),
            },
        };
        let debug = format!("{:?}", msg);
        assert!(debug.contains("ShareMessage"));
    }
}
