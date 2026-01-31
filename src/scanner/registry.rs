//! Scanner registry for managing multiple session scanners.
//!
//! The registry holds multiple `SessionScanner` implementations and
//! provides methods to scan all registered sources at once.

use std::path::Path;

use super::{AgentType, ClaudeScanner, CodexScanner, ScanError, SessionMeta, SessionScanner};

/// A registry that holds multiple session scanners.
///
/// This allows scanning sessions from multiple AI coding agents
/// (Claude, Codex, etc.) through a single interface.
///
/// # Example
///
/// ```ignore
/// use panko::scanner::ScannerRegistry;
///
/// let registry = ScannerRegistry::default();
///
/// // Scan all default locations for all registered agents
/// let sessions = registry.scan_all_defaults();
///
/// for session in sessions {
///     println!("[{}] {}: {} messages",
///         session.agent_type.tag(),
///         session.id,
///         session.message_count
///     );
/// }
/// ```
pub struct ScannerRegistry {
    scanners: Vec<Box<dyn SessionScanner>>,
}

impl Default for ScannerRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ClaudeScanner::new()));
        registry.register(Box::new(CodexScanner::new()));
        registry
    }
}

impl ScannerRegistry {
    /// Create an empty scanner registry.
    pub fn new() -> Self {
        Self {
            scanners: Vec::new(),
        }
    }

    /// Register a new scanner with the registry.
    pub fn register(&mut self, scanner: Box<dyn SessionScanner>) {
        self.scanners.push(scanner);
    }

    /// Get the list of registered scanners.
    pub fn scanners(&self) -> &[Box<dyn SessionScanner>] {
        &self.scanners
    }

    /// Get a scanner by name.
    pub fn get_scanner(&self, name: &str) -> Option<&dyn SessionScanner> {
        self.scanners
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
    }

    /// Get a scanner by agent type.
    pub fn get_scanner_by_type(&self, agent_type: AgentType) -> Option<&dyn SessionScanner> {
        self.scanners
            .iter()
            .find(|s| s.agent_type() == agent_type)
            .map(|s| s.as_ref())
    }

    /// Scan a specific directory with all registered scanners.
    ///
    /// Each scanner will attempt to scan the directory for sessions
    /// it recognizes. Results are combined into a single list.
    pub fn scan_directory(&self, root: &Path) -> Result<Vec<SessionMeta>, ScanError> {
        let mut all_sessions = Vec::new();

        for scanner in &self.scanners {
            match scanner.scan_directory(root) {
                Ok(sessions) => all_sessions.extend(sessions),
                Err(_) => {
                    // Skip scanners that fail - directory might not contain
                    // sessions for this agent type
                    continue;
                }
            }
        }

        // Sort by updated_at descending (newest first)
        all_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(all_sessions)
    }

    /// Scan all default directories for all registered scanners.
    ///
    /// Each scanner provides its default directories (e.g., ~/.claude/projects
    /// for Claude, ~/.codex for Codex). This method scans all of them and
    /// combines the results.
    ///
    /// Missing directories are silently ignored - this is expected when
    /// an agent isn't installed.
    pub fn scan_all_defaults(&self) -> Vec<SessionMeta> {
        let mut all_sessions = Vec::new();

        for scanner in &self.scanners {
            for root in scanner.default_roots() {
                // Skip if the directory doesn't exist
                if !root.exists() {
                    continue;
                }

                match scanner.scan_directory(&root) {
                    Ok(sessions) => all_sessions.extend(sessions),
                    Err(_) => {
                        // Log warning but continue
                        // In production, we'd use tracing::warn! here
                        continue;
                    }
                }
            }
        }

        // Sort by updated_at descending (newest first)
        all_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        all_sessions
    }

    /// Get a list of all agent types registered in the scanner.
    pub fn registered_agent_types(&self) -> Vec<AgentType> {
        self.scanners.iter().map(|s| s.agent_type()).collect()
    }

