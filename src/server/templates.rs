//! Template rendering using minijinja with embedded templates.

use minijinja::{Environment, Error as JinjaError, ErrorKind};
use pulldown_cmark::{html, Options, Parser};
use rust_embed::Embed;
use serde::Serialize;

use crate::parser::{Block, Session, SubAgentMeta};

/// Embedded HTML templates.
#[derive(Embed)]
#[folder = "templates/"]
pub struct Templates;

/// A template engine for rendering sessions.
pub struct TemplateEngine {
    env: Environment<'static>,
}

impl TemplateEngine {
    /// Create a new template engine with embedded templates.
    pub fn new() -> Result<Self, JinjaError> {
        let mut env = Environment::new();

        // Load embedded templates
        for file in Templates::iter() {
            let filename = file.to_string();
            if let Some(content) = Templates::get(&filename) {
                let template_str = std::str::from_utf8(content.data.as_ref())
                    .map_err(|_| JinjaError::from(ErrorKind::InvalidOperation))?;
                env.add_template_owned(filename, template_str.to_string())?;
            }
        }

        Ok(Self { env })
    }

    /// Render a session to HTML.
    pub fn render_session(&self, session: &Session) -> Result<String, JinjaError> {
        let template = self.env.get_template("session.html")?;
        let view = SessionView::from_session(session);
        template.render(minijinja::context! { session => view })
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new().expect("failed to initialize template engine")
    }
}

/// A view model for rendering a session in templates.
#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub id: String,
    pub project: Option<String>,
    pub started_at: String,
    pub blocks: Vec<BlockView>,
}

impl SessionView {
    /// Create a view model from a session.
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            project: session.project.clone(),
            started_at: session.started_at.to_rfc3339(),
            blocks: session
                .blocks
                .iter()
                .map(|block| BlockView::from_block_with_agents(block, &session.sub_agents))
                .collect(),
        }
    }
}

/// A view model for rendering a block in templates.
#[derive(Debug, Clone, Serialize)]
pub struct BlockView {
    /// The block type (user_prompt, assistant_response, etc.)
    #[serde(rename = "type")]
    pub block_type: String,
    /// Timestamp as ISO 8601 string.
    pub timestamp: String,
    /// Content rendered as HTML (for UserPrompt, AssistantResponse, Thinking).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_html: Option<String>,
    /// Tool name (for ToolCall).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool input (for ToolCall).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// Tool output (for ToolCall).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Number of lines in the output JSON (for large output detection).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_lines: Option<usize>,
    /// File path (for FileEdit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Diff content (for FileEdit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// Sub-agent ID (for SubAgentSpawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Sub-agent type (for SubAgentSpawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// Sub-agent description (for SubAgentSpawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Sub-agent prompt (for SubAgentSpawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Sub-agent status (for SubAgentSpawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_status: Option<String>,
    /// Sub-agent result (for SubAgentSpawn, looked up from session.sub_agents).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_result: Option<String>,
}

