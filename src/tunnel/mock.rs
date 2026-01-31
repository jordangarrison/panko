//! Mock tunnel provider for testing.
//!
//! Provides a configurable mock implementation of `TunnelProvider` that returns
//! predictable URLs without spawning subprocesses. Supports simulated startup
//! delays and configurable error scenarios.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{TunnelHandle, TunnelProvider, TunnelResult};

/// Counter for generating unique mock URLs.
static MOCK_URL_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Type of error to simulate during spawn.
#[derive(Debug, Clone)]
pub enum MockError {
    /// Simulate a "not available" error with the given message.
    NotAvailable(String),
    /// Simulate a URL parse failure.
    UrlParseFailed,
    /// Simulate a timeout.
    Timeout,
}

/// Configuration for mock tunnel behavior.
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Base URL template. Use `{n}` as placeholder for unique counter.
    /// Default: "https://mock-{n}.example.com"
    pub url_template: String,
    /// Simulated startup delay before returning URL.
    pub startup_delay: Option<Duration>,
    /// If set, spawn() will return this error instead of a handle.
    pub error: Option<MockError>,
    /// Whether the provider reports as available.
    pub available: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            url_template: "https://mock-{n}.example.com".to_string(),
            startup_delay: None,
            error: None,
            available: true,
        }
    }
}

impl MockConfig {
    /// Create a config that returns the given URL.
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            url_template: url.into(),
            ..Default::default()
        }
    }

    /// Create a config with a startup delay.
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            startup_delay: Some(delay),
            ..Default::default()
        }
    }

    /// Create a config that simulates an error on spawn.
    pub fn with_error(error: MockError) -> Self {
        Self {
            error: Some(error),
            ..Default::default()
        }
    }

    /// Create a config where the provider is not available.
    pub fn unavailable() -> Self {
        Self {
            available: false,
            ..Default::default()
        }
    }
}

/// Mock tunnel provider for testing.
///
/// Returns predictable URLs without spawning actual subprocesses.
/// Supports configurable delays and error scenarios for testing.
///
/// # Examples
///
/// ```ignore
/// use panko::tunnel::mock::{MockTunnelProvider, MockConfig};
/// use panko::tunnel::TunnelProvider;
///
/// // Basic usage - returns unique URLs
/// let provider = MockTunnelProvider::new();
/// let handle = provider.spawn(8080).unwrap();
/// println!("URL: {}", handle.url()); // https://mock-1.example.com
///
/// // With custom URL template
/// let config = MockConfig::with_url("https://test-{n}.tunnel.dev");
/// let provider = MockTunnelProvider::with_config(config);
///
/// // With simulated delay
/// let config = MockConfig::with_delay(Duration::from_millis(100));
/// let provider = MockTunnelProvider::with_config(config);
/// ```
#[derive(Debug, Clone)]
pub struct MockTunnelProvider {
    config: Arc<MockConfig>,
}

impl MockTunnelProvider {
    /// Create a new mock provider with default configuration.
    pub fn new() -> Self {
        Self {
            config: Arc::new(MockConfig::default()),
        }
    }

    /// Create a mock provider with custom configuration.
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Generate a URL from the template.
    fn generate_url(&self) -> String {
        let n = MOCK_URL_COUNTER.fetch_add(1, Ordering::SeqCst);
        self.config.url_template.replace("{n}", &n.to_string())
    }
}

