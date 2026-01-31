//! Background sharing management for TUI.
//!
//! This module provides functionality for managing sharing sessions in the background
//! while the TUI continues to run.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::config::Config;
use crate::parser::{ClaudeParser, SessionParser};
use crate::server::{start_server, ServerConfig};
use crate::tunnel::get_provider_with_config;

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
}
