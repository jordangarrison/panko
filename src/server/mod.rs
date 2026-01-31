//! Web server for viewing sessions.

pub mod assets;
pub mod routes;
pub mod templates;

pub use assets::{content_type, StaticAssets};
pub use routes::{build_router, AppState};
pub use templates::{markdown_to_html, BlockView, SessionView, TemplateEngine, Templates};

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::signal;
use tokio::sync::oneshot;

use crate::parser::Session;

/// Find an available port starting from the given base port.
///
/// Tries ports sequentially until finding one that's available.
/// Returns the available port number.
pub fn find_available_port(base_port: u16) -> Option<u16> {
    (base_port..=base_port + 100).find(|&port| TcpListener::bind(("127.0.0.1", port)).is_ok())
}

/// Configuration for the web server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Base port to try binding to (defaults to 3000).
    pub base_port: u16,
    /// Whether to open the browser automatically.
    pub open_browser: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            base_port: 3000,
            open_browser: true,
        }
    }
}

/// Run the web server with the given session.
///
/// This function will:
/// 1. Find an available port starting from `config.base_port`
/// 2. Start the axum server
/// 3. Optionally open the browser
/// 4. Wait for Ctrl+C to gracefully shut down
///
/// Returns the server address on success.
pub async fn run_server(session: Session, config: ServerConfig) -> anyhow::Result<()> {
    run_server_with_source(session, config, None).await
}

/// Run the web server with the given session and optional source file path.
///
/// This function will:
/// 1. Find an available port starting from `config.base_port`
/// 2. Start the axum server
/// 3. Optionally open the browser
/// 4. Wait for Ctrl+C to gracefully shut down
///
/// Returns the server address on success.
pub async fn run_server_with_source(
    session: Session,
    config: ServerConfig,
    source_path: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let port = find_available_port(config.base_port)
        .ok_or_else(|| anyhow::anyhow!("No available port found"))?;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{}", addr);

    let state = Arc::new(AppState {
        session,
        template_engine: TemplateEngine::default(),
        source_path,
    });

    let app = build_router(state);

    let listener = TokioTcpListener::bind(addr).await?;

    println!("Server running at: {}", url);
    println!("Press Ctrl+C to stop");

    if config.open_browser {
        if let Err(e) = webbrowser::open(&url) {
            eprintln!("Failed to open browser: {}", e);
        }
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    println!("\nServer stopped");
    Ok(())
}

/// Wait for the shutdown signal (Ctrl+C).
pub async fn shutdown_signal() {
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

/// Handle to a running server that can be controlled externally.
pub struct ServerHandle {
    /// The port the server is running on.
    pub port: u16,
    /// The local URL of the server.
    pub local_url: String,
    /// Channel to signal shutdown.
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Handle to the server task.
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ServerHandle {
    /// Get the port the server is running on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the local URL of the server.
    pub fn local_url(&self) -> &str {
        &self.local_url
    }

    /// Stop the server gracefully.
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }
}

/// Start a server that can be controlled externally.
///
/// Returns a ServerHandle that can be used to get the URL and stop the server.
pub async fn start_server(session: Session, config: ServerConfig) -> anyhow::Result<ServerHandle> {
    start_server_with_source(session, config, None).await
}

/// Start a server with optional source path that can be controlled externally.
///
/// Returns a ServerHandle that can be used to get the URL and stop the server.
pub async fn start_server_with_source(
    session: Session,
    config: ServerConfig,
    source_path: Option<std::path::PathBuf>,
) -> anyhow::Result<ServerHandle> {
    let port = find_available_port(config.base_port)
        .ok_or_else(|| anyhow::anyhow!("No available port found"))?;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let local_url = format!("http://{}", addr);

    let state = Arc::new(AppState {
        session,
        template_engine: TemplateEngine::default(),
        source_path,
    });

    let app = build_router(state);
    let listener = TokioTcpListener::bind(addr).await?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let task_handle = tokio::spawn(async move {
        let shutdown = async {
            let _ = shutdown_rx.await;
        };

        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await;
    });

    Ok(ServerHandle {
        port,
        local_url,
        shutdown_tx: Some(shutdown_tx),
        task_handle: Some(task_handle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port() {
        // The first port should generally be available in tests
        let port = find_available_port(49152); // Use ephemeral port range
        assert!(port.is_some());
        let port = port.unwrap();
        assert!(port >= 49152);
    }

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.base_port, 3000);
        assert!(config.open_browser);
    }
}
