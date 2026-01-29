//! Session parser trait and implementations.
//!
//! This module provides a unified interface for parsing AI coding agent session files.
//! Different agent types (Claude Code, Codex, etc.) have their own parser implementations
//! that convert their native formats into the common [`Session`] type.

mod claude;
mod error;
mod types;

pub use claude::ClaudeParser;
pub use error::ParseError;
pub use types::{Block, Session};

use std::path::Path;

/// A parser for AI coding agent session files.
///
/// Implementations of this trait handle specific session file formats.
/// Each parser should be able to detect whether it can handle a given file
/// and convert it into the unified [`Session`] format.
///
/// # Example
///
/// ```ignore
/// use agent_replay::parser::{SessionParser, Session};
///
/// fn parse_file(parser: &dyn SessionParser, path: &Path) -> Result<Session, ParseError> {
///     if parser.can_parse(path) {
///         parser.parse(path)
///     } else {
///         Err(ParseError::unsupported_format(path))
///     }
/// }
/// ```
pub trait SessionParser: Send + Sync {
    /// Returns the name of this parser (e.g., "claude", "codex").
    fn name(&self) -> &'static str;

    /// Check if this parser can handle the given file.
    ///
    /// This may examine the file extension, peek at the file contents,
    /// or use other heuristics to determine compatibility.
    fn can_parse(&self, path: &Path) -> bool;

    /// Parse the session file into a unified Session.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the file cannot be read or parsed.
    fn parse(&self, path: &Path) -> Result<Session, ParseError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    /// A mock parser for testing the trait.
    struct MockParser {
        supported_extension: &'static str,
    }

    impl SessionParser for MockParser {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn can_parse(&self, path: &Path) -> bool {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == self.supported_extension)
                .unwrap_or(false)
        }

        fn parse(&self, path: &Path) -> Result<Session, ParseError> {
            if !self.can_parse(path) {
                return Err(ParseError::unsupported_format(path));
            }
            Ok(Session::new("mock-session", Utc::now()))
        }
    }

    #[test]
    fn test_parser_trait_name() {
        let parser = MockParser {
            supported_extension: "jsonl",
        };
        assert_eq!(parser.name(), "mock");
    }

    #[test]
    fn test_parser_trait_can_parse() {
        let parser = MockParser {
            supported_extension: "jsonl",
        };

        assert!(parser.can_parse(Path::new("session.jsonl")));
        assert!(parser.can_parse(Path::new("/path/to/file.jsonl")));
        assert!(!parser.can_parse(Path::new("session.txt")));
        assert!(!parser.can_parse(Path::new("session")));
    }

    #[test]
    fn test_parser_trait_parse_supported() {
        let parser = MockParser {
            supported_extension: "jsonl",
        };

        let result = parser.parse(Path::new("test.jsonl"));
        assert!(result.is_ok());
        let session = result.unwrap();
        assert_eq!(session.id, "mock-session");
    }

    #[test]
    fn test_parser_trait_parse_unsupported() {
        let parser = MockParser {
            supported_extension: "jsonl",
        };

        let result = parser.parse(Path::new("test.txt"));
        assert!(result.is_err());
        match result {
            Err(ParseError::UnsupportedFormat { path }) => {
                assert_eq!(path, Path::new("test.txt"));
            }
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_parser_trait_object_safety() {
        // Verify the trait is object-safe by creating a trait object
        let parser: Box<dyn SessionParser> = Box::new(MockParser {
            supported_extension: "jsonl",
        });

        assert_eq!(parser.name(), "mock");
        assert!(parser.can_parse(Path::new("test.jsonl")));
    }
}
