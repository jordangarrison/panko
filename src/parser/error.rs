//! Error types for session parsing.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when parsing a session file.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The file could not be read.
    #[error("failed to read file {path}: {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The file format is not supported or could not be detected.
    #[error("unsupported file format: {path}")]
    UnsupportedFormat { path: PathBuf },

    /// The file content could not be parsed as JSON.
    #[error("failed to parse JSON at line {line}: {message}")]
    JsonError { line: usize, message: String },

    /// The file content is missing required fields.
    #[error("missing required field: {field}")]
    MissingField { field: String },

    /// The file content has an invalid value.
    #[error("invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    /// A timestamp could not be parsed.
    #[error("invalid timestamp: {value}")]
    InvalidTimestamp { value: String },

    /// The file is empty or contains no valid entries.
    #[error("empty session: no valid entries found in {path}")]
    EmptySession { path: PathBuf },
}

impl ParseError {
    /// Create an IO error.
    pub fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::IoError {
            path: path.into(),
            source,
        }
    }

    /// Create an unsupported format error.
    pub fn unsupported_format(path: impl Into<PathBuf>) -> Self {
        Self::UnsupportedFormat { path: path.into() }
    }

    /// Create a JSON parsing error.
    pub fn json_error(line: usize, message: impl Into<String>) -> Self {
        Self::JsonError {
            line,
            message: message.into(),
        }
    }

    /// Create a missing field error.
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Create an invalid value error.
    pub fn invalid_value(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidValue {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create an invalid timestamp error.
    pub fn invalid_timestamp(value: impl Into<String>) -> Self {
        Self::InvalidTimestamp {
            value: value.into(),
        }
    }

    /// Create an empty session error.
    pub fn empty_session(path: impl Into<PathBuf>) -> Self {
        Self::EmptySession { path: path.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let err = ParseError::io_error(
            "/path/to/file.jsonl",
            std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        );
        assert!(err.to_string().contains("/path/to/file.jsonl"));
        assert!(err.to_string().contains("failed to read file"));
    }

    #[test]
    fn test_unsupported_format_display() {
        let err = ParseError::unsupported_format("/path/to/file.txt");
        assert_eq!(
            err.to_string(),
            "unsupported file format: /path/to/file.txt"
        );
    }

    #[test]
    fn test_json_error_display() {
        let err = ParseError::json_error(42, "unexpected token");
        assert_eq!(
            err.to_string(),
            "failed to parse JSON at line 42: unexpected token"
        );
    }

    #[test]
    fn test_missing_field_display() {
        let err = ParseError::missing_field("timestamp");
        assert_eq!(err.to_string(), "missing required field: timestamp");
    }

    #[test]
    fn test_invalid_value_display() {
        let err = ParseError::invalid_value("role", "expected 'user' or 'assistant'");
        assert_eq!(
            err.to_string(),
            "invalid value for role: expected 'user' or 'assistant'"
        );
    }

    #[test]
    fn test_invalid_timestamp_display() {
        let err = ParseError::invalid_timestamp("not-a-date");
        assert_eq!(err.to_string(), "invalid timestamp: not-a-date");
    }

    #[test]
    fn test_empty_session_display() {
        let err = ParseError::empty_session("/path/to/empty.jsonl");
        assert_eq!(
            err.to_string(),
            "empty session: no valid entries found in /path/to/empty.jsonl"
        );
    }
}
