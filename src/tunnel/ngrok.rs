//! ngrok tunnel provider
//!
//! Uses ngrok to create tunnels with both free and authenticated accounts.
//! ngrok runs a local API on port 4040 that we query to get the public URL.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// ngrok tunnel provider
///
/// This provider uses the `ngrok` CLI to create tunnels.
/// Works with both free tier and authenticated accounts.
pub struct NgrokTunnel {
    /// Optional auth token for authenticated usage
    auth_token: Option<String>,
    /// Timeout for waiting for the tunnel URL (default: 30 seconds)
    timeout: Duration,
}

impl NgrokTunnel {
    /// Create a new ngrok tunnel provider
    pub fn new() -> Self {
        Self {
            auth_token: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new ngrok tunnel provider with an auth token
    pub fn with_token(token: String) -> Self {
        Self {
            auth_token: Some(token),
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new ngrok tunnel provider with a custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            auth_token: None,
            timeout,
        }
    }

    /// Get the binary name for ngrok
    pub fn binary_name() -> &'static str {
        "ngrok"
    }

    /// Parse the public URL from ngrok's API response
    ///
    /// ngrok's local API returns JSON in the format:
    /// {"tunnels":[{"public_url":"https://xxxx.ngrok.io",...}],...}
    fn parse_url_from_api_response(response: &str) -> Option<String> {
        // Parse the JSON response to extract the public_url
        // We look for the first tunnel's public_url that starts with https://
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
            if let Some(tunnels) = json.get("tunnels").and_then(|t| t.as_array()) {
                for tunnel in tunnels {
                    if let Some(url) = tunnel.get("public_url").and_then(|u| u.as_str()) {
                        if url.starts_with("https://") {
                            return Some(url.to_string());
                        }
                    }
                }
                // Fall back to http if no https tunnel found
                for tunnel in tunnels {
                    if let Some(url) = tunnel.get("public_url").and_then(|u| u.as_str()) {
                        return Some(url.to_string());
                    }
                }
            }
        }
        None
    }

    /// Parse the public URL from ngrok's stdout output
    ///
    /// ngrok outputs the URL to stdout in newer versions:
    /// "Forwarding https://xxxx.ngrok-free.app -> http://localhost:3000"
    fn parse_url_from_output(line: &str) -> Option<String> {
        // Look for Forwarding line with https URL
        if line.contains("Forwarding") {
            // Find https:// URL
            if let Some(start) = line.find("https://") {
                let url_part = &line[start..];
                // Find the end of the URL (space or end of string)
                let end = url_part
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(url_part.len());
                let url = &url_part[..end];
                if url.contains("ngrok") {
                    return Some(url.to_string());
                }
            }
        }
        None
    }

    /// Query the ngrok API to get the tunnel URL
    ///
    /// ngrok runs a local API server on port 4040 by default
    fn query_api_for_url(&self, start_time: Instant) -> Option<String> {
        // Poll the API until we get a tunnel or timeout
        while start_time.elapsed() < self.timeout {
            // Try to connect to ngrok's API
            if let Ok(output) = Command::new("curl")
                .args(["-s", "http://localhost:4040/api/tunnels"])
                .output()
            {
                if output.status.success() {
                    let response = String::from_utf8_lossy(&output.stdout);
                    if let Some(url) = Self::parse_url_from_api_response(&response) {
                        return Some(url);
                    }
                }
            }
            // Wait a bit before trying again
            std::thread::sleep(Duration::from_millis(500));
        }
        None
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

    fn spawn(&self, port: u16) -> TunnelResult<TunnelHandle> {
        if !self.is_available() {
            return Err(TunnelError::BinaryNotFound(
                "ngrok binary not found in PATH".to_string(),
            ));
        }

        // Build the ngrok command
        let mut cmd = Command::new(Self::binary_name());
        cmd.arg("http").arg(port.to_string());

        // Add auth token if provided
        if let Some(ref token) = self.auth_token {
            cmd.env("NGROK_AUTHTOKEN", token);
        }

        // Run ngrok with output capture
        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        let start_time = Instant::now();

        // Try to get URL from stdout first (newer ngrok versions)
        let stdout = child.stdout.take();
        let mut url: Option<String> = None;

        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);

            // Read a few lines looking for the URL
            // But don't block forever - ngrok may output to API only
            for line in reader.lines().take(50) {
                if start_time.elapsed() > Duration::from_secs(5) {
                    // Stop reading stdout, try API instead
                    break;
                }

                // Check if process has exited
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if !status.success() {
                            return Err(TunnelError::ProcessExited);
                        }
                    }
                    Ok(None) => {} // Still running
                    Err(_) => {}   // Continue trying
                }

