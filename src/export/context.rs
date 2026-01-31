//! Context export formatting for sessions.
//!
//! This module provides functionality to format session content as markdown
//! suitable for pasting into a new Claude Code session or sharing.

use crate::parser::{Block, Session};

/// Options for context formatting.
#[derive(Debug, Clone, Default)]
pub struct ContextOptions {
    /// Include thinking blocks in output.
    pub include_thinking: bool,
    /// Include full tool outputs (vs summaries only).
    pub include_full_tool_output: bool,
    /// Maximum lines to include from tool outputs.
    pub max_tool_output_lines: usize,
}

impl ContextOptions {
    /// Create default options suitable for clipboard context.
    pub fn for_clipboard() -> Self {
        Self {
            include_thinking: false,
            include_full_tool_output: false,
            max_tool_output_lines: 10,
        }
    }
}

/// Result of formatting a session context.
#[derive(Debug, Clone)]
pub struct ContextFormat {
    /// The formatted markdown content.
    pub content: String,
    /// Number of messages included.
    pub message_count: usize,
    /// Estimated token count (~4 chars per token).
    pub estimated_tokens: usize,
}

/// Format a session as markdown context.
///
/// This produces a markdown document with:
/// - Session metadata header (project, date)
/// - User prompts and assistant responses
/// - Summarized tool results (not full outputs)
///
/// The output is suitable for pasting into a new Claude Code session
/// to provide context about previous work.
pub fn format_context(session: &Session, options: &ContextOptions) -> ContextFormat {
    let mut output = String::new();
    let mut message_count = 0;

    // Header with session metadata
    output.push_str("# Session Context\n\n");

    if let Some(ref project) = session.project {
        output.push_str(&format!("**Project:** {}\n", project));
    }
    output.push_str(&format!(
        "**Date:** {}\n",
        session.started_at.format("%Y-%m-%d %H:%M UTC")
    ));
    output.push_str("\n---\n\n");

    // Process blocks
    for block in &session.blocks {
        match block {
            Block::UserPrompt { content, .. } => {
                output.push_str("## User\n\n");
                output.push_str(content);
                output.push_str("\n\n");
                message_count += 1;
            }
            Block::AssistantResponse { content, .. } => {
                output.push_str("## Assistant\n\n");
                output.push_str(content);
                output.push_str("\n\n");
                message_count += 1;
            }
            Block::Thinking { content, .. } if options.include_thinking => {
                output.push_str("## Thinking\n\n");
                output.push_str("<details>\n<summary>Internal reasoning</summary>\n\n");
                output.push_str(content);
                output.push_str("\n\n</details>\n\n");
            }
            Block::ToolCall {
                name,
                input,
                output: tool_output,
                ..
            } => {
                // Include a summary of tool usage
                output.push_str(&format!("### Tool: {}\n\n", name));

                // Summarize input based on tool type
                let input_summary = summarize_tool_input(name, input);
                output.push_str(&format!("**Input:** {}\n", input_summary));

                // Include abbreviated output if available
                if let Some(out) = tool_output {
                    let output_summary = summarize_tool_output(out, options.max_tool_output_lines);
                    if !output_summary.is_empty() {
                        output.push_str(&format!("**Result:** {}\n", output_summary));
                    }
                }
                output.push('\n');
            }
            Block::FileEdit { path, .. } => {
                output.push_str(&format!("### File Edit: `{}`\n\n", path));
            }
            _ => {} // Skip thinking blocks if not included
        }
    }

    let estimated_tokens = estimate_tokens(&output);

    ContextFormat {
        content: output,
        message_count,
        estimated_tokens,
    }
}

/// Summarize tool input for context display.
fn summarize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("`{}`", path)
            } else {
                "(file read)".to_string()
            }
        }
        "Write" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("`{}`", path)
            } else {
                "(file write)".to_string()
            }
        }
        "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("`{}`", path)
            } else {
                "(file edit)".to_string()
            }
        }
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let truncated = truncate_str(cmd, 80);
                format!("`{}`", truncated)
            } else {
                "(command)".to_string()
            }
        }
        "Grep" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("pattern: `{}`", truncate_str(pattern, 50))
            } else {
                "(search)".to_string()
            }
        }
        "Glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("pattern: `{}`", pattern)
            } else {
                "(glob)".to_string()
            }
        }
        "Task" => {
            let description = input
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("(sub-agent)");
            truncate_str(description, 60).to_string()
        }
        _ => {
            // For unknown tools, just show they were called
            format!("({})", tool_name)
        }
    }
}

/// Summarize tool output for context display.
fn summarize_tool_output(output: &serde_json::Value, max_lines: usize) -> String {
    // Convert to string representation
    let text = if let Some(s) = output.as_str() {
        s.to_string()
    } else {
        // For non-string outputs, just note the type
        if output.is_object() {
            return "(object result)".to_string();
        } else if output.is_array() {
            return format!(
                "({} items)",
                output.as_array().map(|a| a.len()).unwrap_or(0)
            );
        } else if output.is_boolean() || output.is_number() {
            return output.to_string();
        } else {
            return "(result)".to_string();
        }
    };

    // Count lines and truncate if needed
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        if text.len() > 200 {
            format!("{}...", truncate_str(&text, 200))
        } else {
            text
        }
    } else {
        format!(
            "{}... ({} more lines)",
            lines[..max_lines].join("\n"),
            lines.len() - max_lines
        )
    }
}

