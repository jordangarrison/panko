//! OpenAI Codex session scanner.
//!
//! Scans `~/.codex/` for session files. This is a stub implementation
//! for future support of Codex sessions.

use std::path::{Path, PathBuf};

use super::{AgentType, ScanError, SessionMeta, SessionScanner};

/// Scanner for OpenAI Codex sessions.
///
/// This is a stub implementation that provides the basic structure
/// for scanning Codex session files. The actual file format parsing
/// will be implemented when Codex session format is documented.
#[derive(Debug, Default)]
pub struct CodexScanner;

impl CodexScanner {
    /// Create a new CodexScanner.
    pub fn new() -> Self {
        Self
    }
}

impl SessionScanner for CodexScanner {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }

    fn scan_directory(&self, root: &Path) -> Result<Vec<SessionMeta>, ScanError> {
        // Check if root exists - if not, return empty (not an error)
        if !root.exists() {
            return Ok(Vec::new());
        }

        // Stub implementation: return empty for now
        // TODO: Implement actual Codex session parsing when format is known
        Ok(Vec::new())
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(home) = dirs::home_dir() {
            // Codex sessions are typically stored in ~/.codex/
            let codex_dir = home.join(".codex");
            roots.push(codex_dir);
        }

        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_scanner_name() {
        let scanner = CodexScanner::new();
        assert_eq!(scanner.name(), "codex");
    }

    #[test]
    fn test_codex_scanner_agent_type() {
        let scanner = CodexScanner::new();
        assert_eq!(scanner.agent_type(), AgentType::Codex);
    }

    #[test]
    fn test_codex_scanner_default_roots() {
        let scanner = CodexScanner::new();
        let roots = scanner.default_roots();

        // Should have at least one root (home/.codex)
        assert!(!roots.is_empty());

        // First root should end with .codex
        let first_root = &roots[0];
        assert!(first_root.ends_with(".codex"));
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let scanner = CodexScanner::new();

        // Scanning a non-existent directory should return empty, not error
        let sessions = scanner
            .scan_directory(Path::new("/nonexistent/codex/path"))
            .unwrap();
        assert!(sessions.is_empty());
    }
}
