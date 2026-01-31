//! Configuration management for panko
//!
//! Handles loading and saving configuration from ~/.config/panko/config.toml

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Configuration file name
const CONFIG_FILE: &str = "config.toml";

/// Application name for config directory
const APP_NAME: &str = "panko";

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),

    #[error("Could not determine config directory")]
    NoConfigDir,
}

/// Result type for config operations
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Config {
    /// Default tunnel provider to use (cloudflare, ngrok, tailscale)
    /// If set, skips the provider selection prompt
    #[serde(default)]
    pub default_provider: Option<String>,

    /// ngrok auth token for authenticated usage
    #[serde(default)]
    pub ngrok_token: Option<String>,

    /// Default port for the web server
    #[serde(default)]
    pub default_port: Option<u16>,

    /// Default sort order for TUI session list
    /// Valid values: date_newest, date_oldest, message_count, project_name
    #[serde(default)]
    pub default_sort: Option<String>,

    /// Maximum number of concurrent shares allowed
    #[serde(default)]
    pub max_shares: Option<usize>,
}

impl Config {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the config file path
    ///
    /// Returns ~/.config/panko/config.toml on Linux/macOS
    pub fn config_path() -> ConfigResult<PathBuf> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join(APP_NAME).join(CONFIG_FILE))
    }

    /// Get the config directory path
    ///
    /// Returns ~/.config/panko on Linux/macOS
    pub fn config_dir() -> ConfigResult<PathBuf> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join(APP_NAME))
    }

    /// Load configuration from file
    ///
    /// Returns default config if file doesn't exist
    pub fn load() -> ConfigResult<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to file
    ///
    /// Creates the config directory if it doesn't exist
    pub fn save(&self) -> ConfigResult<()> {
        let path = Self::config_path()?;
        let dir = Self::config_dir()?;

        // Create config directory if needed
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Check if any configuration is set
    pub fn is_empty(&self) -> bool {
        self.default_provider.is_none()
            && self.ngrok_token.is_none()
            && self.default_port.is_none()
            && self.default_sort.is_none()
            && self.max_shares.is_none()
    }

    /// Set the default tunnel provider
    pub fn set_default_provider(&mut self, provider: Option<String>) {
        self.default_provider = provider;
    }

    /// Set the ngrok auth token
    pub fn set_ngrok_token(&mut self, token: Option<String>) {
        self.ngrok_token = token;
    }

    /// Set the default port
    pub fn set_default_port(&mut self, port: Option<u16>) {
        self.default_port = port;
    }

    /// Set the default sort order
    pub fn set_default_sort(&mut self, sort: Option<String>) {
        self.default_sort = sort;
    }

    /// Set the maximum number of concurrent shares
    pub fn set_max_shares(&mut self, max: Option<usize>) {
        self.max_shares = max;
    }

    /// Get effective max shares (from config or default)
    pub fn effective_max_shares(&self, default: usize) -> usize {
        self.max_shares.unwrap_or(default)
    }

    /// Get effective port (from config or default)
    pub fn effective_port(&self, cli_port: u16) -> u16 {
        // CLI argument takes precedence, then config, then default
        if cli_port != 3000 {
            cli_port
        } else {
            self.default_port.unwrap_or(3000)
        }
    }
}

