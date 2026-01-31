//! Daemon client for TUI communication with the sharing daemon
//!
//! The DaemonClient provides a high-level interface for the TUI to communicate
//! with the background daemon process over Unix sockets.

use crate::daemon::protocol::{DaemonRequest, DaemonResponse, ShareId, ShareInfo};
use crate::daemon::server::{default_socket_path, is_daemon_running};
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::UnixStream;
use tracing::{debug, info};

/// Default timeout for daemon operations
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for daemon startup check
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Interval between startup checks
const STARTUP_CHECK_INTERVAL: Duration = Duration::from_millis(100);

/// Errors that can occur when communicating with the daemon
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Failed to connect to daemon: {0}")]
    ConnectionFailed(std::io::Error),

    #[error("Daemon is not running")]
    DaemonNotRunning,

    #[error("Failed to start daemon: {0}")]
    DaemonStartFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Request timed out")]
    Timeout,

    #[error("Daemon returned error: {0}")]
    DaemonError(String),

    #[error("Unexpected response from daemon")]
    UnexpectedResponse,

    #[error("Connection closed by daemon")]
    ConnectionClosed,
}

/// Client for communicating with the panko daemon
pub struct DaemonClient {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
    socket_path: PathBuf,
}

impl DaemonClient {
    /// Connect to an existing daemon at the default socket path
    ///
    /// Returns an error if the daemon is not running.
    pub async fn connect() -> Result<Self, ClientError> {
        Self::connect_to(default_socket_path()).await
    }

