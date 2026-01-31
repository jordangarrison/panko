//! Daemon server for managing persistent shares
//!
//! The daemon listens on a Unix socket and handles IPC requests from the TUI.
//! Shares managed by the daemon persist across TUI restarts.

use crate::daemon::db::{Database, DatabaseError};
use crate::daemon::protocol::{DaemonRequest, DaemonResponse, ShareId, ShareInfo, ShareStatus};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Errors that can occur in the daemon server
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Socket already exists at {0}")]
    SocketExists(PathBuf),

    #[error("Failed to acquire lock")]
    LockError,

    #[error("Server shutdown")]
    Shutdown,
}

/// Daemon server state
pub struct DaemonServer {
    /// Database handle for persistence (using std::sync::Mutex for rusqlite compatibility)
    db: Arc<Mutex<Database>>,
    /// Path to Unix socket
    socket_path: PathBuf,
    /// Path to PID file
    pid_path: PathBuf,
    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,
}

/// Handle to control a running daemon server
pub struct DaemonHandle {
    shutdown_tx: broadcast::Sender<()>,
}

impl DaemonHandle {
    /// Signal the daemon to shut down gracefully
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

impl DaemonServer {
    /// Create a new daemon server with default paths
    pub fn new() -> Result<Self, ServerError> {
        let base_path = default_daemon_dir();
        Self::with_paths(
            base_path.join("daemon.sock"),
            base_path.join("daemon.pid"),
            None,
        )
    }

    /// Create a daemon server with custom paths
    pub fn with_paths(
        socket_path: PathBuf,
        pid_path: PathBuf,
        db_path: Option<PathBuf>,
    ) -> Result<Self, ServerError> {
        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open or create database
        let db = match db_path {
            Some(path) => Database::open(&path)?,
            None => Database::open_default()?,
        };

        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            socket_path,
            pid_path,
            shutdown_tx,
        })
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get the PID file path
    pub fn pid_path(&self) -> &Path {
        &self.pid_path
    }

    /// Run the daemon server
    ///
    /// This will bind to the Unix socket, write the PID file, and start
    /// accepting connections. The server runs until a shutdown signal is
    /// received (SIGTERM/SIGINT or via IPC Shutdown request).
    pub async fn run(&self) -> Result<DaemonHandle, ServerError> {
        // Check if socket already exists
        if self.socket_path.exists() {
            // Try to connect to see if daemon is already running
            if UnixStream::connect(&self.socket_path).await.is_ok() {
                return Err(ServerError::SocketExists(self.socket_path.clone()));
            }
            // Stale socket, remove it
            std::fs::remove_file(&self.socket_path)?;
        }

        // Write PID file
        self.write_pid_file()?;

        // Bind to Unix socket
        let listener = UnixListener::bind(&self.socket_path)?;
        info!("Daemon listening on {:?}", self.socket_path);

        // Create handle for external control
        let handle = DaemonHandle {
            shutdown_tx: self.shutdown_tx.clone(),
        };

        // Clone for the accept loop
        let db = Arc::clone(&self.db);
        let socket_path = self.socket_path.clone();
        let pid_path = self.pid_path.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Spawn the accept loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Accept new connections
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, _addr)) => {
                                debug!("Accepted new connection");
                                let db = Arc::clone(&db);
                                let mut conn_shutdown_rx = shutdown_rx.resubscribe();

                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, db, &mut conn_shutdown_rx).await {
                                        match e {
                                            ServerError::Shutdown => {
                                                debug!("Connection closed due to shutdown");
                                            }
                                            _ => {
                                                error!("Connection error: {}", e);
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Failed to accept connection: {}", e);
                            }
                        }
                    }

                    // Handle shutdown signal
                    _ = shutdown_rx.recv() => {
                        info!("Received shutdown signal, stopping daemon");
                        break;
                    }

                    // Handle OS signals
                    _ = shutdown_signal() => {
                        info!("Received OS signal, stopping daemon");
                        break;
                    }
                }
            }

            // Cleanup
            cleanup_daemon(&socket_path, &pid_path);
            info!("Daemon stopped");
        });

        Ok(handle)
    }

    /// Write the PID file
    fn write_pid_file(&self) -> Result<(), ServerError> {
        let pid = std::process::id();
        if let Some(parent) = self.pid_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.pid_path, pid.to_string())?;
        debug!("Wrote PID file: {:?} (pid={})", self.pid_path, pid);
        Ok(())
    }
}

impl Default for DaemonServer {
    fn default() -> Self {
        Self::new().expect("Failed to create default daemon server")
    }
}

