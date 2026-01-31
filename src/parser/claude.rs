//! Claude Code JSONL session parser.
//!
//! Parses session files from Claude Code stored in `~/.claude/projects/`.
//! Each line is a JSON object representing an event in the session.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{Block, ParseError, Session, SessionParser, SubAgentMeta, SubAgentStatus};

/// Parser for Claude Code JSONL session files.
#[derive(Debug, Default)]
pub struct ClaudeParser;

impl ClaudeParser {
    /// Create a new ClaudeParser.
    pub fn new() -> Self {
        Self
    }
}

impl SessionParser for ClaudeParser {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn can_parse(&self, path: &Path) -> bool {
        // Check for .jsonl extension
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "jsonl")
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<Session, ParseError> {
        if !self.can_parse(path) {
            return Err(ParseError::unsupported_format(path));
        }

        let file = File::open(path).map_err(|e| ParseError::io_error(path, e))?;
        let reader = BufReader::new(file);

        let mut session_id: Option<String> = None;
        let mut project: Option<String> = None;
        let mut started_at: Option<DateTime<Utc>> = None;
        let mut blocks: Vec<Block> = Vec::new();

        // Track pending tool calls that haven't received results yet
        let mut pending_tool_calls: HashMap<String, PendingToolCall> = HashMap::new();

        // Track sub-agents spawned via the Task tool
        let mut sub_agents: Vec<SubAgentMeta> = Vec::new();
        let mut pending_sub_agents: HashMap<String, usize> = HashMap::new(); // tool_id -> index in sub_agents

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result.map_err(|e| ParseError::io_error(path, e))?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            let entry: ClaudeEntry = serde_json::from_str(&line)
                .map_err(|e| ParseError::json_error(line_num + 1, e.to_string()))?;

            // Extract session metadata from first entry we see
            if session_id.is_none() {
                if let Some(ref sid) = entry.session_id {
                    session_id = Some(sid.clone());
                }
            }

            // Extract project path from cwd
            if project.is_none() {
                if let Some(ref cwd) = entry.cwd {
                    project = Some(cwd.clone());
                }
            }

            // Track earliest timestamp as session start
            if let Some(ts) = entry.timestamp {
                if started_at.is_none() || ts < started_at.unwrap() {
                    started_at = Some(ts);
                }
            }

            // Process based on entry type
            match entry.entry_type.as_str() {
                "user" => {
                    if let Some(message) = entry.message {
                        // Skip meta messages (isMeta = true) and tool results
                        if entry.is_meta.unwrap_or(false) {
                            continue;
                        }

                        // Check if this is a tool result
                        if let Some(MessageContent::Array(blocks_arr)) = &message.content {
                            // Tool results come as array of content blocks
                            for block in blocks_arr {
                                if let Some(tool_use_id) = &block.tool_use_id {
                                    // Check if this is a sub-agent completion
                                    if let Some(&agent_idx) = pending_sub_agents.get(tool_use_id) {
                                        // Complete the sub-agent
                                        if let Some(agent) = sub_agents.get_mut(agent_idx) {
                                            let result = block
                                                .content
                                                .as_ref()
                                                .map(|c| c.to_string())
                                                .unwrap_or_default();
                                            let completed_at =
                                                entry.timestamp.unwrap_or_else(Utc::now);

                                            // Check if result indicates an error
                                            if block.is_error.unwrap_or(false) {
                                                agent.fail(result, completed_at);
                                            } else {
                                                agent.complete(result, completed_at);
                                            }

                                            // Update the corresponding SubAgentSpawn block status
                                            for blk in blocks.iter_mut() {
                                                if let Block::SubAgentSpawn {
                                                    agent_id,
                                                    status,
                                                    ..
                                                } = blk
                                                {
                                                    if agent_id == tool_use_id {
                                                        *status = agent.status;
                                                    }
                                                }
                                            }
                                        }
                                        pending_sub_agents.remove(tool_use_id);
                                    }

                                    // This is a tool result
                                    if let Some(pending) = pending_tool_calls.remove(tool_use_id) {
                                        let output = block
                                            .content
                                            .as_ref()
                                            .map(|c| Value::String(c.to_string()));
                                        blocks.push(Block::tool_call(
                                            pending.name,
                                            pending.input,
                                            output,
                                            pending.timestamp,
                                        ));
                                    }
                                }
                            }
                            continue;
                        }

                        // Regular user message
                        if let Some(ts) = entry.timestamp {
                            if let Some(content) = extract_user_content(&message) {
                                // Skip command messages and system output
                                if !content.starts_with("<command-name>")
                                    && !content.starts_with("<local-command")
                                {
                                    blocks.push(Block::user_prompt(content, ts));
                                }
                            }
                        }
                    }
                }
                "assistant" => {
                    if let Some(message) = entry.message {
                        if let Some(ts) = entry.timestamp {
                            process_assistant_message(
                                &message,
                                ts,
                                &mut blocks,
                                &mut pending_tool_calls,
                                &mut sub_agents,
                                &mut pending_sub_agents,
                            );
                        }
                    }
                }
                // Skip progress, summary, system, file-history-snapshot entries
                _ => {}
            }
        }