impl BlockView {
    /// Create a view model from a block with access to sub-agent metadata.
    pub fn from_block_with_agents(block: &Block, sub_agents: &[SubAgentMeta]) -> Self {
        match block {
            Block::UserPrompt { content, timestamp } => Self {
                block_type: "user_prompt".to_string(),
                timestamp: timestamp.to_rfc3339(),
                content_html: Some(markdown_to_html(content)),
                name: None,
                input: None,
                output: None,
                output_lines: None,
                path: None,
                diff: None,
                agent_id: None,
                agent_type: None,
                description: None,
                prompt: None,
                agent_status: None,
                agent_result: None,
            },
            Block::AssistantResponse { content, timestamp } => Self {
                block_type: "assistant_response".to_string(),
                timestamp: timestamp.to_rfc3339(),
                content_html: Some(markdown_to_html(content)),
                name: None,
                input: None,
                output: None,
                output_lines: None,
                path: None,
                diff: None,
                agent_id: None,
                agent_type: None,
                description: None,
                prompt: None,
                agent_status: None,
                agent_result: None,
            },
            Block::Thinking { content, timestamp } => Self {
                block_type: "thinking".to_string(),
                timestamp: timestamp.to_rfc3339(),
                content_html: Some(markdown_to_html(content)),
                name: None,
                input: None,
                output: None,
                output_lines: None,
                path: None,
                diff: None,
                agent_id: None,
                agent_type: None,
                description: None,
                prompt: None,
                agent_status: None,
                agent_result: None,
            },
            Block::ToolCall {
                name,
                input,
                output,
                timestamp,
            } => {
                // Count lines in the output for large output detection.
                // For string values, count content lines (including escaped \n).
                // For objects/arrays, count JSON lines when pretty-printed.
                let output_lines = output.as_ref().map(|o| {
                    match o {
                        serde_json::Value::String(s) => {
                            // Count actual newlines plus escaped \n sequences
                            let actual_newlines = s.lines().count();
                            let escaped_newlines = s.matches("\\n").count();
                            actual_newlines + escaped_newlines
                        }
                        _ => serde_json::to_string_pretty(o)
                            .map(|s| s.lines().count())
                            .unwrap_or(0),
                    }
                });
                Self {
                    block_type: "tool_call".to_string(),
                    timestamp: timestamp.to_rfc3339(),
                    content_html: None,
                    name: Some(name.clone()),
                    input: Some(input.clone()),
                    output: output.clone(),
                    output_lines,
                    path: None,
                    diff: None,
                    agent_id: None,
                    agent_type: None,
                    description: None,
                    prompt: None,
                    agent_status: None,
                    agent_result: None,
                }
            }
            Block::FileEdit {
                path,
                diff,
                timestamp,
            } => Self {
                block_type: "file_edit".to_string(),
                timestamp: timestamp.to_rfc3339(),
                content_html: None,
                name: None,
                input: None,
                output: None,
                output_lines: None,
                path: Some(path.clone()),
                diff: Some(diff.clone()),
                agent_id: None,
                agent_type: None,
                description: None,
                prompt: None,
                agent_status: None,
                agent_result: None,
            },
            Block::SubAgentSpawn {
                agent_id,
                agent_type,
                description,
                prompt,
                status,
                timestamp,
            } => {
                // Look up the result from sub_agents if available
                let agent_result = sub_agents
                    .iter()
                    .find(|a| a.id == *agent_id)
                    .and_then(|a| a.result.clone());

                Self {
                    block_type: "sub_agent_spawn".to_string(),
                    timestamp: timestamp.to_rfc3339(),
                    content_html: Some(markdown_to_html(prompt)),
                    name: None,
                    input: None,
                    output: None,
                    output_lines: None,
                    path: None,
                    diff: None,
                    agent_id: Some(agent_id.clone()),
                    agent_type: Some(agent_type.clone()),
                    description: Some(description.clone()),
                    prompt: Some(prompt.clone()),
                    agent_status: Some(format!("{:?}", status).to_lowercase()),
                    agent_result,
                }
            }
        }
    }

    /// Create a view model from a block (without sub-agent metadata lookup).
    pub fn from_block(block: &Block) -> Self {
        Self::from_block_with_agents(block, &[])
    }
}

