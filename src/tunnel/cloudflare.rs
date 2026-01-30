//! Cloudflare Quick Tunnel provider
//!
//! Uses cloudflared to create free quick tunnels without authentication.

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{binary_exists, TunnelError, TunnelHandle, TunnelProvider, TunnelResult};

/// Cloudflare Quick Tunnel provider
///
/// This provider uses the `cloudflared` CLI to create free quick tunnels.
/// No authentication is required for quick tunnels.
pub struct CloudflareTunnel {
    /// Timeout for waiting for the tunnel URL (default: 30 seconds)
    timeout: Duration,
}

impl CloudflareTunnel {
    /// Create a new Cloudflare tunnel provider
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new Cloudflare tunnel provider with a custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Get the binary name for cloudflared
    pub fn binary_name() -> &'static str {
        "cloudflared"
    }

    /// Parse the public URL from cloudflared output
    ///
    /// Cloudflared outputs the URL to stderr in a format like:
    /// "... https://random-words.trycloudflare.com ..."
    fn parse_url_from_output(line: &str) -> Option<String> {
        // Look for the trycloudflare.com URL pattern
        // The URL appears in the output like:
        // "INF +----------------------------------------------------------+"
        // "INF |  Your quick Tunnel has been created! Visit it at (it may take some time to be reachable):"
        // "INF |  https://random-words.trycloudflare.com"
        // Or sometimes: "INF ... https://random-words.trycloudflare.com ..."

        // Find https:// followed by anything ending in .trycloudflare.com
        if let Some(start) = line.find("https://") {
            let url_part = &line[start..];
            // Find the end of the URL (space, newline, or end of string)
            let end = url_part
                .find(|c: char| c.is_whitespace() || c == '|')
                .unwrap_or(url_part.len());
            let url = &url_part[..end];
            if url.contains("trycloudflare.com") {
                return Some(url.to_string());
            }
        }
        None
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

    fn spawn(&self, port: u16) -> TunnelResult<TunnelHandle> {
        if !self.is_available() {
            return Err(TunnelError::BinaryNotFound(
                "cloudflared binary not found in PATH".to_string(),
            ));
        }

        // Spawn cloudflared tunnel command
        // The --url flag creates a quick tunnel without authentication
        let mut child = Command::new(Self::binary_name())
            .args(["tunnel", "--url", &format!("http://127.0.0.1:{}", port)])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // cloudflared outputs the URL to stderr
        let stderr = child.stderr.take().ok_or_else(|| {
            TunnelError::SpawnFailed(std::io::Error::other("Failed to capture stderr"))
        })?;

        let mut reader = BufReader::new(stderr);
        let start_time = Instant::now();
        let mut url: Option<String> = None;

        // Read stderr line by line looking for the URL
        loop {
            // Check for timeout
            if start_time.elapsed() > self.timeout {
                // Kill the process before returning error
                let _ = child.kill();
                let _ = child.wait();
                return Err(TunnelError::Timeout);
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

            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    // Try to parse URL from this line
                    if let Some(parsed_url) = Self::parse_url_from_output(&line) {
                        url = Some(parsed_url);
                        break;
                    }
                }
                Err(e) => {
                    return Err(TunnelError::SpawnFailed(std::io::Error::other(e)));
                }
            }
        }

        match url {
            Some(url) => {
                // Spawn a thread to keep draining stderr so cloudflared doesn't block/die
                // from SIGPIPE when it tries to write logs
                thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break, // EOF
                            Ok(_) => {}     // Discard the data
                            Err(_) => break,
                        }
                    }
                });

                Ok(TunnelHandle::new(child, url, self.name()))
            }
            None => {
                // Kill the process before returning error
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
    fn test_cloudflare_with_timeout() {
        let provider = CloudflareTunnel::with_timeout(Duration::from_secs(60));
        assert_eq!(provider.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_cloudflare_default_timeout() {
        let provider = CloudflareTunnel::new();
        assert_eq!(provider.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_url_basic() {
        let line = "https://random-words.trycloudflare.com";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(
            url,
            Some("https://random-words.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_parse_url_with_prefix() {
        let line = "INF |  https://cool-tunnel.trycloudflare.com";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(
            url,
            Some("https://cool-tunnel.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_parse_url_with_suffix() {
        let line = "INF https://cool-tunnel.trycloudflare.com |";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(
            url,
            Some("https://cool-tunnel.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_parse_url_in_middle() {
        let line = "some text https://another-tunnel.trycloudflare.com more text";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(
            url,
            Some("https://another-tunnel.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_parse_url_no_match() {
        let line = "INF Starting tunnel";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_wrong_domain() {
        let line = "https://example.com";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_url_http_not_https() {
        // cloudflared always uses https, so http should not match
        let line = "http://test.trycloudflare.com";
        let url = CloudflareTunnel::parse_url_from_output(line);
        assert_eq!(url, None);
    }

    #[test]
    fn test_spawn_returns_error_when_binary_not_found() {
        // Create a provider but use a non-existent binary
        // Note: This test will pass if cloudflared is not installed
        // or will actually spawn a tunnel if it is installed
        let provider = CloudflareTunnel::new();
        if !provider.is_available() {
            let result = provider.spawn(3000);
            assert!(matches!(result, Err(TunnelError::BinaryNotFound(_))));
        }
        // If cloudflared is installed, we can't easily test the binary not found case
        // without mocking, so we skip that case
    }
}
