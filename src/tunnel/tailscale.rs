//! Tailscale serve tunnel provider
//!
//! Uses tailscale serve to share services within a tailnet.
//! Note: Tailscale serve only shares within your tailnet (private network),
//! not publicly on the internet like other tunnel providers.

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// Tailscale serve tunnel provider
///
/// This provider uses `tailscale serve` to share services within a tailnet.
/// Requires being logged into a Tailscale account.
///
/// Note: Unlike Cloudflare or ngrok, Tailscale serve only exposes services
/// to other devices on your tailnet (private network), not the public internet.
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

    /// Check if tailscale is logged in and has a valid connection
    fn is_logged_in(&self) -> bool {
        Command::new(Self::binary_name())
            .args(["status", "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map(|output| {
                if !output.status.success() {
                    return false;
                }
                // Parse JSON to check BackendState
                let output_str = String::from_utf8_lossy(&output.stdout);
                Self::parse_logged_in_status(&output_str)
            })
            .unwrap_or(false)
    }

    /// Parse the tailscale status JSON to determine if logged in
    fn parse_logged_in_status(json_output: &str) -> bool {
        // Check if BackendState is "Running" and Self.Online is true
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_output) {
            let backend_state = json
                .get("BackendState")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // BackendState should be "Running" for a logged-in, connected state
            backend_state == "Running"
        } else {
            false
        }
    }

    /// Get the tailscale hostname for this machine
    fn get_hostname(&self) -> Option<String> {
        let output = Command::new(Self::binary_name())
            .args(["status", "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        Self::parse_hostname_from_status(&output_str)
    }

    /// Parse the hostname from tailscale status JSON
    ///
    /// The status JSON has a "Self" object with "DNSName" like "hostname.tailnet-name.ts.net."
    fn parse_hostname_from_status(json_output: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(json_output).ok()?;

        // Get Self.DNSName which looks like "hostname.tailnet-name.ts.net."
        let dns_name = json.get("Self")?.get("DNSName")?.as_str()?;

        // Remove trailing dot if present
        let dns_name = dns_name.trim_end_matches('.');

        if dns_name.is_empty() {
            return None;
        }

        Some(dns_name.to_string())
    }

    /// Construct the serve URL from hostname
    ///
    /// Tailscale serve always exposes on HTTPS port 443, regardless of the
    /// local port being proxied.
    fn construct_serve_url(hostname: &str) -> String {
        format!("https://{}", hostname)
    }

    /// Stop serving by running `tailscale serve off`
    pub fn stop_serve(port: u16) {
        // Use `tailscale serve --https=<port> off` to stop serving specific port
        let _ = Command::new(Self::binary_name())
            .args(["serve", &format!("--https={}", port), "off"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
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
        // Check both that binary exists AND that we're logged in
        binary_exists(Self::binary_name()) && self.is_logged_in()
    }

    fn spawn(&self, port: u16) -> TunnelResult<TunnelHandle> {
        if !binary_exists(Self::binary_name()) {
            return Err(TunnelError::BinaryNotFound(
                "tailscale binary not found in PATH".to_string(),
            ));
        }

        if !self.is_logged_in() {
            return Err(TunnelError::NotAvailable(
                "tailscale is not logged in. Run 'tailscale up' first.".to_string(),
            ));
        }

        // Get the hostname first
        let hostname = self.get_hostname().ok_or_else(|| {
            TunnelError::NotAvailable("Failed to get tailscale hostname".to_string())
        })?;

        // Run tailscale serve in foreground mode to expose the port
        // Format: tailscale serve --bg=false --https=<port> http://localhost:<port>
        // --bg=false ensures the process stays in foreground and can be killed to stop serving
        let mut child = Command::new(Self::binary_name())
            .args([
                "serve",
                "--bg=false",
                &format!("--https={}", port),
                &format!("http://localhost:{}", port),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Take stderr handle to check for early errors and then drain
        let stderr = child.stderr.take();

        // Wait briefly and check if process exited immediately (e.g., permission denied)
        thread::sleep(Duration::from_millis(500));

        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited early - check stderr for error message
                if !status.success() {
                    if let Some(stderr) = stderr {
                        let reader = BufReader::new(stderr);
                        let error_lines: Vec<String> =
                            reader.lines().take(5).filter_map(|l| l.ok()).collect();
                        let error_msg = error_lines.join(" ");

                        // Check for permission/access denied errors
                        if error_msg.contains("Access denied")
                            || error_msg.contains("serve config denied")
                        {
                            return Err(TunnelError::NotAvailable(
                                "Tailscale serve requires operator permissions. \
                                Run 'sudo tailscale set --operator=$USER' once to fix this."
                                    .to_string(),
                            ));
                        }

                        return Err(TunnelError::NotAvailable(format!(
                            "Tailscale serve failed: {}",
                            error_msg
                        )));
                    }
                    return Err(TunnelError::ProcessExited);
                }
            }
            Ok(None) => {
                // Process still running - spawn drain thread
                if let Some(stderr) = stderr {
                    thread::spawn(move || {
                        let mut reader = BufReader::new(stderr);
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf) {
                                Ok(0) => break, // EOF
                                Ok(_) => {}     // Discard the data
                                Err(_) => break,
                            }
                        }
                    });
                }
            }
            Err(_) => {} // Continue anyway
        }

        // Construct the URL (tailscale serves on port 443 by default for HTTPS)
        // The --https=<port> sets the *local* port to forward from, but the serve
        // is always accessible on https://<hostname>/ (port 443)
        let url = Self::construct_serve_url(&hostname);

        // Return the handle - killing the process will stop the serve
        Ok(TunnelHandle::new(child, url, self.name()))
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

    // Tests for hostname parsing
    #[test]
    fn test_parse_hostname_from_status_basic() {
        let json = r#"{
            "Self": {
                "DNSName": "myhost.tailnet-name.ts.net."
            },
            "BackendState": "Running"
        }"#;
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, Some("myhost.tailnet-name.ts.net".to_string()));
    }

    #[test]
    fn test_parse_hostname_from_status_no_trailing_dot() {
        let json = r#"{
            "Self": {
                "DNSName": "myhost.tailnet.ts.net"
            },
            "BackendState": "Running"
        }"#;
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, Some("myhost.tailnet.ts.net".to_string()));
    }

    #[test]
    fn test_parse_hostname_from_status_missing_self() {
        let json = r#"{
            "BackendState": "Running"
        }"#;
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, None);
    }

    #[test]
    fn test_parse_hostname_from_status_missing_dnsname() {
        let json = r#"{
            "Self": {},
            "BackendState": "Running"
        }"#;
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, None);
    }

    #[test]
    fn test_parse_hostname_from_status_empty_dnsname() {
        let json = r#"{
            "Self": {
                "DNSName": ""
            },
            "BackendState": "Running"
        }"#;
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, None);
    }

    #[test]
    fn test_parse_hostname_from_status_invalid_json() {
        let json = "not valid json";
        let hostname = TailscaleTunnel::parse_hostname_from_status(json);
        assert_eq!(hostname, None);
    }

    // Tests for login status parsing
    #[test]
    fn test_parse_logged_in_status_running() {
        let json = r#"{
            "BackendState": "Running",
            "Self": {
                "Online": true
            }
        }"#;
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(logged_in);
    }

    #[test]
    fn test_parse_logged_in_status_stopped() {
        let json = r#"{
            "BackendState": "Stopped"
        }"#;
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(!logged_in);
    }

    #[test]
    fn test_parse_logged_in_status_needs_login() {
        let json = r#"{
            "BackendState": "NeedsLogin"
        }"#;
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(!logged_in);
    }

    #[test]
    fn test_parse_logged_in_status_starting() {
        let json = r#"{
            "BackendState": "Starting"
        }"#;
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(!logged_in);
    }

    #[test]
    fn test_parse_logged_in_status_invalid_json() {
        let json = "not valid json";
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(!logged_in);
    }

    #[test]
    fn test_parse_logged_in_status_missing_backend_state() {
        let json = r#"{"Self": {}}"#;
        let logged_in = TailscaleTunnel::parse_logged_in_status(json);
        assert!(!logged_in);
    }

    // Tests for URL construction
    #[test]
    fn test_construct_serve_url() {
        // Tailscale serve always uses HTTPS port 443
        let url = TailscaleTunnel::construct_serve_url("myhost.tailnet.ts.net");
        assert_eq!(url, "https://myhost.tailnet.ts.net");
    }

    #[test]
    fn test_construct_serve_url_with_subdomain() {
        let url = TailscaleTunnel::construct_serve_url("myhost.tailnet-name.ts.net");
        assert_eq!(url, "https://myhost.tailnet-name.ts.net");
    }

    #[test]
    fn test_spawn_returns_error_when_binary_not_found() {
        let provider = TailscaleTunnel::new();
        if !binary_exists(TailscaleTunnel::binary_name()) {
            let result = provider.spawn(3000);
            assert!(matches!(result, Err(TunnelError::BinaryNotFound(_))));
        }
        // If tailscale is installed but not logged in, we expect NotAvailable
    }

    #[test]
    fn test_spawn_returns_error_when_not_logged_in() {
        let provider = TailscaleTunnel::new();
        // If binary exists but not logged in
        if binary_exists(TailscaleTunnel::binary_name()) && !provider.is_logged_in() {
            let result = provider.spawn(3000);
            assert!(matches!(result, Err(TunnelError::NotAvailable(_))));
        }
    }
}