/// Handle a single client connection
async fn handle_connection(
    stream: UnixStream,
    db: Arc<Mutex<Database>>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<(), ServerError> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();

        tokio::select! {
            // Read a line from the client
            read_result = reader.read_line(&mut line) => {
                match read_result {
                    Ok(0) => {
                        // EOF, client disconnected
                        debug!("Client disconnected");
                        return Ok(());
                    }
                    Ok(_) => {
                        // Parse and handle the request
                        let response = match serde_json::from_str::<DaemonRequest>(&line) {
                            Ok(request) => {
                                debug!("Received request: {:?}", request);
                                handle_request(request, &db)
                            }
                            Err(e) => {
                                warn!("Failed to parse request: {}", e);
                                DaemonResponse::Error {
                                    message: format!("Invalid request: {}", e),
                                }
                            }
                        };

                        // Check if we should shutdown
                        let should_shutdown = matches!(response, DaemonResponse::ShuttingDown);

                        // Send response
                        let response_json = serde_json::to_string(&response)?;
                        writer.write_all(response_json.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;

                        if should_shutdown {
                            return Err(ServerError::Shutdown);
                        }
                    }
                    Err(e) => {
                        error!("Read error: {}", e);
                        return Err(e.into());
                    }
                }
            }

            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                return Err(ServerError::Shutdown);
            }
        }
    }
}

/// Handle a daemon request and return a response
fn handle_request(request: DaemonRequest, db: &Arc<Mutex<Database>>) -> DaemonResponse {
    match request {
        DaemonRequest::Ping => {
            debug!("Handling Ping request");
            DaemonResponse::Pong
        }

        DaemonRequest::Shutdown => {
            info!("Handling Shutdown request");
            DaemonResponse::ShuttingDown
        }

        DaemonRequest::ListShares => {
            debug!("Handling ListShares request");
            match list_shares(db) {
                Ok(shares) => DaemonResponse::ShareList(shares),
                Err(e) => DaemonResponse::Error {
                    message: format!("Failed to list shares: {}", e),
                },
            }
        }

        DaemonRequest::StartShare {
            session_path,
            provider,
        } => {
            debug!(
                "Handling StartShare request for {:?} with provider {}",
                session_path, provider
            );
            // Note: Actual share starting will be implemented in share_service.rs
            // For now, we create a placeholder share record
            match start_share_placeholder(db, session_path, provider) {
                Ok(info) => DaemonResponse::ShareStarted(info),
                Err(e) => DaemonResponse::Error {
                    message: format!("Failed to start share: {}", e),
                },
            }
        }

        DaemonRequest::StopShare { share_id } => {
            debug!("Handling StopShare request for {}", share_id);
            match stop_share(db, share_id) {
                Ok(()) => DaemonResponse::ShareStopped { share_id },
                Err(e) => DaemonResponse::Error {
                    message: format!("Failed to stop share: {}", e),
                },
            }
        }
    }
}

/// List all shares from the database
fn list_shares(db: &Arc<Mutex<Database>>) -> Result<Vec<ShareInfo>, ServerError> {
    let db = db.lock().map_err(|_| ServerError::LockError)?;
    let shares = db.list_shares(None)?;
    Ok(shares)
}

/// Create a placeholder share (actual implementation in share_service.rs)
fn start_share_placeholder(
    db: &Arc<Mutex<Database>>,
    session_path: PathBuf,
    provider: String,
) -> Result<ShareInfo, ServerError> {
    let session_name = session_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let info = ShareInfo {
        id: ShareId::new(),
        session_path,
        session_name,
        public_url: "placeholder://pending".to_string(),
        provider_name: provider,
        local_port: 0,
        started_at: chrono::Utc::now(),
        status: ShareStatus::Starting,
    };

    {
        let db = db.lock().map_err(|_| ServerError::LockError)?;
        db.insert_share(&info)?;
    }

    Ok(info)
}

/// Stop a share
fn stop_share(db: &Arc<Mutex<Database>>, share_id: ShareId) -> Result<(), ServerError> {
    let db = db.lock().map_err(|_| ServerError::LockError)?;
    db.update_share_status(share_id, ShareStatus::Stopped)?;
    Ok(())
}

/// Clean up daemon files on shutdown
fn cleanup_daemon(socket_path: &Path, pid_path: &Path) {
    if let Err(e) = std::fs::remove_file(socket_path) {
        warn!("Failed to remove socket file: {}", e);
    }
    if let Err(e) = std::fs::remove_file(pid_path) {
        warn!("Failed to remove PID file: {}", e);
    }
}

/// Wait for shutdown signal (SIGTERM or SIGINT)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Get the default daemon directory
pub fn default_daemon_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("panko")
}

/// Get the default socket path
pub fn default_socket_path() -> PathBuf {
    default_daemon_dir().join("daemon.sock")
}

