//! Tunnel providers for sharing sessions
//!
//! This module provides an abstraction for tunnel providers that can expose a local
//! server to the internet. Supported providers include Cloudflare Quick Tunnels,
//! ngrok, and Tailscale serve.

use std::io;
use std::process::{Child, Command};
use thiserror::Error;

mod cloudflare;
mod ngrok;
mod tailscale;

pub use cloudflare::CloudflareTunnel;
pub use ngrok::NgrokTunnel;
pub use tailscale::TailscaleTunnel;

/// Error types for tunnel operations
#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("Tunnel binary not found: {0}")]
    BinaryNotFound(String),

    #[error("Failed to spawn tunnel process: {0}")]
    SpawnFailed(#[from] io::Error),

    #[error("Failed to parse tunnel URL from output")]
    UrlParseFailed,

    #[error("Tunnel process exited unexpectedly")]
    ProcessExited,

    #[error("Timeout waiting for tunnel URL")]
    Timeout,

    #[error("Tunnel provider not available: {0}")]
    NotAvailable(String),
}

/// Result type for tunnel operations
pub type TunnelResult<T> = Result<T, TunnelError>;

/// Handle to a running tunnel subprocess
///
/// When dropped, the tunnel subprocess will be terminated.
pub struct TunnelHandle {
    /// The subprocess running the tunnel
    process: Option<Child>,
    /// The public URL exposed by the tunnel
    pub url: String,
    /// Name of the provider (for logging)
    provider_name: String,
}

impl TunnelHandle {
    /// Create a new tunnel handle
    pub fn new(process: Child, url: String, provider_name: &str) -> Self {
        Self {
            process: Some(process),
            url,
            provider_name: provider_name.to_string(),
        }
    }

    /// Get the public URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the provider name
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// Check if the tunnel process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Process still running
                Err(_) => false,      // Error checking status
            }
        } else {
            false
        }
    }

    /// Stop the tunnel process
    pub fn stop(&mut self) -> io::Result<()> {
        if let Some(ref mut process) = self.process.take() {
            process.kill()?;
            process.wait()?;
        }
        Ok(())
    }
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        if let Some(ref mut process) = self.process.take() {
            // Try to kill the process gracefully
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// Trait for tunnel providers
///
/// Each tunnel provider (Cloudflare, ngrok, Tailscale) implements this trait
/// to provide a consistent interface for spawning tunnels.
pub trait TunnelProvider: Send + Sync {
    /// Get the name of this tunnel provider
    fn name(&self) -> &'static str;

    /// Get the display name for user-facing output
    fn display_name(&self) -> &'static str {
        self.name()
    }

    /// Check if this tunnel provider is available (binary installed)
    fn is_available(&self) -> bool;

    /// Spawn a tunnel to the given local port
    ///
    /// Returns a handle containing the public URL and subprocess handle.
    fn spawn(&self, port: u16) -> TunnelResult<TunnelHandle>;
}

/// Information about an available tunnel provider
#[derive(Debug, Clone)]
pub struct AvailableProvider {
    /// The provider name (e.g., "cloudflare", "ngrok", "tailscale")
    pub name: &'static str,
    /// Human-readable display name
    pub display_name: &'static str,
}

/// Detect which tunnel providers are available on this system
///
/// Checks for the presence of cloudflared, ngrok, and tailscale binaries
/// in the system PATH.
pub fn detect_available_providers() -> Vec<AvailableProvider> {
    let providers: Vec<Box<dyn TunnelProvider>> = vec![
        Box::new(CloudflareTunnel::new()),
        Box::new(NgrokTunnel::new()),
        Box::new(TailscaleTunnel::new()),
    ];

    providers
        .into_iter()
        .filter(|p| p.is_available())
        .map(|p| AvailableProvider {
            name: p.name(),
            display_name: p.display_name(),
        })
        .collect()
}

/// Get a tunnel provider by name
pub fn get_provider(name: &str) -> Option<Box<dyn TunnelProvider>> {
    get_provider_with_config(name, None)
}

/// Get a tunnel provider by name with optional configuration
///
/// # Arguments
/// * `name` - Provider name (cloudflare, ngrok, tailscale)
/// * `ngrok_token` - Optional auth token for ngrok
pub fn get_provider_with_config(
    name: &str,
    ngrok_token: Option<&str>,
) -> Option<Box<dyn TunnelProvider>> {
    match name.to_lowercase().as_str() {
        "cloudflare" | "cloudflared" => Some(Box::new(CloudflareTunnel::new())),
        "ngrok" => {
            if let Some(token) = ngrok_token {
                Some(Box::new(NgrokTunnel::with_token(token.to_string())))
            } else {
                Some(Box::new(NgrokTunnel::new()))
            }
        }
        "tailscale" => Some(Box::new(TailscaleTunnel::new())),
        _ => None,
    }
}

/// Check if a binary exists in PATH
fn binary_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_handle_creation() {
        // We can't easily create a real child process in tests,
        // but we can test the Drop behavior doesn't panic
    }

    #[test]
    fn test_detect_available_providers_returns_list() {
        // This test just verifies the function runs without panicking
        // The actual availability depends on the system
        let providers = detect_available_providers();
        // providers may be empty or have items depending on system
        assert!(providers.len() <= 3);
    }

    #[test]
    fn test_get_provider_cloudflare() {
        let provider = get_provider("cloudflare");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "cloudflare");
    }

    #[test]
    fn test_get_provider_cloudflared() {
        let provider = get_provider("cloudflared");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "cloudflare");
    }

    #[test]
    fn test_get_provider_ngrok() {
        let provider = get_provider("ngrok");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "ngrok");
    }

    #[test]
    fn test_get_provider_tailscale() {
        let provider = get_provider("tailscale");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "tailscale");
    }

    #[test]
    fn test_get_provider_unknown() {
        let provider = get_provider("unknown");
        assert!(provider.is_none());
    }

    #[test]
    fn test_get_provider_case_insensitive() {
        let provider = get_provider("CLOUDFLARE");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "cloudflare");
    }

    #[test]
    fn test_binary_exists_false_for_nonexistent() {
        assert!(!binary_exists("definitely_not_a_real_binary_12345"));
    }

    #[test]
    fn test_get_provider_with_config_ngrok_token() {
        let provider = get_provider_with_config("ngrok", Some("test_token"));
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "ngrok");
    }

    #[test]
    fn test_get_provider_with_config_ngrok_no_token() {
        let provider = get_provider_with_config("ngrok", None);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "ngrok");
    }

    #[test]
    fn test_get_provider_with_config_cloudflare() {
        // Token is ignored for cloudflare
        let provider = get_provider_with_config("cloudflare", Some("ignored"));
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "cloudflare");
    }

    #[test]
    fn test_get_provider_with_config_tailscale() {
        // Token is ignored for tailscale
        let provider = get_provider_with_config("tailscale", None);
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name(), "tailscale");
    }

    #[test]
    fn test_get_provider_with_config_unknown() {
        let provider = get_provider_with_config("unknown", None);
        assert!(provider.is_none());
    }
}