/// Convert markdown text to HTML.
pub fn markdown_to_html(text: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn test_templates_embedded() {
        assert!(Templates::get("session.html").is_some());
        assert!(Templates::get("block.html").is_some());
    }

    #[test]
    fn test_template_engine_creation() {
        let engine = TemplateEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_markdown_to_html_simple() {
        let result = markdown_to_html("Hello **world**!");
        assert!(result.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_markdown_to_html_code_block() {
        let result = markdown_to_html("```rust\nfn main() {}\n```");
        assert!(result.contains("<code"));
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let result = markdown_to_html("[link](https://example.com)");
        assert!(result.contains("href=\"https://example.com\""));
    }

    #[test]
    fn test_block_view_user_prompt() {
        let ts = test_timestamp();
        let block = Block::user_prompt("Hello **world**", ts);
        let view = BlockView::from_block(&block);

        assert_eq!(view.block_type, "user_prompt");
        assert!(view.content_html.is_some());
        assert!(view.content_html.unwrap().contains("<strong>"));
    }

    #[test]
    fn test_block_view_tool_call() {
        let ts = test_timestamp();
        let block = Block::tool_call(
            "read_file",
            serde_json::json!({"path": "/test"}),
            Some(serde_json::json!({"content": "data"})),
            ts,
        );
        let view = BlockView::from_block(&block);

        assert_eq!(view.block_type, "tool_call");
        assert_eq!(view.name, Some("read_file".to_string()));
        assert!(view.input.is_some());
        assert!(view.output.is_some());
        // output_lines should be calculated
        assert!(view.output_lines.is_some());
    }

    #[test]
    fn test_block_view_tool_call_output_lines_json() {
        let ts = test_timestamp();
        // Create a multi-line JSON output
        let output = serde_json::json!({
            "line1": "value1",
            "line2": "value2",
            "line3": "value3",
            "nested": {
                "a": 1,
                "b": 2
            }
        });
        let block = Block::tool_call("test", serde_json::json!({}), Some(output), ts);
        let view = BlockView::from_block(&block);

        // Pretty-printed JSON should have multiple lines
        assert!(view.output_lines.is_some());
        let lines = view.output_lines.unwrap();
        assert!(lines > 1, "Expected multiple lines, got {}", lines);
    }

    #[test]
    fn test_block_view_tool_call_output_lines_string() {
        let ts = test_timestamp();
        // Create a multi-line string output (like file content)
        let output = serde_json::json!(
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10"
        );
        let block = Block::tool_call("Read", serde_json::json!({}), Some(output), ts);
        let view = BlockView::from_block(&block);

        // String content lines should be counted (10 lines = 9 actual \n + 1 base)
        assert!(view.output_lines.is_some());
        let lines = view.output_lines.unwrap();
        assert_eq!(lines, 10, "Expected 10 lines, got {}", lines);
    }

    #[test]
    fn test_block_view_tool_call_output_lines_escaped_string() {
        let ts = test_timestamp();
        // Create a string with escaped newlines (like JSON-encoded content)
        // This simulates: {"content": "line 1\nline 2\nline 3"}
        let output = serde_json::json!(r#"{"content": "line 1\nline 2\nline 3"}"#);
        let block = Block::tool_call("Read", serde_json::json!({}), Some(output), ts);
        let view = BlockView::from_block(&block);

        // Should count escaped \n sequences
        assert!(view.output_lines.is_some());
        let lines = view.output_lines.unwrap();
        // 1 actual line + 2 escaped \n = 3
        assert_eq!(lines, 3, "Expected 3 lines, got {}", lines);
    }

    #[test]
    fn test_block_view_tool_call_no_output() {
        let ts = test_timestamp();
        let block = Block::tool_call("test", serde_json::json!({}), None, ts);
        let view = BlockView::from_block(&block);

        assert!(view.output.is_none());
        assert!(view.output_lines.is_none());
    }

    #[test]
    fn test_block_view_file_edit() {
        let ts = test_timestamp();
        let block = Block::file_edit("src/main.rs", "+fn main() {}", ts);
        let view = BlockView::from_block(&block);

        assert_eq!(view.block_type, "file_edit");
        assert_eq!(view.path, Some("src/main.rs".to_string()));
        assert_eq!(view.diff, Some("+fn main() {}".to_string()));
    }

    #[test]
    fn test_session_view() {
        let ts = test_timestamp();
        let mut session = Session::new("test-id", ts).with_project("my-project");
        session.add_block(Block::user_prompt("Hello", ts));

        let view = SessionView::from_session(&session);

        assert_eq!(view.id, "test-id");
        assert_eq!(view.project, Some("my-project".to_string()));
        assert_eq!(view.blocks.len(), 1);
    }

    #[test]
    fn test_render_session() {
        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts).with_project("test-project");
        session.add_block(Block::user_prompt("Hello **world**", ts));
        session.add_block(Block::assistant_response("Hi there!", ts));

        let engine = TemplateEngine::new().unwrap();
        let html = engine.render_session(&session);

        assert!(html.is_ok(), "render_session failed: {:?}", html.err());
        let html = html.unwrap();
        assert!(html.contains("test-session"));
        assert!(html.contains("test-project"));
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_block_view_sub_agent_spawn_basic() {
        use crate::parser::SubAgentStatus;

        let ts = test_timestamp();
        let block = Block::sub_agent_spawn(
            "agent-123",
            "Explore",
            "Search codebase",
            "Find all entry points in the project",
            SubAgentStatus::Running,
            ts,
        );
        let view = BlockView::from_block(&block);

        assert_eq!(view.block_type, "sub_agent_spawn");
        assert_eq!(view.agent_id, Some("agent-123".to_string()));
        assert_eq!(view.agent_type, Some("Explore".to_string()));
        assert_eq!(view.description, Some("Search codebase".to_string()));
        assert_eq!(
            view.prompt,
            Some("Find all entry points in the project".to_string())
        );
        assert_eq!(view.agent_status, Some("running".to_string()));
        assert!(view.agent_result.is_none()); // No result when using from_block without agents
        assert!(view.content_html.is_some()); // Prompt should be rendered as markdown
    }

    #[test]
    fn test_block_view_sub_agent_with_result() {
        use crate::parser::SubAgentStatus;

        let ts = test_timestamp();
        let block = Block::sub_agent_spawn(
            "agent-456",
            "Plan",
            "Design implementation",
            "Create a plan for adding authentication",
            SubAgentStatus::Completed,
            ts,
        );

        // Create matching sub-agent metadata with a result
        let mut meta = SubAgentMeta::new(
            "agent-456",
            "Plan",
            "Design implementation",
            "Create a plan for adding authentication",
            ts,
        );
        meta.complete("Step 1: Add login route\nStep 2: Implement JWT", ts);

        let view = BlockView::from_block_with_agents(&block, &[meta]);

        assert_eq!(view.block_type, "sub_agent_spawn");
        assert_eq!(view.agent_status, Some("completed".to_string()));
        assert_eq!(
            view.agent_result,
            Some("Step 1: Add login route\nStep 2: Implement JWT".to_string())
        );
    }

    #[test]
    fn test_block_view_sub_agent_failed() {
        use crate::parser::SubAgentStatus;

        let ts = test_timestamp();
        let block = Block::sub_agent_spawn(
            "agent-789",
            "general-purpose",
            "Implement feature",
            "Write the authentication module",
            SubAgentStatus::Failed,
            ts,
        );

        let mut meta = SubAgentMeta::new(
            "agent-789",
            "general-purpose",
            "Implement feature",
            "Write the authentication module",
            ts,
        );
        meta.fail("Timeout: Task exceeded time limit", ts);

        let view = BlockView::from_block_with_agents(&block, &[meta]);

        assert_eq!(view.agent_status, Some("failed".to_string()));
        assert_eq!(
            view.agent_result,
            Some("Timeout: Task exceeded time limit".to_string())
        );
    }

    #[test]
    fn test_session_view_with_sub_agents() {
        use crate::parser::SubAgentStatus;

        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts);

        // Add a sub-agent spawn block
        session.add_block(Block::sub_agent_spawn(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find main entry points",
            SubAgentStatus::Completed,
            ts,
        ));

        // Add matching sub-agent metadata
        let mut meta = SubAgentMeta::new(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find main entry points",
            ts,
        );
        meta.complete("Found src/main.rs and src/lib.rs", ts);
        session.sub_agents.push(meta);

        let view = SessionView::from_session(&session);

        assert_eq!(view.blocks.len(), 1);
        let block_view = &view.blocks[0];
        assert_eq!(block_view.block_type, "sub_agent_spawn");
        assert_eq!(
            block_view.agent_result,
            Some("Found src/main.rs and src/lib.rs".to_string())
        );
    }

    #[test]
    fn test_render_session_with_sub_agent() {
        use crate::parser::SubAgentStatus;

        let ts = test_timestamp();
        let mut session = Session::new("test-session", ts).with_project("test-project");

        session.add_block(Block::user_prompt("Help me explore", ts));
        session.add_block(Block::sub_agent_spawn(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find **important** files",
            SubAgentStatus::Completed,
            ts,
        ));

        let mut meta = SubAgentMeta::new(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find **important** files",
            ts,
        );
        meta.complete("Found main.rs", ts);
        session.sub_agents.push(meta);

        let engine = TemplateEngine::new().unwrap();
        let html = engine.render_session(&session);

        assert!(html.is_ok(), "render_session failed: {:?}", html.err());
        let html = html.unwrap();

        // Check sub-agent block is rendered
        assert!(html.contains("block-sub-agent"));
        assert!(html.contains("sub_agent_spawn"));
        assert!(html.contains("Explore")); // Agent type badge
        assert!(html.contains("Explore codebase")); // Description
        assert!(html.contains("Found main.rs")); // Result
        assert!(html.contains("<strong>important</strong>")); // Markdown in prompt
    }
}