    /// Filter sessions by agent type.
    pub fn filter_by_agent_type(
        sessions: &[SessionMeta],
        agent_type: AgentType,
    ) -> Vec<&SessionMeta> {
        sessions
            .iter()
            .filter(|s| s.agent_type == agent_type)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Create a test directory with Claude-style session files.
    fn create_claude_test_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        let project_dir = temp_dir.path().join("-home-user-myproject");
        fs::create_dir(&project_dir).unwrap();

        let session_path = project_dir.join("session-abc123.jsonl");
        let mut file = File::create(&session_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"Test prompt"}}}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"Response"}}]}}}}"#).unwrap();

        temp_dir
    }

    #[test]
    fn test_registry_default() {
        let registry = ScannerRegistry::default();

        // Should have Claude and Codex scanners by default
        assert_eq!(registry.scanners().len(), 2);

        // Check scanners are registered
        assert!(registry.get_scanner("claude").is_some());
        assert!(registry.get_scanner("codex").is_some());
    }

    #[test]
    fn test_registry_new_empty() {
        let registry = ScannerRegistry::new();
        assert!(registry.scanners().is_empty());
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ScannerRegistry::new();
        registry.register(Box::new(ClaudeScanner::new()));

        assert_eq!(registry.scanners().len(), 1);
        assert!(registry.get_scanner("claude").is_some());
    }

    #[test]
    fn test_get_scanner_by_type() {
        let registry = ScannerRegistry::default();

        let claude = registry.get_scanner_by_type(AgentType::Claude);
        assert!(claude.is_some());
        assert_eq!(claude.unwrap().name(), "claude");

        let codex = registry.get_scanner_by_type(AgentType::Codex);
        assert!(codex.is_some());
        assert_eq!(codex.unwrap().name(), "codex");
    }

    #[test]
    fn test_registered_agent_types() {
        let registry = ScannerRegistry::default();
        let types = registry.registered_agent_types();

        assert!(types.contains(&AgentType::Claude));
        assert!(types.contains(&AgentType::Codex));
    }

    #[test]
    fn test_scan_directory() {
        let temp_dir = create_claude_test_dir();
        let registry = ScannerRegistry::default();

        let sessions = registry.scan_directory(temp_dir.path()).unwrap();

        // Should find the Claude session
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent_type, AgentType::Claude);
        assert!(sessions[0].id.contains("abc123"));
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let registry = ScannerRegistry::default();

        let sessions = registry
            .scan_directory(Path::new("/nonexistent/path"))
            .unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_filter_by_agent_type() {
        let sessions = vec![
            SessionMeta::new("s1", PathBuf::from("/p1"), "proj1", test_timestamp())
                .with_agent_type(AgentType::Claude),
            SessionMeta::new("s2", PathBuf::from("/p2"), "proj2", test_timestamp())
                .with_agent_type(AgentType::Codex),
            SessionMeta::new("s3", PathBuf::from("/p3"), "proj3", test_timestamp())
                .with_agent_type(AgentType::Claude),
        ];

        let claude_sessions = ScannerRegistry::filter_by_agent_type(&sessions, AgentType::Claude);
        assert_eq!(claude_sessions.len(), 2);

        let codex_sessions = ScannerRegistry::filter_by_agent_type(&sessions, AgentType::Codex);
        assert_eq!(codex_sessions.len(), 1);
    }

    #[test]
    fn test_agent_type_display() {
        assert_eq!(AgentType::Claude.display_name(), "Claude");
        assert_eq!(AgentType::Codex.display_name(), "Codex");

        assert_eq!(AgentType::Claude.tag(), "CC");
        assert_eq!(AgentType::Codex.tag(), "CX");

        assert_eq!(format!("{}", AgentType::Claude), "Claude");
        assert_eq!(format!("{}", AgentType::Codex), "Codex");
    }

    #[test]
    fn test_agent_type_default() {
        let default = AgentType::default();
        assert_eq!(default, AgentType::Claude);
    }
}
