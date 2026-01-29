//! Cloudflare Quick Tunnel provider
//!
//! Uses cloudflared to create free quick tunnels without authentication.

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// Cloudflare Quick Tunnel provider
///
/// This provider uses the `cloudflared` CLI to create free quick tunnels.
/// No authentication is required for quick tunnels.
pub struct CloudflareTunnel;

impl CloudflareTunnel {
    /// Create a new Cloudflare tunnel provider
    pub fn new() -> Self {
        Self
    }

    /// Get the binary name for cloudflared
    pub fn binary_name() -> &'static str {
        "cloudflared"
    }
}

impl Default for CloudflareTunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelProvider for CloudflareTunnel {
    fn name(&self) -> &'static str {
        "cloudflare"
    }

    fn display_name(&self) -> &'static str {
        "Cloudflare Quick Tunnel"
    }

    fn is_available(&self) -> bool {
        binary_exists(Self::binary_name())
    }

    fn spawn(&self, _port: u16) -> TunnelResult<TunnelHandle> {
        if !self.is_available() {
            return Err(TunnelError::NotAvailable(
                "cloudflared binary not found".to_string(),
            ));
        }

        // Full implementation in Story 7
        Err(TunnelError::NotAvailable(
            "Cloudflare tunnel not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloudflare_name() {
        let provider = CloudflareTunnel::new();
        assert_eq!(provider.name(), "cloudflare");
    }

    #[test]
    fn test_cloudflare_display_name() {
        let provider = CloudflareTunnel::new();
        assert_eq!(provider.display_name(), "Cloudflare Quick Tunnel");
    }

    #[test]
    fn test_cloudflare_binary_name() {
        assert_eq!(CloudflareTunnel::binary_name(), "cloudflared");
    }

    #[test]
    fn test_cloudflare_default() {
        let provider = CloudflareTunnel::default();
        assert_eq!(provider.name(), "cloudflare");
    }

    #[test]
    fn test_spawn_returns_not_implemented() {
        let provider = CloudflareTunnel::new();
        // If cloudflared is not available, we get NotAvailable error
        // If it is available, we get NotAvailable with "not yet implemented"
        let result = provider.spawn(3000);
        assert!(result.is_err());
        match result {
            Err(TunnelError::NotAvailable(_)) => {}
            _ => panic!("Expected NotAvailable error"),
        }
    }
}
