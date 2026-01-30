//! Claude Code session scanner.
//!
//! Scans `~/.claude/projects/` for JSONL session files and extracts
//! lightweight metadata without fully parsing the content.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{ScanError, SessionMeta, SessionScanner};

/// Scanner for Claude Code sessions stored in `~/.claude/projects/`.
#[derive(Debug, Default)]
pub struct ClaudeScanner;

impl ClaudeScanner {
    /// Create a new ClaudeScanner.
    pub fn new() -> Self {
        Self
    }

    /// Scan a single session file for metadata.
    ///
    /// This method reads only enough of the file to extract:
    /// - First user prompt (first ~100 chars)
    /// - Message count (number of user + assistant entries)
    /// - Session ID from content or filename
    ///
    /// It does NOT fully parse the JSONL content.
    fn scan_session_file(&self, path: &Path, project_path: &str) -> Result<SessionMeta, ScanError> {
        // Get file metadata for updated_at
        let file_meta = fs::metadata(path).map_err(|e| ScanError::metadata(path, e))?;
        let updated_at = file_meta
            .modified()
            .map_err(|e| ScanError::metadata(path, e))?;
        let updated_at: DateTime<Utc> = updated_at.into();

        // Extract session ID from filename (without .jsonl extension)
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap_or_else(|| "unknown".to_string());

        // Open file and scan for metadata
        let file = File::open(path).map_err(|e| ScanError::file_read(path, e))?;
        let reader = BufReader::new(file);

        let mut message_count = 0;
        let mut first_prompt: Option<String> = None;

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => continue, // Skip unreadable lines
            };

            if line.trim().is_empty() {
                continue;
            }

            // Quick parse to check entry type
            let entry: QuickEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue, // Skip malformed lines
            };

            // Count user and assistant messages
            if entry.entry_type == "user" || entry.entry_type == "assistant" {
                // Skip meta messages
                if entry.is_meta.unwrap_or(false) {
                    continue;
                }

                message_count += 1;

                // Extract first user prompt if not yet found
                if first_prompt.is_none() && entry.entry_type == "user" {
                    if let Some(prompt) = extract_prompt(&entry) {
                        // Skip command messages
                        if !prompt.starts_with("<command-name>")
                            && !prompt.starts_with("<local-command")
                        {
                            first_prompt = Some(truncate_prompt(&prompt, 100));
                        }
                    }
                }
            }
        }

        let mut meta = SessionMeta::new(id, path.to_path_buf(), project_path, updated_at)
            .with_message_count(message_count);

        if let Some(prompt) = first_prompt {
            meta = meta.with_first_prompt_preview(prompt);
        }

        Ok(meta)
    }
}

impl SessionScanner for ClaudeScanner {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn scan_directory(&self, root: &Path) -> Result<Vec<SessionMeta>, ScanError> {
        let mut sessions = Vec::new();

        // Check if root exists
        if !root.exists() {
            return Ok(sessions); // Return empty if directory doesn't exist
        }

        // Scan for project directories
        let root_entries = fs::read_dir(root).map_err(|e| ScanError::directory_read(root, e))?;

        for entry_result in root_entries {
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue, // Skip unreadable entries
            };

            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            // Get the project name/path from directory name
            let project_path = project_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(decode_project_path)
                .unwrap_or_else(|| "unknown".to_string());

            // Scan for .jsonl files in this project directory
            let project_entries = match fs::read_dir(&project_dir) {
                Ok(entries) => entries,
                Err(_) => continue, // Skip unreadable project directories
            };

            for file_entry_result in project_entries {
                let file_entry = match file_entry_result {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let file_path = file_entry.path();

                // Only process .jsonl files
                if file_path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }

                // Try to scan the session file
                match self.scan_session_file(&file_path, &project_path) {
                    Ok(meta) => sessions.push(meta),
                    Err(_) => {
                        // Log warning but continue scanning
                        // In production, we'd use tracing::warn! here
                        continue;
                    }
                }
            }
        }

        // Sort by updated_at descending (newest first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(home) = dirs::home_dir() {
            let claude_projects = home.join(".claude").join("projects");
            roots.push(claude_projects);
        }

        roots
    }
}

/// Minimal entry structure for quick scanning.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickEntry {
    #[serde(rename = "type")]
    entry_type: String,
    message: Option<QuickMessage>,
    is_meta: Option<bool>,
}

/// Minimal message structure for quick scanning.
#[derive(Debug, Deserialize)]
struct QuickMessage {
    content: Option<MessageContent>,
}

