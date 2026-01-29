//! Integration tests for the Claude Code JSONL parser.

use agent_replay::parser::{Block, ClaudeParser, SessionParser};
use std::path::Path;

const SAMPLE_SESSION: &str = "tests/fixtures/sample_claude_session.jsonl";

#[test]
fn test_parse_sample_session() {
    let parser = ClaudeParser::new();
    let path = Path::new(SAMPLE_SESSION);

    assert!(parser.can_parse(path));

    let session = parser.parse(path).expect("Failed to parse sample session");

    // Verify session metadata
    assert_eq!(session.id, "abc12345-1234-5678-abcd-123456789abc");
    assert_eq!(session.project, Some("/home/user/my-project".to_string()));

    // Verify we have the expected blocks
    // Session should contain:
    // - 2 user prompts
    // - 1 thinking block
    // - 4 assistant responses
    // - 2 file edits (Write and Edit)
    // - 2 tool calls
    assert!(
        session.blocks.len() >= 8,
        "Expected at least 8 blocks, got {}",
        session.blocks.len()
    );
}

#[test]
fn test_user_prompts_extracted() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    let user_prompts: Vec<_> = session
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::UserPrompt { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(user_prompts.len(), 2);
    assert!(user_prompts[0].contains("hello world"));
    assert!(user_prompts[1].contains("specific name"));
}

#[test]
fn test_thinking_block_extracted() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    let thinking_blocks: Vec<_> = session
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Thinking { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(thinking_blocks.len(), 1);
    assert!(thinking_blocks[0].contains("simple hello world"));
}

#[test]
fn test_tool_calls_have_outputs() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    let tool_calls: Vec<_> = session
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::ToolCall {
                name,
                input,
                output,
                ..
            } => Some((name.clone(), input.clone(), output.clone())),
            _ => None,
        })
        .collect();

    // Should have Write and Edit tool calls
    assert_eq!(tool_calls.len(), 2);

    // Verify Write tool call
    let write_call = tool_calls
        .iter()
        .find(|(name, _, _)| name == "Write")
        .expect("Write tool call not found");
    assert!(write_call.1.get("file_path").is_some());
    assert!(write_call.2.is_some()); // Should have output

    // Verify Edit tool call
    let edit_call = tool_calls
        .iter()
        .find(|(name, _, _)| name == "Edit")
        .expect("Edit tool call not found");
    assert!(edit_call.1.get("old_string").is_some());
    assert!(edit_call.1.get("new_string").is_some());
}

#[test]
fn test_file_edits_extracted() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    let file_edits: Vec<_> = session
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::FileEdit { path, diff, .. } => Some((path.clone(), diff.clone())),
            _ => None,
        })
        .collect();

    // Should have 2 file edits (Write and Edit)
    assert_eq!(file_edits.len(), 2);

    // Verify file paths
    for (path, _) in &file_edits {
        assert!(path.contains("main.rs"));
    }

    // Verify Edit diff contains old/new strings
    let edit_diff = &file_edits[1].1;
    assert!(edit_diff.contains("greet"));
}

#[test]
fn test_assistant_responses() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    let responses: Vec<_> = session
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::AssistantResponse { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();

    // Should have assistant text responses
    assert!(responses.len() >= 2);

    // Verify content of responses
    assert!(responses.iter().any(|r| r.contains("cargo run")));
    assert!(responses.iter().any(|r| r.contains("greet")));
}

#[test]
fn test_timestamps_ordered() {
    let parser = ClaudeParser::new();
    let session = parser
        .parse(Path::new(SAMPLE_SESSION))
        .expect("Failed to parse");

    // Verify session started_at is the earliest timestamp
    for block in &session.blocks {
        assert!(
            block.timestamp() >= session.started_at,
            "Block timestamp {:?} is before session start {:?}",
            block.timestamp(),
            session.started_at
        );
    }
}
