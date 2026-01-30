//! Session scanner for discovering AI coding agent sessions.
//!
//! This module provides a lightweight way to discover and list sessions
//! without fully parsing their contents. It's designed for fast directory
//! scanning and metadata extraction.

mod claude;

pub use claude::ClaudeScanner;

use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Metadata about a discovered session.
///
/// This struct contains lightweight information that can be extracted
/// quickly without parsing the full session content.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionMeta {
    /// Unique identifier for the session (usually the filename without extension).
    pub id: String,
    /// Full path to the session file.
    pub path: PathBuf,
    /// Project path this session belongs to (e.g., "~/projects/api-server").
    pub project_path: String,
    /// When the session was last modified.
    pub updated_at: DateTime<Utc>,
    /// Number of messages in the session.
    pub message_count: usize,
    /// Preview of the first user prompt (truncated to ~100 chars).
    pub first_prompt_preview: Option<String>,
}

impl SessionMeta {
    /// Create a new SessionMeta with the required fields.
    pub fn new(
        id: impl Into<String>,
        path: PathBuf,
        project_path: impl Into<String>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            path,
            project_path: project_path.into(),
            updated_at,
            message_count: 0,
            first_prompt_preview: None,
        }
    }

    /// Set the message count.
    pub fn with_message_count(mut self, count: usize) -> Self {
        self.message_count = count;
        self
    }

    /// Set the first prompt preview.
    pub fn with_first_prompt_preview(mut self, preview: impl Into<String>) -> Self {
        self.first_prompt_preview = Some(preview.into());
        self
    }
}

/// Errors that can occur during session scanning.
#[derive(Debug, Error)]
pub enum ScanError {
    /// Error reading a directory.
    #[error("failed to read directory {path}: {source}")]
    DirectoryRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Error reading a file.
    #[error("failed to read file {path}: {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Error getting file metadata.
    #[error("failed to get metadata for {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl ScanError {
    /// Create a directory read error.
    pub fn directory_read(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::DirectoryRead {
            path: path.into(),
            source,
        }
    }

    /// Create a file read error.
    pub fn file_read(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::FileRead {
            path: path.into(),
            source,
        }
    }

    /// Create a metadata error.
    pub fn metadata(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Metadata {
            path: path.into(),
            source,
        }
    }
}

/// A scanner for discovering AI coding agent sessions.
///
/// Implementations of this trait scan directories for session files
/// and extract lightweight metadata without full parsing.
///
/// # Example
///
/// ```ignore
/// use panko::scanner::{SessionScanner, ClaudeScanner};
///
/// let scanner = ClaudeScanner::new();
/// for root in scanner.default_roots() {
///     match scanner.scan_directory(&root) {
///         Ok(sessions) => {
///             for session in sessions {
///                 println!("{}: {} messages", session.id, session.message_count);
///             }
///         }
///         Err(e) => eprintln!("Failed to scan {}: {}", root.display(), e),
///     }
/// }
/// ```
pub trait SessionScanner: Send + Sync {
    /// Returns the name of this scanner (e.g., "claude", "codex").
    fn name(&self) -> &'static str;

    /// Scan a directory for sessions and return their metadata.
    ///
    /// This method should be efficient and not fully parse session contents.
    /// Corrupted or unreadable files should be skipped with a warning logged.
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to scan.
    ///
    /// # Returns
    ///
    /// A vector of session metadata, or an error if the directory cannot be read.
    fn scan_directory(&self, root: &Path) -> Result<Vec<SessionMeta>, ScanError>;

    /// Returns the default directories to scan for this agent type.
    ///
    /// For example, Claude Code sessions are stored in `~/.claude/projects/`.
    fn default_roots(&self) -> Vec<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn test_session_meta_creation() {
        let ts = test_timestamp();
        let meta = SessionMeta::new(
            "session-123",
            PathBuf::from("/path/to/session.jsonl"),
            "~/projects/api-server",
            ts,
        );

        assert_eq!(meta.id, "session-123");
        assert_eq!(meta.path, PathBuf::from("/path/to/session.jsonl"));
        assert_eq!(meta.project_path, "~/projects/api-server");
        assert_eq!(meta.updated_at, ts);
        assert_eq!(meta.message_count, 0);
        assert!(meta.first_prompt_preview.is_none());
    }

    #[test]
    fn test_session_meta_with_message_count() {
        let ts = test_timestamp();
        let meta = SessionMeta::new("session-123", PathBuf::from("/path"), "~/proj", ts)
            .with_message_count(42);

        assert_eq!(meta.message_count, 42);
    }

    #[test]
    fn test_session_meta_with_first_prompt() {
        let ts = test_timestamp();
        let meta = SessionMeta::new("session-123", PathBuf::from("/path"), "~/proj", ts)
            .with_first_prompt_preview("Help me refactor the auth module");

        assert_eq!(
            meta.first_prompt_preview,
            Some("Help me refactor the auth module".to_string())
        );
    }

    #[test]
    fn test_session_meta_chained_builders() {
        let ts = test_timestamp();
        let meta = SessionMeta::new("session-123", PathBuf::from("/path"), "~/proj", ts)
            .with_message_count(15)
            .with_first_prompt_preview("Write tests for...");

        assert_eq!(meta.message_count, 15);
        assert_eq!(
            meta.first_prompt_preview,
            Some("Write tests for...".to_string())
        );
    }

    #[test]
    fn test_scan_error_display() {
        let err = ScanError::directory_read(
            "/some/path",
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
        let msg = format!("{}", err);
        assert!(msg.contains("failed to read directory"));
        assert!(msg.contains("/some/path"));

        let err = ScanError::file_read(
            "/file/path",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        let msg = format!("{}", err);
        assert!(msg.contains("failed to read file"));
        assert!(msg.contains("/file/path"));

        let err = ScanError::metadata(
            "/meta/path",
            std::io::Error::new(std::io::ErrorKind::Other, "error"),
        );
        let msg = format!("{}", err);
        assert!(msg.contains("failed to get metadata"));
        assert!(msg.contains("/meta/path"));
    }
}