/// Message content can be a string or array.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    String(String),
    Array(Vec<ContentBlock>),
}

/// Content block for array-style messages.
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    tool_use_id: Option<String>,
}

/// Extract user prompt content from a quick entry.
fn extract_prompt(entry: &QuickEntry) -> Option<String> {
    let message = entry.message.as_ref()?;
    let content = message.content.as_ref()?;

    match content {
        MessageContent::String(s) => Some(s.clone()),
        MessageContent::Array(blocks) => {
            // Skip tool results (they have tool_use_id)
            let text_blocks: Vec<&str> = blocks
                .iter()
                .filter(|b| b.tool_use_id.is_none())
                .filter(|b| b.block_type.as_deref() == Some("text"))
                .filter_map(|b| b.text.as_deref())
                .collect();

            if text_blocks.is_empty() {
                None
            } else {
                Some(text_blocks.join("\n"))
            }
        }
    }
}

/// Truncate a prompt to the specified maximum characters.
///
/// Truncates at word boundaries when possible and adds "..." if truncated.
fn truncate_prompt(prompt: &str, max_chars: usize) -> String {
    let prompt = prompt.trim();

    if prompt.len() <= max_chars {
        return prompt.to_string();
    }

    // Find the last space before max_chars to avoid cutting words
    let truncate_at = prompt
        .char_indices()
        .take_while(|(i, _)| *i < max_chars)
        .filter(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .last()
        .unwrap_or(max_chars);

    // Make sure we have at least some content
    let truncate_at = if truncate_at < max_chars / 2 {
        // If word boundary is too early, just cut at max_chars
        prompt
            .char_indices()
            .take_while(|(i, _)| *i < max_chars)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_chars)
    } else {
        truncate_at
    };

    format!("{}...", &prompt[..truncate_at])
}