/// Get the default PID file path
pub fn default_pid_path() -> PathBuf {
    default_daemon_dir().join("daemon.pid")
}

/// Check if a daemon is already running by reading the PID file
pub fn is_daemon_running() -> bool {
    let pid_path = default_pid_path();
    if !pid_path.exists() {
        return false;
    }

    // Read PID from file
    let pid_str = match std::fs::read_to_string(&pid_path) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Check if process is running
    #[cfg(unix)]
    {
        // On Unix, we can check if process exists by sending signal 0
        use std::process::Command;
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, just check if the socket exists and is connectable
        let socket_path = default_socket_path();
        std::net::UnixStream::connect(&socket_path).is_ok()
    }
}

/// Read the daemon PID from the PID file
pub fn read_daemon_pid() -> Option<u32> {
    let pid_path = default_pid_path();
    std::fs::read_to_string(&pid_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    async fn create_test_server(temp_dir: &TempDir) -> DaemonServer {
        let socket_path = temp_dir.path().join("test.sock");
        let pid_path = temp_dir.path().join("test.pid");
        let db_path = temp_dir.path().join("test.db");

        DaemonServer::with_paths(socket_path, pid_path, Some(db_path))
            .expect("Failed to create test server")
    }

    #[tokio::test]
    async fn test_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        assert!(server.socket_path().ends_with("test.sock"));
        assert!(server.pid_path().ends_with("test.pid"));
    }

    #[tokio::test]
    async fn test_server_run_and_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();
        let pid_path = server.pid_path().to_path_buf();

        // Start the server
        let handle = server.run().await.expect("Failed to start server");

        // Give it time to bind
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check socket exists
        assert!(socket_path.exists());
        assert!(pid_path.exists());

        // Shutdown
        handle.shutdown();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check cleanup
        assert!(!socket_path.exists());
        assert!(!pid_path.exists());
    }

    #[tokio::test]
    async fn test_server_ping_pong() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Connect and send ping
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let request = DaemonRequest::Ping;
        let request_json = serde_json::to_string(&request).unwrap();
        stream.write_all(request_json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        // Read response
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        assert!(matches!(response, DaemonResponse::Pong));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_list_shares_empty() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let request = DaemonRequest::ListShares;
        let request_json = serde_json::to_string(&request).unwrap();
        stream.write_all(request_json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        match response {
            DaemonResponse::ShareList(shares) => {
                assert!(shares.is_empty());
            }
            _ => panic!("Expected ShareList response"),
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_start_share_placeholder() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let request = DaemonRequest::StartShare {
            session_path: PathBuf::from("/test/session.jsonl"),
            provider: "cloudflare".to_string(),
        };
        let request_json = serde_json::to_string(&request).unwrap();
        stream.write_all(request_json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        match response {
            DaemonResponse::ShareStarted(info) => {
                assert_eq!(info.session_name, "session");
                assert_eq!(info.provider_name, "cloudflare");
                assert_eq!(info.status, ShareStatus::Starting);
            }
            _ => panic!("Expected ShareStarted response"),
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_stop_share() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // First, start a share
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let start_request = DaemonRequest::StartShare {
            session_path: PathBuf::from("/test/session.jsonl"),
            provider: "cloudflare".to_string(),
        };
        let request_json = serde_json::to_string(&start_request).unwrap();
        stream.write_all(request_json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        let share_id = match response {
            DaemonResponse::ShareStarted(info) => info.id,
            _ => panic!("Expected ShareStarted response"),
        };

        // Now stop the share
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        let stop_request = DaemonRequest::StopShare { share_id };
        let request_json = serde_json::to_string(&stop_request).unwrap();
        stream.write_all(request_json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        match response {
            DaemonResponse::ShareStopped { share_id: id } => {
                assert_eq!(id, share_id);
            }
            _ => panic!("Expected ShareStopped response"),
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_invalid_request() {
        let temp_dir = TempDir::new().unwrap();
        let server = create_test_server(&temp_dir).await;
        let socket_path = server.socket_path().to_path_buf();

        let handle = server.run().await.expect("Failed to start server");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();

        // Send invalid JSON
        stream.write_all(b"not valid json\n").await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();

        let response: DaemonResponse = serde_json::from_str(&response_line).unwrap();
        match response {
            DaemonResponse::Error { message } => {
                assert!(message.contains("Invalid request"));
            }
            _ => panic!("Expected Error response"),
        }

        handle.shutdown();
    }

    #[test]
    fn test_default_paths() {
        let socket_path = default_socket_path();
        let pid_path = default_pid_path();
        let daemon_dir = default_daemon_dir();

        assert!(socket_path.ends_with("daemon.sock"));
        assert!(pid_path.ends_with("daemon.pid"));
        assert!(daemon_dir.ends_with("panko"));
    }
}
