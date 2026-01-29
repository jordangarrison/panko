//! Tailscale serve tunnel provider
//!
//! Uses tailscale serve to share services within a tailnet.

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// Tailscale serve tunnel provider
///
/// This provider uses `tailscale serve` to share services within a tailnet.
/// Requires being logged into a Tailscale account.
pub struct TailscaleTunnel;

impl TailscaleTunnel {
    /// Create a new Tailscale tunnel provider
    pub fn new() -> Self {
        Self
    }

    /// Get the binary name for tailscale
    pub fn binary_name() -> &'static str {
        "tailscale"
    }

    /// Check if tailscale is logged in
    #[allow(dead_code)]
    fn is_logged_in(&self) -> bool {
        use std::process::Command;

        Command::new(Self::binary_name())
            .args(["status", "--json"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl Default for TailscaleTunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelProvider for TailscaleTunnel {
    fn name(&self) -> &'static str {
        "tailscale"
    }

    fn display_name(&self) -> &'static str {
        "Tailscale Serve"
    }

    fn is_available(&self) -> bool {
        // For now, just check if the binary exists
        // Story 10 will add the logged-in check
        binary_exists(Self::binary_name())
    }

    fn spawn(&self, _port: u16) -> TunnelResult<TunnelHandle> {
        if !self.is_available() {
            return Err(TunnelError::NotAvailable(
                "tailscale binary not found".to_string(),
            ));
        }

        // Full implementation in Story 10
        Err(TunnelError::NotAvailable(
            "Tailscale tunnel not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tailscale_name() {
        let provider = TailscaleTunnel::new();
        assert_eq!(provider.name(), "tailscale");
    }

    #[test]
    fn test_tailscale_display_name() {
        let provider = TailscaleTunnel::new();
        assert_eq!(provider.display_name(), "Tailscale Serve");
    }

    #[test]
    fn test_tailscale_binary_name() {
        assert_eq!(TailscaleTunnel::binary_name(), "tailscale");
    }

    #[test]
    fn test_tailscale_default() {
        let provider = TailscaleTunnel::default();
        assert_eq!(provider.name(), "tailscale");
    }

    #[test]
    fn test_spawn_returns_not_implemented() {
        let provider = TailscaleTunnel::new();
        let result = provider.spawn(3000);
        assert!(result.is_err());
        match result {
            Err(TunnelError::NotAvailable(_)) => {}
            _ => panic!("Expected NotAvailable error"),
        }
    }
}