/// Truncate a string at a word boundary if possible.
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }

    // Find a good break point (space, newline)
    let search_slice = &s[..max_len];
    if let Some(pos) = search_slice.rfind(|c: char| c.is_whitespace()) {
        &s[..pos]
    } else {
        &s[..max_len]
    }
}

/// Estimate token count (~4 characters per token on average).
fn estimate_tokens(text: &str) -> usize {
    // This is a rough estimate. Real tokenization varies by model.
    // Using ~4 chars per token as a reasonable approximation.
    text.len() / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn test_format_context_basic() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts).with_project("my-project");
        session.add_block(Block::user_prompt("Write hello world", ts));
        session.add_block(Block::assistant_response("Here's the code:", ts));

        let options = ContextOptions::for_clipboard();
        let result = format_context(&session, &options);

        assert!(result.content.contains("# Session Context"));
        assert!(result.content.contains("**Project:** my-project"));
        assert!(result.content.contains("## User"));
        assert!(result.content.contains("Write hello world"));
        assert!(result.content.contains("## Assistant"));
        assert!(result.content.contains("Here's the code:"));
        assert_eq!(result.message_count, 2);
    }

    #[test]
    fn test_format_context_excludes_thinking_by_default() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts);
        session.add_block(Block::thinking("Let me think about this...", ts));
        session.add_block(Block::assistant_response("Response", ts));

        let options = ContextOptions::for_clipboard();
        let result = format_context(&session, &options);

        assert!(!result.content.contains("Let me think about this"));
        assert!(result.content.contains("Response"));
    }

    #[test]
    fn test_format_context_includes_thinking_when_enabled() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts);
        session.add_block(Block::thinking("Let me think about this...", ts));

        let options = ContextOptions {
            include_thinking: true,
            ..Default::default()
        };
        let result = format_context(&session, &options);

        assert!(result.content.contains("## Thinking"));
        assert!(result.content.contains("Let me think about this"));
    }

    #[test]
    fn test_format_context_tool_calls() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts);
        session.add_block(Block::tool_call(
            "Read",
            json!({"file_path": "/src/main.rs"}),
            Some(json!("fn main() {}")),
            ts,
        ));

        let options = ContextOptions::for_clipboard();
        let result = format_context(&session, &options);

        assert!(result.content.contains("### Tool: Read"));
        assert!(result.content.contains("`/src/main.rs`"));
    }

    #[test]
    fn test_format_context_file_edit() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts);
        session.add_block(Block::file_edit("src/lib.rs", "+new line", ts));

        let options = ContextOptions::for_clipboard();
        let result = format_context(&session, &options);

        assert!(result.content.contains("### File Edit: `src/lib.rs`"));
    }

    #[test]
    fn test_summarize_tool_input_read() {
        let input = json!({"file_path": "/path/to/file.rs"});
        let summary = summarize_tool_input("Read", &input);
        assert_eq!(summary, "`/path/to/file.rs`");
    }

    #[test]
    fn test_summarize_tool_input_bash() {
        let input = json!({"command": "cargo build"});
        let summary = summarize_tool_input("Bash", &input);
        assert_eq!(summary, "`cargo build`");
    }

    #[test]
    fn test_summarize_tool_input_bash_truncates() {
        let long_cmd = "echo ".to_string() + &"x".repeat(200);
        let input = json!({"command": long_cmd});
        let summary = summarize_tool_input("Bash", &input);
        assert!(summary.len() < 100);
    }

    #[test]
    fn test_summarize_tool_output_string() {
        let output = json!("success");
        let summary = summarize_tool_output(&output, 10);
        assert_eq!(summary, "success");
    }

    #[test]
    fn test_summarize_tool_output_object() {
        let output = json!({"key": "value"});
        let summary = summarize_tool_output(&output, 10);
        assert_eq!(summary, "(object result)");
    }

    #[test]
    fn test_summarize_tool_output_array() {
        let output = json!(["a", "b", "c"]);
        let summary = summarize_tool_output(&output, 10);
        assert_eq!(summary, "(3 items)");
    }

    #[test]
    fn test_estimate_tokens() {
        let text = "Hello world!"; // 12 chars
        let tokens = estimate_tokens(text);
        assert_eq!(tokens, 3); // 12 / 4 = 3
    }

    #[test]
    fn test_truncate_str_short() {
        let s = "Hello";
        assert_eq!(truncate_str(s, 10), "Hello");
    }

    #[test]
    fn test_truncate_str_at_word() {
        let s = "Hello world this is long";
        let truncated = truncate_str(s, 15);
        assert_eq!(truncated, "Hello world");
    }

    #[test]
    fn test_context_options_for_clipboard() {
        let options = ContextOptions::for_clipboard();
        assert!(!options.include_thinking);
        assert!(!options.include_full_tool_output);
        assert_eq!(options.max_tool_output_lines, 10);
    }
}
