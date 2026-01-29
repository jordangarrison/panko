//! ngrok tunnel provider
//!
//! Uses ngrok to create tunnels with both free and authenticated accounts.

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// ngrok tunnel provider
///
/// This provider uses the `ngrok` CLI to create tunnels.
/// Works with both free tier and authenticated accounts.
pub struct NgrokTunnel {
    /// Optional auth token for authenticated usage
    #[allow(dead_code)]
    auth_token: Option<String>,
}

impl NgrokTunnel {
    /// Create a new ngrok tunnel provider
    pub fn new() -> Self {
        Self { auth_token: None }
    }

    /// Create a new ngrok tunnel provider with an auth token
    #[allow(dead_code)]
    pub fn with_token(token: String) -> Self {
        Self {
            auth_token: Some(token),
        }
    }

    /// Get the binary name for ngrok
    pub fn binary_name() -> &'static str {
        "ngrok"
    }
}

impl Default for NgrokTunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelProvider for NgrokTunnel {
    fn name(&self) -> &'static str {
        "ngrok"
    }

    fn display_name(&self) -> &'static str {
        "ngrok"
    }

    fn is_available(&self) -> bool {
        binary_exists(Self::binary_name())
    }

    fn spawn(&self, _port: u16) -> TunnelResult<TunnelHandle> {
        if !self.is_available() {
            return Err(TunnelError::NotAvailable(
                "ngrok binary not found".to_string(),
            ));
        }

        // Full implementation in Story 9
        Err(TunnelError::NotAvailable(
            "ngrok tunnel not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ngrok_name() {
        let provider = NgrokTunnel::new();
        assert_eq!(provider.name(), "ngrok");
    }

    #[test]
    fn test_ngrok_display_name() {
        let provider = NgrokTunnel::new();
        assert_eq!(provider.display_name(), "ngrok");
    }

    #[test]
    fn test_ngrok_binary_name() {
        assert_eq!(NgrokTunnel::binary_name(), "ngrok");
    }

    #[test]
    fn test_ngrok_default() {
        let provider = NgrokTunnel::default();
        assert_eq!(provider.name(), "ngrok");
    }

    #[test]
    fn test_ngrok_with_token() {
        let provider = NgrokTunnel::with_token("test_token".to_string());
        assert_eq!(provider.name(), "ngrok");
    }

    #[test]
    fn test_spawn_returns_not_implemented() {
        let provider = NgrokTunnel::new();
        let result = provider.spawn(3000);
        assert!(result.is_err());
        match result {
            Err(TunnelError::NotAvailable(_)) => {}
            _ => panic!("Expected NotAvailable error"),
        }
    }
}