    /// Connect to an existing daemon at the specified socket path
    ///
    /// Returns an error if the daemon is not running.
    pub async fn connect_to(socket_path: PathBuf) -> Result<Self, ClientError> {
        debug!("Connecting to daemon at {:?}", socket_path);

        // Check if socket exists
        if !socket_path.exists() {
            return Err(ClientError::DaemonNotRunning);
        }

        // Try to connect
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                    ClientError::DaemonNotRunning
                }
                _ => ClientError::ConnectionFailed(e),
            })?;

        let (read_half, write_half) = stream.into_split();
        let reader = BufReader::new(read_half);
        let writer = BufWriter::new(write_half);

        debug!("Connected to daemon at {:?}", socket_path);

        Ok(Self {
            reader,
            writer,
            socket_path,
        })
    }

    /// Connect to the daemon, starting it if not running
    ///
    /// This will:
    /// 1. Try to connect to an existing daemon
    /// 2. If that fails, spawn `panko serve` as a detached process
    /// 3. Wait up to 5 seconds for the socket to appear
    /// 4. Connect to the newly started daemon
    pub async fn connect_or_start() -> Result<Self, ClientError> {
        Self::connect_or_start_with_path(default_socket_path()).await
    }

    /// Connect to the daemon at the specified path, starting it if not running
    pub async fn connect_or_start_with_path(socket_path: PathBuf) -> Result<Self, ClientError> {
        // First, try to connect to existing daemon
        match Self::connect_to(socket_path.clone()).await {
            Ok(client) => {
                debug!("Connected to existing daemon");
                return Ok(client);
            }
            Err(ClientError::DaemonNotRunning) => {
                // Need to start the daemon
                debug!("Daemon not running, starting it");
            }
            Err(e) => return Err(e),
        }

        // Start the daemon
        Self::start_daemon().await?;

        // Wait for the socket to appear
        let start_time = std::time::Instant::now();
        while start_time.elapsed() < STARTUP_TIMEOUT {
            if socket_path.exists() {
                // Try to connect
                match Self::connect_to(socket_path.clone()).await {
                    Ok(client) => {
                        info!("Connected to newly started daemon");
                        return Ok(client);
                    }
                    Err(ClientError::DaemonNotRunning) => {
                        // Socket exists but can't connect yet, keep waiting
                    }
                    Err(e) => return Err(e),
                }
            }
            tokio::time::sleep(STARTUP_CHECK_INTERVAL).await;
        }

        Err(ClientError::DaemonStartFailed(
            "Daemon started but socket did not become available within timeout".to_string(),
        ))
    }

    /// Start the daemon process
    #[cfg(unix)]
    async fn start_daemon() -> Result<(), ClientError> {
        use std::process::{Command, Stdio};

        info!("Starting daemon process");

        // Get the current executable path
        let exe = std::env::current_exe().map_err(|e| {
            ClientError::DaemonStartFailed(format!("Failed to get current executable: {}", e))
        })?;

        // Spawn the daemon as a detached process
        Command::new(&exe)
            .args(["serve", "--foreground"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                ClientError::DaemonStartFailed(format!("Failed to spawn daemon process: {}", e))
            })?;

        debug!("Daemon process spawned");
        Ok(())
    }

    #[cfg(not(unix))]
    async fn start_daemon() -> Result<(), ClientError> {
        Err(ClientError::DaemonStartFailed(
            "Automatic daemon startup is not supported on this platform".to_string(),
        ))
    }

    /// Get the socket path this client is connected to
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Send a request to the daemon and receive a response
    async fn send_request(
        &mut self,
        request: DaemonRequest,
    ) -> Result<DaemonResponse, ClientError> {
        self.send_request_with_timeout(request, DEFAULT_TIMEOUT)
            .await
    }

    /// Send a request with a custom timeout
    async fn send_request_with_timeout(
        &mut self,
        request: DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, ClientError> {
        let result = tokio::time::timeout(timeout, self.send_request_inner(request)).await;

        match result {
            Ok(inner_result) => inner_result,
            Err(_) => Err(ClientError::Timeout),
        }
    }

    /// Internal request sending (without timeout wrapper)
    async fn send_request_inner(
        &mut self,
        request: DaemonRequest,
    ) -> Result<DaemonResponse, ClientError> {
        // Serialize request
        let request_json = serde_json::to_string(&request)?;
        debug!("Sending request: {}", request_json);

        // Write request
        self.writer.write_all(request_json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        // Read response
        let mut response_line = String::new();
        let bytes_read = self.reader.read_line(&mut response_line).await?;

        if bytes_read == 0 {
            return Err(ClientError::ConnectionClosed);
        }

        debug!("Received response: {}", response_line.trim());

        // Parse response
        let response: DaemonResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    /// Check if the daemon is alive by sending a ping
    pub async fn ping(&mut self) -> Result<(), ClientError> {
        let response = self.send_request(DaemonRequest::Ping).await?;

        match response {
            DaemonResponse::Pong => Ok(()),
            DaemonResponse::Error { message } => Err(ClientError::DaemonError(message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    /// Start a new share for a session
    ///
    /// # Arguments
    /// * `session_path` - Path to the session file to share
    /// * `provider` - Name of the tunnel provider to use (e.g., "cloudflare", "ngrok")
    ///
    /// # Returns
    /// ShareInfo with the details of the started share
    pub async fn start_share(
        &mut self,
        session_path: PathBuf,
        provider: String,
    ) -> Result<ShareInfo, ClientError> {
        let request = DaemonRequest::StartShare {
            session_path,
            provider,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::ShareStarted(info) => Ok(info),
            DaemonResponse::Error { message } => Err(ClientError::DaemonError(message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    /// Stop an existing share
    ///
    /// # Arguments
    /// * `share_id` - ID of the share to stop
    pub async fn stop_share(&mut self, share_id: ShareId) -> Result<(), ClientError> {
        let request = DaemonRequest::StopShare { share_id };
        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::ShareStopped { share_id: _ } => Ok(()),
            DaemonResponse::Error { message } => Err(ClientError::DaemonError(message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    /// List all shares (active and inactive)
    pub async fn list_shares(&mut self) -> Result<Vec<ShareInfo>, ClientError> {
        let response = self.send_request(DaemonRequest::ListShares).await?;

        match response {
            DaemonResponse::ShareList(shares) => Ok(shares),
            DaemonResponse::Error { message } => Err(ClientError::DaemonError(message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }

    /// Request the daemon to shut down
    pub async fn shutdown(&mut self) -> Result<(), ClientError> {
        let response = self.send_request(DaemonRequest::Shutdown).await?;

        match response {
            DaemonResponse::ShuttingDown => Ok(()),
            DaemonResponse::Error { message } => Err(ClientError::DaemonError(message)),
            _ => Err(ClientError::UnexpectedResponse),
        }
    }
}

/// Check if the daemon is currently running
///
/// This is a convenience function that checks if the daemon socket exists
/// and if the PID file indicates a running process.
pub fn daemon_running() -> bool {
    is_daemon_running()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonServer;
    use std::time::Duration;
    use tempfile::TempDir;

    /// Helper to create a test daemon server and return the client connection
    async fn create_test_setup() -> (TempDir, DaemonServer, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");
        let pid_path = temp_dir.path().join("test.pid");
        let db_path = temp_dir.path().join("test.db");

        let server = DaemonServer::with_paths(socket_path.clone(), pid_path, Some(db_path))
            .expect("Failed to create test server");

        (temp_dir, server, socket_path)
    }

    #[tokio::test]
    async fn test_connect_daemon_not_running() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("nonexistent.sock");

        let result = DaemonClient::connect_to(socket_path).await;
        assert!(matches!(result, Err(ClientError::DaemonNotRunning)));
    }

    #[tokio::test]
    async fn test_connect_to_running_daemon() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        // Start the server
        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Connect client
        let result = DaemonClient::connect_to(socket_path).await;
        assert!(result.is_ok());

        // Cleanup
        handle.shutdown();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_ping() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path)
            .await
            .expect("Failed to connect");

        let result = client.ping().await;
        assert!(result.is_ok());

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_list_shares_empty() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path)
            .await
            .expect("Failed to connect");

        let shares = client.list_shares().await.expect("Failed to list shares");
        assert!(shares.is_empty());

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_start_share_invalid_session() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path)
            .await
            .expect("Failed to connect");

        // Try to start a share with a non-existent session file
        let result = client
            .start_share(
                PathBuf::from("/nonexistent/session.jsonl"),
                "cloudflare".to_string(),
            )
            .await;

        assert!(matches!(result, Err(ClientError::DaemonError(_))));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_stop_share_nonexistent() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path)
            .await
            .expect("Failed to connect");

        // Stopping a non-existent share should still succeed (idempotent)
        let share_id = ShareId::new();
        let result = client.stop_share(share_id).await;
        assert!(result.is_ok());

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_shutdown() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let _handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path.clone())
            .await
            .expect("Failed to connect");

        // Request shutdown
        let result = client.shutdown().await;
        assert!(result.is_ok());

        // Give time for shutdown
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Socket should be cleaned up
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn test_multiple_requests_same_connection() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut client = DaemonClient::connect_to(socket_path)
            .await
            .expect("Failed to connect");

        // Send multiple requests on the same connection
        client.ping().await.expect("Ping 1 failed");
        client.list_shares().await.expect("ListShares failed");
        client.ping().await.expect("Ping 2 failed");

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_socket_path_accessor() {
        let (_temp_dir, server, socket_path) = create_test_setup().await;

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = DaemonClient::connect_to(socket_path.clone())
            .await
            .expect("Failed to connect");

        assert_eq!(client.socket_path(), socket_path);

        handle.shutdown();
    }
}
