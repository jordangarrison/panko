//! Structured logging and diagnostics for Panko.
//!
//! This module provides tracing-based logging infrastructure for sharing operations.
//! Logs capture all phases (parsing, server start, tunnel spawn, URL detection)
//! with timing. Supports configurable log file output since TUI uses stdout/stderr.

use std::path::Path;

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

/// Log verbosity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// No logging (only errors when log_file is set)
    #[default]
    Quiet,
    /// Normal logging (info level)
    Normal,
    /// Verbose logging (debug level)
    Verbose,
    /// Very verbose logging (trace level)
    Trace,
}

impl Verbosity {
    /// Get the tracing level filter for this verbosity.
    pub fn as_level_filter(&self) -> LevelFilter {
        match self {
            Verbosity::Quiet => LevelFilter::ERROR,
            Verbosity::Normal => LevelFilter::INFO,
            Verbosity::Verbose => LevelFilter::DEBUG,
            Verbosity::Trace => LevelFilter::TRACE,
        }
    }

    /// Get the tracing level for this verbosity.
    pub fn as_level(&self) -> Level {
        match self {
            Verbosity::Quiet => Level::ERROR,
            Verbosity::Normal => Level::INFO,
            Verbosity::Verbose => Level::DEBUG,
            Verbosity::Trace => Level::TRACE,
        }
    }
}

/// Configuration for the logging system.
#[derive(Debug, Clone, Default)]
pub struct LogConfig {
    /// Verbosity level for stderr output.
    pub verbosity: Verbosity,
    /// Optional path to log file.
    pub log_file: Option<String>,
}

/// Guard that must be kept alive for the duration of logging.
///
/// When this guard is dropped, the logging system will flush pending logs.
pub struct LogGuard {
    _file_guard: Option<WorkerGuard>,
}

impl LogGuard {
    fn new(file_guard: Option<WorkerGuard>) -> Self {
        Self {
            _file_guard: file_guard,
        }
    }
}

/// Initialize the logging system.
///
/// Returns a guard that must be kept alive for the duration of logging.
/// When the guard is dropped, pending log entries will be flushed.
///
/// # Arguments
///
/// * `config` - Logging configuration including verbosity and optional log file.
///
/// # Example
///
/// ```ignore
/// use panko::logging::{init_logging, LogConfig, Verbosity};
///
/// let config = LogConfig {
///     verbosity: Verbosity::Verbose,
///     log_file: Some("/tmp/panko.log".to_string()),
/// };
/// let _guard = init_logging(&config);
/// tracing::info!("Logging initialized");
/// ```
pub fn init_logging(config: &LogConfig) -> LogGuard {
    let env_filter = EnvFilter::builder()
        .with_default_directive(config.verbosity.as_level_filter().into())
        .from_env_lossy();

    // Set up file logging if configured
    let (file_layer, file_guard) = if let Some(ref log_file_path) = config.log_file {
        let path = Path::new(log_file_path);
        let parent_dir = path.parent().unwrap_or(Path::new("."));
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("panko.log");

        let file_appender = tracing_appender::rolling::never(parent_dir, filename);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_timer(fmt::time::uptime())
            .with_writer(non_blocking)
            .with_filter(LevelFilter::DEBUG); // Always log at debug level to file

        (Some(file_layer), Some(guard))
    } else {
        (None, None)
    };

    // Set up stderr logging only if verbosity is not Quiet
    let stderr_layer = if config.verbosity != Verbosity::Quiet {
        Some(
            fmt::layer()
                .with_ansi(true)
                .with_target(false)
                .with_timer(fmt::time::uptime())
                .with_writer(std::io::stderr)
                .with_filter(config.verbosity.as_level_filter()),
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    LogGuard::new(file_guard)
}

/// Initialize minimal logging for TUI mode.
///
/// In TUI mode, we only want to log to a file (if configured) since stderr
/// is used by the TUI. This function sets up file-only logging.
///
/// If `RUST_LOG` is set but no log file is configured, logs are written to
/// `/tmp/panko-tui.log` to enable debugging without disrupting the TUI.
pub fn init_tui_logging(log_file: Option<&str>) -> LogGuard {
    // Check if RUST_LOG is set - if so, we should enable file logging even without explicit config
    let rust_log_set = std::env::var("RUST_LOG").is_ok();

    // Determine the log file path: explicit config takes precedence, then RUST_LOG triggers default
    let effective_log_file = match log_file {
        Some(path) => Some(path.to_string()),
        None if rust_log_set => Some("/tmp/panko-tui.log".to_string()),
        None => None,
    };

    let (file_layer, file_guard) = if let Some(log_file_path) = effective_log_file {
        let path = Path::new(&log_file_path);
        let parent_dir = path.parent().unwrap_or(Path::new("."));
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("panko.log");

        let file_appender = tracing_appender::rolling::never(parent_dir, filename);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // Build EnvFilter respecting RUST_LOG, with DEBUG as default
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy();

        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_timer(fmt::time::uptime())
            .with_writer(non_blocking)
            .with_filter(env_filter);

        (Some(file_layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry().with(file_layer).init();

    LogGuard::new(file_guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_default() {
        let v = Verbosity::default();
        assert_eq!(v, Verbosity::Quiet);
    }

    #[test]
    fn test_verbosity_as_level_filter() {
        assert_eq!(Verbosity::Quiet.as_level_filter(), LevelFilter::ERROR);
        assert_eq!(Verbosity::Normal.as_level_filter(), LevelFilter::INFO);
        assert_eq!(Verbosity::Verbose.as_level_filter(), LevelFilter::DEBUG);
        assert_eq!(Verbosity::Trace.as_level_filter(), LevelFilter::TRACE);
    }

    #[test]
    fn test_verbosity_as_level() {
        assert_eq!(Verbosity::Quiet.as_level(), Level::ERROR);
        assert_eq!(Verbosity::Normal.as_level(), Level::INFO);
        assert_eq!(Verbosity::Verbose.as_level(), Level::DEBUG);
        assert_eq!(Verbosity::Trace.as_level(), Level::TRACE);
    }

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.verbosity, Verbosity::Quiet);
        assert!(config.log_file.is_none());
    }
}