/// Format the configuration for display
pub fn format_config(config: &Config) -> String {
    let mut lines = Vec::new();

    lines.push("Current configuration:".to_string());
    lines.push(String::new());

    if let Some(ref provider) = config.default_provider {
        lines.push(format!("  default_provider = \"{}\"", provider));
    } else {
        lines.push("  default_provider = (not set)".to_string());
    }

    if config.ngrok_token.is_some() {
        lines.push("  ngrok_token = \"********\" (set)".to_string());
    } else {
        lines.push("  ngrok_token = (not set)".to_string());
    }

    if let Some(port) = config.default_port {
        lines.push(format!("  default_port = {}", port));
    } else {
        lines.push("  default_port = (not set, using 3000)".to_string());
    }

    if let Some(ref sort) = config.default_sort {
        lines.push(format!("  default_sort = \"{}\"", sort));
    } else {
        lines.push("  default_sort = (not set, using date_newest)".to_string());
    }

    if let Some(max) = config.max_shares {
        lines.push(format!("  max_shares = {}", max));
    } else {
        lines.push("  max_shares = (not set, using 5)".to_string());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.default_provider.is_none());
        assert!(config.ngrok_token.is_none());
        assert!(config.default_port.is_none());
        assert!(config.is_empty());
    }

    #[test]
    fn test_config_new() {
        let config = Config::new();
        assert!(config.is_empty());
    }

    #[test]
    fn test_config_setters() {
        let mut config = Config::new();

        config.set_default_provider(Some("cloudflare".to_string()));
        assert_eq!(config.default_provider, Some("cloudflare".to_string()));

        config.set_ngrok_token(Some("token123".to_string()));
        assert_eq!(config.ngrok_token, Some("token123".to_string()));

        config.set_default_port(Some(8080));
        assert_eq!(config.default_port, Some(8080));

        assert!(!config.is_empty());
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let mut config = Config::new();
        config.set_default_provider(Some("ngrok".to_string()));
        config.set_ngrok_token(Some("secret".to_string()));
        config.set_default_port(Some(4000));

        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config, parsed);
    }

    #[test]
    fn test_config_serialize_empty() {
        let config = Config::new();
        let toml_str = toml::to_string(&config).unwrap();

        // Empty config should serialize to minimal TOML
        assert!(!toml_str.contains("default_provider"));
        assert!(!toml_str.contains("ngrok_token"));
        assert!(!toml_str.contains("default_port"));
    }

    #[test]
    fn test_config_deserialize_partial() {
        let toml_str = r#"
            default_provider = "cloudflare"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_provider, Some("cloudflare".to_string()));
        assert!(config.ngrok_token.is_none());
        assert!(config.default_port.is_none());
    }

    #[test]
    fn test_config_deserialize_empty() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn test_effective_port_cli_takes_precedence() {
        let mut config = Config::new();
        config.set_default_port(Some(4000));

        // Non-default CLI port takes precedence
        assert_eq!(config.effective_port(5000), 5000);
    }

    #[test]
    fn test_effective_port_config_used() {
        let mut config = Config::new();
        config.set_default_port(Some(4000));

        // Default CLI port (3000) uses config value
        assert_eq!(config.effective_port(3000), 4000);
    }

    #[test]
    fn test_effective_port_default() {
        let config = Config::new();

        // No config, default CLI port returns 3000
        assert_eq!(config.effective_port(3000), 3000);
    }

    #[test]
    fn test_format_config_empty() {
        let config = Config::new();
        let output = format_config(&config);

        assert!(output.contains("default_provider = (not set)"));
        assert!(output.contains("ngrok_token = (not set)"));
        assert!(output.contains("default_port = (not set, using 3000)"));
    }

    #[test]
    fn test_format_config_with_values() {
        let mut config = Config::new();
        config.set_default_provider(Some("cloudflare".to_string()));
        config.set_ngrok_token(Some("secret".to_string()));
        config.set_default_port(Some(8080));

        let output = format_config(&config);

        assert!(output.contains("default_provider = \"cloudflare\""));
        assert!(output.contains("ngrok_token = \"********\" (set)"));
        assert!(output.contains("default_port = 8080"));
    }

    #[test]
    fn test_config_path() {
        let result = Config::config_path();
        // This should work on most systems
        if let Ok(path) = result {
            assert!(path.to_string_lossy().contains("panko"));
            assert!(path.to_string_lossy().contains("config.toml"));
        }
    }

    #[test]
    fn test_config_dir() {
        let result = Config::config_dir();
        if let Ok(path) = result {
            assert!(path.to_string_lossy().contains("panko"));
        }
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Create a config and manually save it
        let mut config = Config::new();
        config.set_default_provider(Some("tailscale".to_string()));
        config.set_ngrok_token(Some("test_token".to_string()));
        config.set_default_port(Some(9000));

        let contents = toml::to_string_pretty(&config).unwrap();
        fs::write(&config_path, contents).unwrap();

        // Load and verify
        let loaded_contents = fs::read_to_string(&config_path).unwrap();
        let loaded: Config = toml::from_str(&loaded_contents).unwrap();

        assert_eq!(loaded.default_provider, Some("tailscale".to_string()));
        assert_eq!(loaded.ngrok_token, Some("test_token".to_string()));
        assert_eq!(loaded.default_port, Some(9000));
    }

    #[test]
    fn test_default_sort_getter_setter() {
        let mut config = Config::new();
        assert!(config.default_sort.is_none());

        config.set_default_sort(Some("date_newest".to_string()));
        assert_eq!(config.default_sort, Some("date_newest".to_string()));

        config.set_default_sort(None);
        assert!(config.default_sort.is_none());
    }

    #[test]
    fn test_default_sort_serialization() {
        let mut config = Config::new();
        config.set_default_sort(Some("message_count".to_string()));

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("default_sort"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.default_sort, Some("message_count".to_string()));
    }

    #[test]
    fn test_format_config_with_sort() {
        let mut config = Config::new();
        config.set_default_sort(Some("project_name".to_string()));

        let output = format_config(&config);
        assert!(output.contains("default_sort = \"project_name\""));
    }

    #[test]
    fn test_format_config_without_sort() {
        let config = Config::new();
        let output = format_config(&config);
        assert!(output.contains("default_sort = (not set, using date_newest)"));
    }

    #[test]
    fn test_is_empty_with_only_sort() {
        let mut config = Config::new();
        assert!(config.is_empty());

        config.set_default_sort(Some("date_oldest".to_string()));
        assert!(!config.is_empty());
    }

    #[test]
    fn test_max_shares_getter_setter() {
        let mut config = Config::new();
        assert!(config.max_shares.is_none());

        config.set_max_shares(Some(10));
        assert_eq!(config.max_shares, Some(10));

        config.set_max_shares(None);
        assert!(config.max_shares.is_none());
    }

    #[test]
    fn test_max_shares_serialization() {
        let mut config = Config::new();
        config.set_max_shares(Some(3));

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("max_shares"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.max_shares, Some(3));
    }

    #[test]
    fn test_format_config_with_max_shares() {
        let mut config = Config::new();
        config.set_max_shares(Some(10));

        let output = format_config(&config);
        assert!(output.contains("max_shares = 10"));
    }

    #[test]
    fn test_format_config_without_max_shares() {
        let config = Config::new();
        let output = format_config(&config);
        assert!(output.contains("max_shares = (not set, using 5)"));
    }

    #[test]
    fn test_effective_max_shares_uses_config() {
        let mut config = Config::new();
        config.set_max_shares(Some(10));

        assert_eq!(config.effective_max_shares(5), 10);
    }

    #[test]
    fn test_effective_max_shares_uses_default() {
        let config = Config::new();
        assert_eq!(config.effective_max_shares(5), 5);
    }

    #[test]
    fn test_is_empty_with_only_max_shares() {
        let mut config = Config::new();
        assert!(config.is_empty());

        config.set_max_shares(Some(3));
        assert!(!config.is_empty());
    }
}