impl Default for MockTunnelProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TunnelProvider for MockTunnelProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn display_name(&self) -> &'static str {
        "Mock Tunnel (Test)"
    }

    fn is_available(&self) -> bool {
        self.config.available
    }

    fn spawn(&self, _port: u16) -> TunnelResult<TunnelHandle> {
        // Simulate startup delay if configured
        if let Some(delay) = self.config.startup_delay {
            thread::sleep(delay);
        }

        // Return error if configured
        if let Some(ref err) = self.config.error {
            return Err(match err {
                MockError::NotAvailable(msg) => super::TunnelError::NotAvailable(msg.clone()),
                MockError::UrlParseFailed => super::TunnelError::UrlParseFailed,
                MockError::Timeout => super::TunnelError::Timeout,
            });
        }

        let url = self.generate_url();
        Ok(TunnelHandle::without_process(url, self.name()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider_default() {
        let provider = MockTunnelProvider::new();
        assert_eq!(provider.name(), "mock");
        assert!(provider.is_available());
    }

    #[test]
    fn test_mock_provider_spawn_returns_url() {
        let provider = MockTunnelProvider::new();
        let handle = provider.spawn(8080).unwrap();

        assert!(handle.url().starts_with("https://mock-"));
        assert!(handle.url().ends_with(".example.com"));
        assert_eq!(handle.provider_name(), "mock");
    }

    #[test]
    fn test_mock_provider_unique_urls() {
        let provider = MockTunnelProvider::new();

        let handle1 = provider.spawn(8080).unwrap();
        let handle2 = provider.spawn(8081).unwrap();
        let handle3 = provider.spawn(8082).unwrap();

        // URLs should be unique
        assert_ne!(handle1.url(), handle2.url());
        assert_ne!(handle2.url(), handle3.url());
        assert_ne!(handle1.url(), handle3.url());
    }

    #[test]
    fn test_mock_provider_custom_url_template() {
        let config = MockConfig::with_url("https://custom-{n}.test");
        let provider = MockTunnelProvider::with_config(config);
        let handle = provider.spawn(8080).unwrap();

        assert!(handle.url().starts_with("https://custom-"));
        assert!(handle.url().ends_with(".test"));
    }

    #[test]
    fn test_mock_provider_with_delay() {
        let delay = Duration::from_millis(50);
        let config = MockConfig::with_delay(delay);
        let provider = MockTunnelProvider::with_config(config);

        let start = std::time::Instant::now();
        let _handle = provider.spawn(8080).unwrap();
        let elapsed = start.elapsed();

        // Should have waited at least the delay duration
        assert!(elapsed >= delay);
    }

    #[test]
    fn test_mock_provider_with_error() {
        let config = MockConfig::with_error(MockError::NotAvailable("test".to_string()));
        let provider = MockTunnelProvider::with_config(config);
        let result = provider.spawn(8080);

        assert!(result.is_err());
    }

    #[test]
    fn test_mock_provider_unavailable() {
        let config = MockConfig::unavailable();
        let provider = MockTunnelProvider::with_config(config);

        assert!(!provider.is_available());
    }

    #[test]
    fn test_mock_config_default() {
        let config = MockConfig::default();
        assert!(config.available);
        assert!(config.startup_delay.is_none());
        assert!(config.error.is_none());
        assert_eq!(config.url_template, "https://mock-{n}.example.com");
    }

    #[test]
    fn test_mock_provider_display_name() {
        let provider = MockTunnelProvider::new();
        assert_eq!(provider.display_name(), "Mock Tunnel (Test)");
    }

    #[test]
    fn test_tunnel_handle_without_process() {
        let handle = TunnelHandle::without_process("https://test.com".to_string(), "test");
        assert_eq!(handle.url(), "https://test.com");
        assert_eq!(handle.provider_name(), "test");
        // Should not panic when dropped (no process to kill)
    }

    #[test]
    fn test_tunnel_handle_without_process_is_not_running() {
        let mut handle = TunnelHandle::without_process("https://test.com".to_string(), "test");
        // A handle without a process should report as not running
        assert!(!handle.is_running());
    }

    #[test]
    fn test_tunnel_handle_without_process_stop() {
        let mut handle = TunnelHandle::without_process("https://test.com".to_string(), "test");
        // Should not panic when stopping (no process to kill)
        let result = handle.stop();
        assert!(result.is_ok());
    }
}