        // Handle any remaining pending tool calls (no result received)
        for (_id, pending) in pending_tool_calls {
            blocks.push(Block::tool_call(
                pending.name,
                pending.input,
                None,
                pending.timestamp,
            ));
        }

        // Create session from extracted data
        let session_id = session_id.unwrap_or_else(|| {
            // Fall back to filename without extension
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
                .unwrap_or_else(|| "unknown".to_string())
        });

        // Use current time as fallback for sessions with no timestamp data
        let started_at = started_at.unwrap_or_else(Utc::now);

        // Allow empty sessions (zero blocks) - they're valid JSONL files
        let mut session = Session::new(session_id, started_at);
        if let Some(proj) = project {
            session = session.with_project(proj);
        }
        session.blocks = blocks;
        session.sub_agents = sub_agents;

        Ok(session)
    }
}

/// A pending tool call waiting for its result.
#[derive(Debug)]
struct PendingToolCall {
    name: String,
    input: Value,
    timestamp: DateTime<Utc>,
}

/// Represents a single entry/line in a Claude JSONL file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    session_id: Option<String>,
    cwd: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    message: Option<ClaudeMessage>,
    is_meta: Option<bool>,
}

/// A message within a Claude entry.
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<MessageContent>,
}

/// Message content can be a string or an array of content blocks.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    String(String),
    Array(Vec<ContentBlock>),
}

/// A content block within a message.
#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    name: Option<String>,
    input: Option<Value>,
    id: Option<String>,
    tool_use_id: Option<String>,
    content: Option<ToolResultContent>,
    /// Whether this tool result indicates an error.
    is_error: Option<bool>,
}

/// Tool result content can be a string or an array of content blocks.
/// This handles the polymorphic nature of tool_result content in Claude sessions.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ToolResultContent {
    String(String),
    Array(Vec<ToolResultContentBlock>),
}

/// A content block within a tool result array.
#[derive(Debug, Deserialize)]
struct ToolResultContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    block_type: Option<String>,
    text: Option<String>,
}

impl std::fmt::Display for ToolResultContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolResultContent::String(s) => write!(f, "{}", s),
            ToolResultContent::Array(blocks) => {
                let text = blocks
                    .iter()
                    .filter_map(|b| b.text.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                write!(f, "{}", text)
            }
        }
    }
}

/// Extract user content from a message.
fn extract_user_content(message: &ClaudeMessage) -> Option<String> {
    match &message.content {
        Some(MessageContent::String(s)) => Some(s.clone()),
        Some(MessageContent::Array(blocks)) => {
            // Concatenate text blocks
            let text: Vec<&str> = blocks
                .iter()
                .filter_map(|b| {
                    if b.block_type.as_deref() == Some("text") {
                        b.text.as_deref()
                    } else {
                        None
                    }
                })
                .collect();
            if text.is_empty() {
                None
            } else {
                Some(text.join("\n"))
            }
        }
        None => None,
    }
}