                if let Ok(line) = line {
                    if let Some(parsed_url) = Self::parse_url_from_output(&line) {
                        url = Some(parsed_url);
                        break;
                    }
                }
            }
        }

        // If we didn't get URL from stdout, try the API
        if url.is_none() {
            url = self.query_api_for_url(start_time);
        }

        // Check for timeout
        if start_time.elapsed() > self.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TunnelError::Timeout);
        }

        match url {
            Some(url) => Ok(TunnelHandle::new(child, url, self.name())),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                Err(TunnelError::UrlParseFailed)
            }
        }
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
        assert_eq!(provider.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_ngrok_with_token() {
        let provider = NgrokTunnel::with_token("test_token".to_string());
        assert_eq!(provider.name(), "ngrok");
        assert!(provider.auth_token.is_some());
        assert_eq!(provider.auth_token.as_deref(), Some("test_token"));
    }

    #[test]
    fn test_ngrok_with_timeout() {
        let provider = NgrokTunnel::with_timeout(Duration::from_secs(60));
        assert_eq!(provider.timeout, Duration::from_secs(60));
        assert!(provider.auth_token.is_none());
    }

    #[test]
    fn test_ngrok_default_timeout() {
        let provider = NgrokTunnel::new();
        assert_eq!(provider.timeout, Duration::from_secs(30));
    }

    // Tests for URL parsing from stdout
    #[test]
    fn test_parse_url_from_output_basic() {
        let line =
            "Forwarding                    https://abc123.ngrok-free.app -> http://localhost:3000";
        let url = NgrokTunnel::parse_url_from_output(line);
        assert_eq!(url, Some("https://abc123.ngrok-free.app".to_string()));
    }

    #[test]
    fn test_parse_url_from_output_ngrok_io() {
        let line = "Forwarding                    https://abc123.ngrok.io -> http://localhost:3000";
        let url = NgrokTunnel::parse_url_from_output(line);
        assert_eq!(url, Some("https://abc123.ngrok.io".to_string()));
    }

    #[test]
    fn test_parse_url_from_output_no_forwarding() {
        let line = "Session Status                online";
        let url = NgrokTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_from_output_http_only() {
        // We prefer https, so http-only lines without https should not match
        let line = "Forwarding                    http://abc123.ngrok.io -> http://localhost:3000";
        let url = NgrokTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_from_output_non_ngrok_domain() {
        let line = "Forwarding                    https://example.com -> http://localhost:3000";
        let url = NgrokTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    // Tests for URL parsing from API response
    #[test]
    fn test_parse_url_from_api_response_basic() {
        let response = r#"{"tunnels":[{"name":"command_line","public_url":"https://abc123.ngrok-free.app","proto":"https"}]}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        assert_eq!(url, Some("https://abc123.ngrok-free.app".to_string()));
    }

    #[test]
    fn test_parse_url_from_api_response_multiple_tunnels() {
        let response = r#"{"tunnels":[{"name":"http","public_url":"http://abc123.ngrok-free.app","proto":"http"},{"name":"https","public_url":"https://abc123.ngrok-free.app","proto":"https"}]}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        // Should prefer https
        assert_eq!(url, Some("https://abc123.ngrok-free.app".to_string()));
    }

    #[test]
    fn test_parse_url_from_api_response_http_fallback() {
        let response = r#"{"tunnels":[{"name":"http","public_url":"http://abc123.ngrok-free.app","proto":"http"}]}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        // Should fall back to http if no https
        assert_eq!(url, Some("http://abc123.ngrok-free.app".to_string()));
    }

    #[test]
    fn test_parse_url_from_api_response_empty_tunnels() {
        let response = r#"{"tunnels":[]}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_from_api_response_invalid_json() {
        let response = "not valid json";
        let url = NgrokTunnel::parse_url_from_api_response(response);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_from_api_response_missing_tunnels() {
        let response = r#"{"error":"no tunnels"}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_from_api_response_ngrok_io_domain() {
        let response = r#"{"tunnels":[{"name":"command_line","public_url":"https://abc123.ngrok.io","proto":"https"}]}"#;
        let url = NgrokTunnel::parse_url_from_api_response(response);
        assert_eq!(url, Some("https://abc123.ngrok.io".to_string()));
    }

    #[test]
    fn test_spawn_returns_error_when_binary_not_found() {
        let provider = NgrokTunnel::new();
        if !provider.is_available() {
            let result = provider.spawn(3000);
            assert!(matches!(result, Err(TunnelError::BinaryNotFound(_))));
        }
        // If ngrok is installed, we can't easily test the binary not found case
    }
}