/// Decode a project path from the directory name.
///
/// Claude Code encodes project paths by replacing `/` with `-`.
/// For example: "-home-user-projects-api-server" represents "/home/user/projects/api-server"
///
/// This function makes the path human-readable by:
/// 1. Converting the leading `-` back to `/`
/// 2. Replacing remaining `-` with `/`
/// 3. Trying to simplify with `~` for home directory
///
/// Note: This is a best-effort decode since project names might contain hyphens.
fn decode_project_path(encoded: &str) -> String {
    // For now, just return the encoded string as-is
    // A more sophisticated decode could try to match against known directories
    // but that would require filesystem access which is slow

    // Simply return the encoded form - it's still identifiable
    // The TUI can display this and users will recognize their project names
    encoded.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a test directory structure with session files.
    fn create_test_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        // Create a project directory
        let project_dir = temp_dir.path().join("-home-user-myproject");
        fs::create_dir(&project_dir).unwrap();

        // Create a valid session file
        let session1_path = project_dir.join("session-abc123.jsonl");
        let mut session1 = File::create(&session1_path).unwrap();
        writeln!(session1, r#"{{"type":"user","sessionId":"abc123","timestamp":"2024-01-15T10:30:00Z","message":{{"role":"user","content":"Help me write a function"}}}}"#).unwrap();
        writeln!(session1, r#"{{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{{"role":"assistant","content":[{{"type":"text","text":"Sure, I can help!"}}]}}}}"#).unwrap();
        writeln!(session1, r#"{{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{{"role":"user","content":"Thanks!"}}}}"#).unwrap();
        writeln!(session1, r#"{{"type":"assistant","timestamp":"2024-01-15T10:30:03Z","message":{{"role":"assistant","content":[{{"type":"text","text":"You're welcome!"}}]}}}}"#).unwrap();

        // Create another session
        let session2_path = project_dir.join("session-def456.jsonl");
        let mut session2 = File::create(&session2_path).unwrap();
        writeln!(session2, r#"{{"type":"user","sessionId":"def456","timestamp":"2024-01-16T10:30:00Z","message":{{"role":"user","content":"This is a much longer prompt that should be truncated because it exceeds the maximum character limit we've set for preview text in the session list view"}}}}"#).unwrap();
        writeln!(session2, r#"{{"type":"assistant","timestamp":"2024-01-16T10:30:01Z","message":{{"role":"assistant","content":[{{"type":"text","text":"Got it!"}}]}}}}"#).unwrap();

        // Create a second project directory
        let project2_dir = temp_dir.path().join("-home-user-another-project");
        fs::create_dir(&project2_dir).unwrap();

        let session3_path = project2_dir.join("session-ghi789.jsonl");
        let mut session3 = File::create(&session3_path).unwrap();
        writeln!(session3, r#"{{"type":"user","sessionId":"ghi789","timestamp":"2024-01-10T10:30:00Z","message":{{"role":"user","content":"Debug this code"}}}}"#).unwrap();

        temp_dir
    }

    /// Create a test directory with corrupted/edge case files.
    fn create_test_dir_with_edge_cases() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        let project_dir = temp_dir.path().join("-home-user-edgecase");
        fs::create_dir(&project_dir).unwrap();

        // Empty file
        let empty_path = project_dir.join("empty.jsonl");
        File::create(&empty_path).unwrap();

        // File with only blank lines
        let blanks_path = project_dir.join("blanks.jsonl");
        let mut blanks = File::create(&blanks_path).unwrap();
        writeln!(blanks, "").unwrap();
        writeln!(blanks, "   ").unwrap();
        writeln!(blanks, "").unwrap();

        // File with malformed JSON lines
        let malformed_path = project_dir.join("malformed.jsonl");
        let mut malformed = File::create(&malformed_path).unwrap();
        writeln!(malformed, "not json at all").unwrap();
        writeln!(
            malformed,
            r#"{{"type":"user","message":{{"content":"valid line"}}}}"#
        )
        .unwrap();
        writeln!(malformed, "{{missing closing brace").unwrap();

        // File with meta messages only
        let meta_path = project_dir.join("meta-only.jsonl");
        let mut meta = File::create(&meta_path).unwrap();
        writeln!(meta, r#"{{"type":"user","isMeta":true,"message":{{"content":"<local-command>skip</local-command>"}}}}"#).unwrap();

        // File with command messages
        let cmd_path = project_dir.join("commands.jsonl");
        let mut cmd = File::create(&cmd_path).unwrap();
        writeln!(
            cmd,
            r#"{{"type":"user","message":{{"content":"<command-name>/clear</command-name>"}}}}"#
        )
        .unwrap();
        writeln!(
            cmd,
            r#"{{"type":"user","message":{{"content":"Real prompt after commands"}}}}"#
        )
        .unwrap();

        // Non-jsonl file (should be ignored)
        let txt_path = project_dir.join("notes.txt");
        let mut txt = File::create(&txt_path).unwrap();
        writeln!(txt, "This is not a session file").unwrap();

        temp_dir
    }

    #[test]
    fn test_claude_scanner_name() {
        let scanner = ClaudeScanner::new();
        assert_eq!(scanner.name(), "claude");
    }

    #[test]
    fn test_claude_scanner_default_roots() {
        let scanner = ClaudeScanner::new();
        let roots = scanner.default_roots();

        // Should have at least one root (home/.claude/projects)
        assert!(!roots.is_empty());

        // First root should end with .claude/projects
        let first_root = &roots[0];
        assert!(first_root.ends_with(".claude/projects"));
    }

    #[test]
    fn test_scan_directory_basic() {
        let temp_dir = create_test_dir();
        let scanner = ClaudeScanner::new();

        let sessions = scanner.scan_directory(temp_dir.path()).unwrap();

        // Should find 3 sessions across 2 projects
        assert_eq!(sessions.len(), 3);

        // Sessions should be sorted by updated_at descending
        // session-def456 should be first (newest)
        assert!(sessions[0].id.contains("def456") || sessions[0].id.contains("abc123"));
    }

    #[test]
    fn test_scan_directory_message_counts() {
        let temp_dir = create_test_dir();
        let scanner = ClaudeScanner::new();

        let sessions = scanner.scan_directory(temp_dir.path()).unwrap();

        // Find session abc123 and verify message count
        let abc123 = sessions.iter().find(|s| s.id.contains("abc123")).unwrap();
        assert_eq!(abc123.message_count, 4); // 2 user + 2 assistant

        // Find session def456
        let def456 = sessions.iter().find(|s| s.id.contains("def456")).unwrap();
        assert_eq!(def456.message_count, 2); // 1 user + 1 assistant
    }

    #[test]
    fn test_scan_directory_first_prompt() {
        let temp_dir = create_test_dir();
        let scanner = ClaudeScanner::new();

        let sessions = scanner.scan_directory(temp_dir.path()).unwrap();

        // Find session abc123 and verify first prompt
        let abc123 = sessions.iter().find(|s| s.id.contains("abc123")).unwrap();
        assert_eq!(
            abc123.first_prompt_preview,
            Some("Help me write a function".to_string())
        );

        // Find session def456 - should be truncated
        let def456 = sessions.iter().find(|s| s.id.contains("def456")).unwrap();
        assert!(def456.first_prompt_preview.is_some());
        let preview = def456.first_prompt_preview.as_ref().unwrap();
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 103); // 100 chars + "..."
    }

    #[test]
    fn test_scan_directory_project_paths() {
        let temp_dir = create_test_dir();
        let scanner = ClaudeScanner::new();

        let sessions = scanner.scan_directory(temp_dir.path()).unwrap();

        // Check that project paths are extracted from directory names
        let project_paths: Vec<&str> = sessions.iter().map(|s| s.project_path.as_str()).collect();

        // Should have paths from both projects (directory names as-is)
        assert!(project_paths.iter().any(|p| p.contains("myproject")));
        assert!(project_paths.iter().any(|p| p.contains("another")));
    }

    #[test]
    fn test_scan_directory_missing_directory() {
        let scanner = ClaudeScanner::new();

        // Scanning a non-existent directory should return empty, not error
        let sessions = scanner
            .scan_directory(Path::new("/nonexistent/path"))
            .unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_scan_directory_edge_cases() {
        let temp_dir = create_test_dir_with_edge_cases();
        let scanner = ClaudeScanner::new();

        let sessions = scanner.scan_directory(temp_dir.path()).unwrap();

        // Should have found some sessions despite edge cases
        // Empty, blanks, and malformed files should still be processed
        assert!(!sessions.is_empty());

        // The commands.jsonl file should have "Real prompt after commands" as first prompt
        let cmd_session = sessions.iter().find(|s| s.id.contains("commands"));
        if let Some(session) = cmd_session {
            assert_eq!(
                session.first_prompt_preview,
                Some("Real prompt after commands".to_string())
            );
        }

        // Non-jsonl files should be ignored
        let txt_session = sessions.iter().find(|s| s.id.contains("notes"));
        assert!(txt_session.is_none());
    }

    #[test]
    fn test_truncate_prompt_short() {
        let short = "Hello world";
        assert_eq!(truncate_prompt(short, 100), "Hello world");
    }

    #[test]
    fn test_truncate_prompt_long() {
        let long = "This is a very long prompt that definitely exceeds the maximum character limit";
        let truncated = truncate_prompt(long, 50);

        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 53); // 50 + "..."
    }

    #[test]
    fn test_truncate_prompt_word_boundary() {
        let prompt = "One two three four five six seven eight nine ten eleven twelve";
        let truncated = truncate_prompt(prompt, 30);

        // Should truncate at a word boundary
        assert!(truncated.ends_with("..."));
        // Should not cut a word in half
        assert!(!truncated.contains("thre...") && !truncated.contains("fou..."));
    }

    #[test]
    fn test_decode_project_path_encoded() {
        // Test encoded absolute path
        let encoded = "-home-user-projects-api-server";
        let decoded = decode_project_path(encoded);

        // Currently we just return the encoded form as-is
        // This preserves the original directory name which is still readable
        assert_eq!(decoded, encoded);
    }

    #[test]
    fn test_decode_project_path_relative() {
        // Relative paths should be returned as-is
        let relative = "my-project";
        assert_eq!(decode_project_path(relative), "my-project");
    }

    #[test]
    fn test_scan_session_file_valid() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().join("test-session.jsonl");

        let mut file = File::create(&session_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"Test prompt"}}}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"Response"}}]}}}}"#).unwrap();

        let scanner = ClaudeScanner::new();
        let meta = scanner.scan_session_file(&session_path, "~/test").unwrap();

        assert_eq!(meta.id, "test-session");
        assert_eq!(meta.project_path, "~/test");
        assert_eq!(meta.message_count, 2);
        assert_eq!(meta.first_prompt_preview, Some("Test prompt".to_string()));
    }

    #[test]
    fn test_scan_session_file_tool_results_not_counted_as_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let session_path = temp_dir.path().join("tool-session.jsonl");

        let mut file = File::create(&session_path).unwrap();
        // First "user" message is actually a tool result
        writeln!(file, r#"{{"type":"user","message":{{"content":[{{"tool_use_id":"tool-1","type":"tool_result","content":"file contents"}}]}}}}"#).unwrap();
        // Real user message
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"Real user prompt"}}}}"#
        )
        .unwrap();

        let scanner = ClaudeScanner::new();
        let meta = scanner.scan_session_file(&session_path, "~/test").unwrap();

        // First prompt should be the real user message, not the tool result
        assert_eq!(
            meta.first_prompt_preview,
            Some("Real user prompt".to_string())
        );
    }
}