/// Process an assistant message and add blocks.
fn process_assistant_message(
    message: &ClaudeMessage,
    timestamp: DateTime<Utc>,
    blocks: &mut Vec<Block>,
    pending_tool_calls: &mut HashMap<String, PendingToolCall>,
    sub_agents: &mut Vec<SubAgentMeta>,
    pending_sub_agents: &mut HashMap<String, usize>,
) {
    if let Some(MessageContent::Array(content_blocks)) = &message.content {
        for block in content_blocks {
            match block.block_type.as_deref() {
                Some("text") => {
                    if let Some(text) = &block.text {
                        if !text.trim().is_empty() {
                            blocks.push(Block::assistant_response(text.clone(), timestamp));
                        }
                    }
                }
                Some("thinking") => {
                    if let Some(text) = &block.text {
                        if !text.trim().is_empty() {
                            blocks.push(Block::thinking(text.clone(), timestamp));
                        }
                    }
                }
                Some("tool_use") => {
                    if let (Some(name), Some(id)) = (&block.name, &block.id) {
                        let input = block.input.clone().unwrap_or(Value::Null);

                        // Check if this is a Task tool call (sub-agent spawn)
                        if name == "Task" {
                            if let Some(sub_agent_spawn) =
                                extract_sub_agent_spawn(id, &input, timestamp)
                            {
                                // Create SubAgentMeta and track it
                                let agent_meta = SubAgentMeta::new(
                                    id.clone(),
                                    input
                                        .get("subagent_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown"),
                                    input
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    input.get("prompt").and_then(|v| v.as_str()).unwrap_or(""),
                                    timestamp,
                                );
                                let agent_idx = sub_agents.len();
                                sub_agents.push(agent_meta);
                                pending_sub_agents.insert(id.clone(), agent_idx);
                                blocks.push(sub_agent_spawn);
                            }
                        }
                        // Check if this is a file edit tool
                        else if is_file_edit_tool(name) {
                            if let Some(file_edit) = extract_file_edit(name, &input, timestamp) {
                                blocks.push(file_edit);
                            }
                        }

                        // Store as pending tool call
                        pending_tool_calls.insert(
                            id.clone(),
                            PendingToolCall {
                                name: name.clone(),
                                input,
                                timestamp,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Extract a SubAgentSpawn block from a Task tool call.
fn extract_sub_agent_spawn(
    tool_id: &str,
    input: &Value,
    timestamp: DateTime<Utc>,
) -> Option<Block> {
    let agent_type = input.get("subagent_type").and_then(|v| v.as_str())?;
    let description = input
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");

    Some(Block::sub_agent_spawn(
        tool_id,
        agent_type,
        description,
        prompt,
        SubAgentStatus::Running,
        timestamp,
    ))
}

/// Check if a tool name corresponds to a file edit operation.
fn is_file_edit_tool(name: &str) -> bool {
    matches!(name, "Edit" | "Write" | "NotebookEdit")
}

/// Extract a FileEdit block from a file editing tool call.
fn extract_file_edit(tool_name: &str, input: &Value, timestamp: DateTime<Utc>) -> Option<Block> {
    match tool_name {
        "Edit" => {
            let path = input.get("file_path")?.as_str()?;
            let old_string = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_string = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let diff = format!("--- {path}\n+++ {path}\n- {old_string}\n+ {new_string}");
            Some(Block::file_edit(path, diff, timestamp))
        }
        "Write" => {
            let path = input.get("file_path")?.as_str()?;
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            // For writes, show the full content as an addition
            // Use char_indices to safely truncate at character boundaries
            let preview = if content.len() > 500 {
                let truncate_at = content
                    .char_indices()
                    .take_while(|(i, _)| *i < 500)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                format!("{}...\n[content truncated]", &content[..truncate_at])
            } else {
                content.to_string()
            };
            let diff = format!("+++ {path}\n{preview}");
            Some(Block::file_edit(path, diff, timestamp))
        }
        "NotebookEdit" => {
            let path = input.get("notebook_path")?.as_str()?;
            let new_source = input
                .get("new_source")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let edit_mode = input
                .get("edit_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("replace");
            let diff = format!("Notebook edit ({edit_mode}): {path}\n{new_source}");
            Some(Block::file_edit(path, diff, timestamp))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(".jsonl")
            .tempfile()
            .unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_claude_parser_name() {
        let parser = ClaudeParser::new();
        assert_eq!(parser.name(), "claude");
    }

    #[test]
    fn test_claude_parser_can_parse() {
        let parser = ClaudeParser::new();

        assert!(parser.can_parse(Path::new("session.jsonl")));
        assert!(parser.can_parse(Path::new("/path/to/file.jsonl")));
        assert!(!parser.can_parse(Path::new("session.json")));
        assert!(!parser.can_parse(Path::new("session.txt")));
        assert!(!parser.can_parse(Path::new("session")));
    }

    #[test]
    fn test_parse_user_message() {
        let content = r#"{"type":"user","sessionId":"test-123","cwd":"/project","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Hello, help me write code"}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.id, "test-123");
        assert_eq!(session.project, Some("/project".to_string()));
        assert_eq!(session.blocks.len(), 1);

        match &session.blocks[0] {
            Block::UserPrompt { content, .. } => {
                assert_eq!(content, "Hello, help me write code");
            }
            _ => panic!("Expected UserPrompt block"),
        }
    }

    #[test]
    fn test_parse_assistant_text_response() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Hello"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi there! How can I help you?"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.blocks.len(), 2);

        match &session.blocks[1] {
            Block::AssistantResponse { content, .. } => {
                assert_eq!(content, "Hi there! How can I help you?");
            }
            _ => panic!("Expected AssistantResponse block"),
        }
    }

    #[test]
    fn test_parse_tool_call_with_result() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Read the file"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{"role":"user","content":[{"tool_use_id":"tool-1","type":"tool_result","content":"fn main() {}"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Should have user prompt and tool call (with result merged)
        assert_eq!(session.blocks.len(), 2);

        match &session.blocks[1] {
            Block::ToolCall {
                name,
                input,
                output,
                ..
            } => {
                assert_eq!(name, "Read");
                assert_eq!(input["file_path"], "/src/main.rs");
                assert_eq!(
                    output.as_ref().unwrap(),
                    &Value::String("fn main() {}".to_string())
                );
            }
            _ => panic!("Expected ToolCall block"),
        }
    }

    #[test]
    fn test_parse_thinking_block() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Complex problem"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"thinking","text":"Let me think about this carefully..."},{"type":"text","text":"Here's my answer"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.blocks.len(), 3);

        match &session.blocks[1] {
            Block::Thinking { content, .. } => {
                assert_eq!(content, "Let me think about this carefully...");
            }
            _ => panic!("Expected Thinking block"),
        }
    }

    #[test]
    fn test_parse_file_edit() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Edit the file"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"Edit","input":{"file_path":"/src/main.rs","old_string":"old code","new_string":"new code"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{"role":"user","content":[{"tool_use_id":"tool-1","type":"tool_result","content":"File edited"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Should have user prompt, file edit, and tool call
        assert_eq!(session.blocks.len(), 3);

        match &session.blocks[1] {
            Block::FileEdit { path, diff, .. } => {
                assert_eq!(path, "/src/main.rs");
                assert!(diff.contains("old code"));
                assert!(diff.contains("new code"));
            }
            _ => panic!("Expected FileEdit block"),
        }
    }

    #[test]
    fn test_skip_meta_messages() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","isMeta":true,"message":{"role":"user","content":"<local-command-caveat>Skip this</local-command-caveat>"}}
{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:01Z","message":{"role":"user","content":"Real user message"}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.blocks.len(), 1);
        match &session.blocks[0] {
            Block::UserPrompt { content, .. } => {
                assert_eq!(content, "Real user message");
            }
            _ => panic!("Expected UserPrompt block"),
        }
    }

    #[test]
    fn test_skip_command_messages() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"<command-name>/clear</command-name>"}}
{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:01Z","message":{"role":"user","content":"Real user message"}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.blocks.len(), 1);
        match &session.blocks[0] {
            Block::UserPrompt { content, .. } => {
                assert_eq!(content, "Real user message");
            }
            _ => panic!("Expected UserPrompt block"),
        }
    }

    #[test]
    fn test_empty_session_succeeds_with_zero_blocks() {
        let content = r#"{"type":"progress","timestamp":"2024-01-15T10:30:00Z"}
{"type":"summary","summary":"Empty session"}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let result = parser.parse(file.path());

        // Empty sessions should succeed with zero blocks
        assert!(result.is_ok());
        let session = result.unwrap();
        assert!(session.blocks.is_empty());
    }

    #[test]
    fn test_unsupported_format() {
        let parser = ClaudeParser::new();
        let result = parser.parse(Path::new("test.txt"));

        assert!(result.is_err());
        match result {
            Err(ParseError::UnsupportedFormat { .. }) => {}
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_session_id_from_filename() {
        let content = r#"{"type":"user","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Hello"}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Session ID should be extracted from filename (tempfile gives random name)
        assert!(!session.id.is_empty());
    }

    #[test]
    fn test_parse_multiple_tool_calls() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Do multiple things"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"Glob","input":{"pattern":"*.rs"}},{"type":"tool_use","id":"tool-2","name":"Grep","input":{"pattern":"main"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{"role":"user","content":[{"tool_use_id":"tool-1","type":"tool_result","content":"main.rs\nlib.rs"}]}}
{"type":"user","timestamp":"2024-01-15T10:30:03Z","message":{"role":"user","content":[{"tool_use_id":"tool-2","type":"tool_result","content":"main.rs:1: fn main()"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Should have user prompt and 2 tool calls
        assert_eq!(session.blocks.len(), 3);

        // Verify both tool calls exist
        let tool_names: Vec<_> = session
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::ToolCall { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(tool_names.contains(&"Glob"));
        assert!(tool_names.contains(&"Grep"));
    }

    #[test]
    fn test_parse_tool_result_with_array_content() {
        // Test that tool results with array-style content are parsed correctly
        // This is the polymorphic content case that previously caused parsing failures
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Read files"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"Read","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{"role":"user","content":[{"tool_use_id":"tool-1","type":"tool_result","content":[{"type":"text","text":"fn main() {\n    println!(\"Hello\");\n}"}]}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Should have user prompt and tool call
        assert_eq!(session.blocks.len(), 2);

        match &session.blocks[1] {
            Block::ToolCall { name, output, .. } => {
                assert_eq!(name, "Read");
                // The array content should be extracted as a string
                let output_str = output.as_ref().unwrap().as_str().unwrap();
                assert!(output_str.contains("fn main()"));
                assert!(output_str.contains("println!"));
            }
            _ => panic!("Expected ToolCall block"),
        }
    }

    #[test]
    fn test_parse_tool_result_with_multiple_text_blocks() {
        // Test array content with multiple text blocks joined by newline
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Read files"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"Read","input":{"file_path":"/test.txt"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:02Z","message":{"role":"user","content":[{"tool_use_id":"tool-1","type":"tool_result","content":[{"type":"text","text":"First part"},{"type":"text","text":"Second part"}]}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.blocks.len(), 2);

        match &session.blocks[1] {
            Block::ToolCall { output, .. } => {
                let output_str = output.as_ref().unwrap().as_str().unwrap();
                // Multiple text blocks should be joined with newlines
                assert!(output_str.contains("First part"));
                assert!(output_str.contains("Second part"));
                assert_eq!(output_str, "First part\nSecond part");
            }
            _ => panic!("Expected ToolCall block"),
        }
    }

    #[test]
    fn test_tool_result_content_to_string() {
        // Unit test for ToolResultContent::to_string()
        let string_content = ToolResultContent::String("test output".to_string());
        assert_eq!(string_content.to_string(), "test output");

        let array_content = ToolResultContent::Array(vec![
            ToolResultContentBlock {
                block_type: Some("text".to_string()),
                text: Some("line 1".to_string()),
            },
            ToolResultContentBlock {
                block_type: Some("text".to_string()),
                text: Some("line 2".to_string()),
            },
        ]);
        assert_eq!(array_content.to_string(), "line 1\nline 2");

        // Test with empty array
        let empty_array = ToolResultContent::Array(vec![]);
        assert_eq!(empty_array.to_string(), "");

        // Test with None text fields
        let array_with_none = ToolResultContent::Array(vec![
            ToolResultContentBlock {
                block_type: Some("text".to_string()),
                text: None,
            },
            ToolResultContentBlock {
                block_type: Some("text".to_string()),
                text: Some("only this".to_string()),
            },
        ]);
        assert_eq!(array_with_none.to_string(), "only this");
    }

    #[test]
    fn test_parse_task_tool_creates_sub_agent_spawn_block() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Explore the codebase"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"text","text":"Let me explore."},{"type":"tool_use","id":"task-1","name":"Task","input":{"subagent_type":"Explore","description":"Explore codebase","prompt":"Search for main entry points"}}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Should have: user prompt, assistant response, sub-agent spawn, and tool call
        assert_eq!(session.blocks.len(), 4);

        // Verify SubAgentSpawn block
        let mut found_sub_agent = false;
        for block in &session.blocks {
            if let Block::SubAgentSpawn {
                agent_id,
                agent_type,
                description,
                prompt,
                status,
                ..
            } = block
            {
                assert_eq!(agent_id, "task-1");
                assert_eq!(agent_type, "Explore");
                assert_eq!(description, "Explore codebase");
                assert_eq!(prompt, "Search for main entry points");
                assert_eq!(*status, SubAgentStatus::Running);
                found_sub_agent = true;
            }
        }
        assert!(found_sub_agent, "Expected SubAgentSpawn block");

        // Verify SubAgentMeta in session
        assert_eq!(session.sub_agents.len(), 1);
        assert_eq!(session.sub_agents[0].id, "task-1");
        assert_eq!(session.sub_agents[0].agent_type, "Explore");
        assert_eq!(session.sub_agents[0].status, SubAgentStatus::Running);
    }

    #[test]
    fn test_parse_task_tool_with_result_completes_sub_agent() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Explore"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{"subagent_type":"Explore","description":"Explore","prompt":"Find files"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:10Z","message":{"role":"user","content":[{"tool_use_id":"task-1","type":"tool_result","content":"Found: main.rs, lib.rs"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Verify SubAgentMeta is completed
        assert_eq!(session.sub_agents.len(), 1);
        assert_eq!(session.sub_agents[0].status, SubAgentStatus::Completed);
        assert_eq!(
            session.sub_agents[0].result.as_ref().unwrap(),
            "Found: main.rs, lib.rs"
        );
        assert!(session.sub_agents[0].completed_at.is_some());

        // Verify SubAgentSpawn block status is updated
        for block in &session.blocks {
            if let Block::SubAgentSpawn {
                agent_id, status, ..
            } = block
            {
                if agent_id == "task-1" {
                    assert_eq!(*status, SubAgentStatus::Completed);
                }
            }
        }
    }

    #[test]
    fn test_parse_task_tool_with_error_fails_sub_agent() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Run task"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{"subagent_type":"general-purpose","description":"Implement","prompt":"Write code"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:10Z","message":{"role":"user","content":[{"tool_use_id":"task-1","type":"tool_result","is_error":true,"content":"Timeout error"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Verify SubAgentMeta is failed
        assert_eq!(session.sub_agents.len(), 1);
        assert_eq!(session.sub_agents[0].status, SubAgentStatus::Failed);
        assert_eq!(
            session.sub_agents[0].result.as_ref().unwrap(),
            "Timeout error"
        );

        // Verify SubAgentSpawn block status is updated
        for block in &session.blocks {
            if let Block::SubAgentSpawn {
                agent_id, status, ..
            } = block
            {
                if agent_id == "task-1" {
                    assert_eq!(*status, SubAgentStatus::Failed);
                }
            }
        }
    }

    #[test]
    fn test_parse_multiple_sub_agents() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Help"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{"subagent_type":"Explore","description":"Explore","prompt":"Find files"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:10Z","message":{"role":"user","content":[{"tool_use_id":"task-1","type":"tool_result","content":"Found files"}]}}
{"type":"assistant","timestamp":"2024-01-15T10:30:11Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"task-2","name":"Task","input":{"subagent_type":"Plan","description":"Plan","prompt":"Make plan"}}]}}
{"type":"user","timestamp":"2024-01-15T10:30:20Z","message":{"role":"user","content":[{"tool_use_id":"task-2","type":"tool_result","content":"Plan complete"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Both sub-agents should be tracked and completed
        assert_eq!(session.sub_agents.len(), 2);
        assert_eq!(session.sub_agents[0].agent_type, "Explore");
        assert_eq!(session.sub_agents[0].status, SubAgentStatus::Completed);
        assert_eq!(session.sub_agents[1].agent_type, "Plan");
        assert_eq!(session.sub_agents[1].status, SubAgentStatus::Completed);

        // Count SubAgentSpawn blocks
        let sub_agent_blocks: Vec<_> = session
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::SubAgentSpawn { .. }))
            .collect();
        assert_eq!(sub_agent_blocks.len(), 2);
    }

    #[test]
    fn test_parse_sub_agent_without_result_stays_running() {
        let content = r#"{"type":"user","sessionId":"test-123","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Start task"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"task-1","name":"Task","input":{"subagent_type":"Explore","description":"Explore","prompt":"Find files"}}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        // Sub-agent should still be running (no result received)
        assert_eq!(session.sub_agents.len(), 1);
        assert_eq!(session.sub_agents[0].status, SubAgentStatus::Running);
        assert!(session.sub_agents[0].completed_at.is_none());
        assert!(session.sub_agents[0].result.is_none());
    }

    #[test]
    fn test_backwards_compatibility_old_sessions_still_parse() {
        // Old session without Task tool calls should still parse correctly
        let content = r#"{"type":"user","sessionId":"old-session","timestamp":"2024-01-15T10:30:00Z","message":{"role":"user","content":"Hello"}}
{"type":"assistant","timestamp":"2024-01-15T10:30:01Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi there!"}]}}"#;

        let file = create_test_file(content);
        let parser = ClaudeParser::new();
        let session = parser.parse(file.path()).unwrap();

        assert_eq!(session.id, "old-session");
        assert_eq!(session.blocks.len(), 2);
        assert!(session.sub_agents.is_empty());
    }

    #[test]
    fn test_sub_agent_meta_new() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let meta = SubAgentMeta::new(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find main files",
            ts,
        );

        assert_eq!(meta.id, "agent-1");
        assert_eq!(meta.agent_type, "Explore");
        assert_eq!(meta.description, "Explore codebase");
        assert_eq!(meta.prompt, "Find main files");
        assert_eq!(meta.status, SubAgentStatus::Running);
        assert_eq!(meta.spawned_at, ts);
        assert!(meta.completed_at.is_none());
        assert!(meta.result.is_none());
    }

    #[test]
    fn test_sub_agent_meta_complete() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let completed_ts = DateTime::parse_from_rfc3339("2024-01-15T10:31:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let mut meta = SubAgentMeta::new("agent-1", "Explore", "Explore", "Find files", ts);
        meta.complete("Found 5 files", completed_ts);

        assert_eq!(meta.status, SubAgentStatus::Completed);
        assert_eq!(meta.completed_at, Some(completed_ts));
        assert_eq!(meta.result, Some("Found 5 files".to_string()));
    }

    #[test]
    fn test_sub_agent_meta_fail() {
        let ts = DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let failed_ts = DateTime::parse_from_rfc3339("2024-01-15T10:31:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let mut meta =
            SubAgentMeta::new("agent-1", "general-purpose", "Implement", "Write code", ts);
        meta.fail("Timeout error", failed_ts);

        assert_eq!(meta.status, SubAgentStatus::Failed);
        assert_eq!(meta.completed_at, Some(failed_ts));
        assert_eq!(meta.result, Some("Timeout error".to_string()));
    }
}
