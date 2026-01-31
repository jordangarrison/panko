//! Unified session types for AI coding agent transcripts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A parsed session from an AI coding agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    /// Unique identifier for the session.
    pub id: String,
    /// Project name or path, if available.
    pub project: Option<String>,
    /// When the session started.
    pub started_at: DateTime<Utc>,
    /// The blocks of content in the session.
    pub blocks: Vec<Block>,
    /// Sub-agents spawned during the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_agents: Vec<SubAgentMeta>,
}

/// Metadata about a sub-agent spawned during a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentMeta {
    /// Unique identifier for the sub-agent (tool call ID).
    pub id: String,
    /// Type of sub-agent (e.g., "Explore", "Plan", "Bash", "general-purpose").
    pub agent_type: String,
    /// Short description of the task.
    pub description: String,
    /// The full prompt given to the sub-agent.
    pub prompt: String,
    /// Current status of the sub-agent.
    pub status: SubAgentStatus,
    /// When the sub-agent was spawned.
    pub spawned_at: DateTime<Utc>,
    /// When the sub-agent completed (if finished).
    pub completed_at: Option<DateTime<Utc>>,
    /// The result returned by the sub-agent (if completed).
    pub result: Option<String>,
}

/// Status of a sub-agent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    /// Sub-agent is currently running.
    #[default]
    Running,
    /// Sub-agent completed successfully.
    Completed,
    /// Sub-agent encountered an error.
    Failed,
}

/// A single block of content within a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A prompt from the user.
    UserPrompt {
        content: String,
        timestamp: DateTime<Utc>,
    },
    /// A response from the AI assistant.
    AssistantResponse {
        content: String,
        timestamp: DateTime<Utc>,
    },
    /// A tool invocation by the assistant.
    ToolCall {
        name: String,
        input: Value,
        output: Option<Value>,
        timestamp: DateTime<Utc>,
    },
    /// Extended thinking content from the assistant.
    Thinking {
        content: String,
        timestamp: DateTime<Utc>,
    },
    /// A file edit operation.
    FileEdit {
        path: String,
        diff: String,
        timestamp: DateTime<Utc>,
    },
    /// A sub-agent spawn via the Task tool.
    SubAgentSpawn {
        /// Unique identifier for the sub-agent (tool call ID).
        agent_id: String,
        /// Type of sub-agent (e.g., "Explore", "Plan", "Bash").
        agent_type: String,
        /// Short description of the task.
        description: String,
        /// The full prompt given to the sub-agent.
        prompt: String,
        /// Current status of the sub-agent.
        status: SubAgentStatus,
        timestamp: DateTime<Utc>,
    },
}

impl Session {
    /// Create a new session with the given ID and start time.
    pub fn new(id: impl Into<String>, started_at: DateTime<Utc>) -> Self {
        Self {
            id: id.into(),
            project: None,
            started_at,
            blocks: Vec::new(),
            sub_agents: Vec::new(),
        }
    }

    /// Set the project name.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Add a block to the session.
    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }
}

impl Block {
    /// Get the timestamp of this block.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Block::UserPrompt { timestamp, .. } => *timestamp,
            Block::AssistantResponse { timestamp, .. } => *timestamp,
            Block::ToolCall { timestamp, .. } => *timestamp,
            Block::Thinking { timestamp, .. } => *timestamp,
            Block::FileEdit { timestamp, .. } => *timestamp,
            Block::SubAgentSpawn { timestamp, .. } => *timestamp,
        }
    }

    /// Create a new user prompt block.
    pub fn user_prompt(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Block::UserPrompt {
            content: content.into(),
            timestamp,
        }
    }

    /// Create a new assistant response block.
    pub fn assistant_response(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Block::AssistantResponse {
            content: content.into(),
            timestamp,
        }
    }

    /// Create a new tool call block.
    pub fn tool_call(
        name: impl Into<String>,
        input: Value,
        output: Option<Value>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Block::ToolCall {
            name: name.into(),
            input,
            output,
            timestamp,
        }
    }

    /// Create a new thinking block.
    pub fn thinking(content: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Block::Thinking {
            content: content.into(),
            timestamp,
        }
    }

    /// Create a new file edit block.
    pub fn file_edit(
        path: impl Into<String>,
        diff: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Block::FileEdit {
            path: path.into(),
            diff: diff.into(),
            timestamp,
        }
    }

    /// Create a new sub-agent spawn block.
    pub fn sub_agent_spawn(
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
        status: SubAgentStatus,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Block::SubAgentSpawn {
            agent_id: agent_id.into(),
            agent_type: agent_type.into(),
            description: description.into(),
            prompt: prompt.into(),
            status,
            timestamp,
        }
    }
}

impl SubAgentMeta {
    /// Create a new sub-agent metadata entry.
    pub fn new(
        id: impl Into<String>,
        agent_type: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
        spawned_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            agent_type: agent_type.into(),
            description: description.into(),
            prompt: prompt.into(),
            status: SubAgentStatus::Running,
            spawned_at,
            completed_at: None,
            result: None,
        }
    }

    /// Mark the sub-agent as completed with a result.
    pub fn complete(&mut self, result: impl Into<String>, completed_at: DateTime<Utc>) {
        self.status = SubAgentStatus::Completed;
        self.completed_at = Some(completed_at);
        self.result = Some(result.into());
    }

    /// Mark the sub-agent as failed with an error message.
    pub fn fail(&mut self, error: impl Into<String>, completed_at: DateTime<Utc>) {
        self.status = SubAgentStatus::Failed;
        self.completed_at = Some(completed_at);
        self.result = Some(error.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn test_session_creation() {
        let ts = test_timestamp();
        let session = Session::new("test-id", ts).with_project("my-project");

        assert_eq!(session.id, "test-id");
        assert_eq!(session.project, Some("my-project".to_string()));
        assert_eq!(session.started_at, ts);
        assert!(session.blocks.is_empty());
    }

    #[test]
    fn test_session_add_block() {
        let ts = test_timestamp();
        let mut session = Session::new("test-id", ts);

        session.add_block(Block::user_prompt("Hello", ts));
        session.add_block(Block::assistant_response("Hi there!", ts));

        assert_eq!(session.blocks.len(), 2);
    }

    #[test]
    fn test_block_timestamp() {
        let ts = test_timestamp();

        let blocks = vec![
            Block::user_prompt("test", ts),
            Block::assistant_response("test", ts),
            Block::tool_call("test", json!({}), None, ts),
            Block::thinking("test", ts),
            Block::file_edit("path", "diff", ts),
        ];

        for block in blocks {
            assert_eq!(block.timestamp(), ts);
        }
    }

    #[test]
    fn test_session_serialization() {
        let ts = test_timestamp();
        let mut session = Session::new("session-123", ts).with_project("my-app");

        session.add_block(Block::user_prompt("Write a function", ts));

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(session, deserialized);
    }

    #[test]
    fn test_block_serialization_user_prompt() {
        let ts = test_timestamp();
        let block = Block::user_prompt("Hello world", ts);

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        // Verify the JSON structure includes the type tag
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "user_prompt");
        assert_eq!(value["content"], "Hello world");
    }

    #[test]
    fn test_block_serialization_assistant_response() {
        let ts = test_timestamp();
        let block = Block::assistant_response("Here's the code", ts);

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "assistant_response");
    }

    #[test]
    fn test_block_serialization_tool_call() {
        let ts = test_timestamp();
        let block = Block::tool_call(
            "read_file",
            json!({"path": "/foo/bar.rs"}),
            Some(json!({"content": "fn main() {}"})),
            ts,
        );

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["name"], "read_file");
        assert_eq!(value["input"]["path"], "/foo/bar.rs");
        assert_eq!(value["output"]["content"], "fn main() {}");
    }

    #[test]
    fn test_block_serialization_tool_call_no_output() {
        let ts = test_timestamp();
        let block = Block::tool_call("bash", json!({"command": "ls"}), None, ts);

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["output"].is_null());
    }

    #[test]
    fn test_block_serialization_thinking() {
        let ts = test_timestamp();
        let block = Block::thinking("Let me consider the approach...", ts);

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "thinking");
    }

    #[test]
    fn test_block_serialization_file_edit() {
        let ts = test_timestamp();
        let diff = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new";
        let block = Block::file_edit("src/main.rs", diff, ts);

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "file_edit");
        assert_eq!(value["path"], "src/main.rs");
    }

    #[test]
    fn test_full_session_roundtrip() {
        let ts = test_timestamp();
        let mut session = Session::new("complex-session", ts).with_project("test-project");

        session.add_block(Block::user_prompt("Write a hello world program", ts));
        session.add_block(Block::thinking("I'll create a simple Rust program", ts));
        session.add_block(Block::tool_call(
            "write_file",
            json!({"path": "main.rs", "content": "fn main() { println!(\"Hello!\"); }"}),
            Some(json!({"success": true})),
            ts,
        ));
        session.add_block(Block::file_edit(
            "main.rs",
            "+fn main() { println!(\"Hello!\"); }",
            ts,
        ));
        session.add_block(Block::assistant_response(
            "I've created a hello world program for you.",
            ts,
        ));

        let json = serde_json::to_string_pretty(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(session, deserialized);
        assert_eq!(deserialized.blocks.len(), 5);
    }

    #[test]
    fn test_block_serialization_sub_agent_spawn() {
        let ts = test_timestamp();
        let block = Block::sub_agent_spawn(
            "agent-123",
            "Explore",
            "Explore codebase",
            "Find main entry points",
            SubAgentStatus::Running,
            ts,
        );

        let json = serde_json::to_string(&block).unwrap();
        let deserialized: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(block, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "sub_agent_spawn");
        assert_eq!(value["agent_id"], "agent-123");
        assert_eq!(value["agent_type"], "Explore");
        assert_eq!(value["description"], "Explore codebase");
        assert_eq!(value["prompt"], "Find main entry points");
        assert_eq!(value["status"], "running");
    }

    #[test]
    fn test_sub_agent_status_serialization() {
        let running = SubAgentStatus::Running;
        let completed = SubAgentStatus::Completed;
        let failed = SubAgentStatus::Failed;

        assert_eq!(serde_json::to_string(&running).unwrap(), r#""running""#);
        assert_eq!(serde_json::to_string(&completed).unwrap(), r#""completed""#);
        assert_eq!(serde_json::to_string(&failed).unwrap(), r#""failed""#);

        // Deserialize
        let running_de: SubAgentStatus = serde_json::from_str(r#""running""#).unwrap();
        assert_eq!(running_de, SubAgentStatus::Running);

        let completed_de: SubAgentStatus = serde_json::from_str(r#""completed""#).unwrap();
        assert_eq!(completed_de, SubAgentStatus::Completed);

        let failed_de: SubAgentStatus = serde_json::from_str(r#""failed""#).unwrap();
        assert_eq!(failed_de, SubAgentStatus::Failed);
    }

    #[test]
    fn test_sub_agent_meta_serialization() {
        let ts = test_timestamp();
        let meta = SubAgentMeta::new(
            "agent-1",
            "Plan",
            "Plan feature",
            "Design implementation",
            ts,
        );

        let json = serde_json::to_string_pretty(&meta).unwrap();
        let deserialized: SubAgentMeta = serde_json::from_str(&json).unwrap();

        assert_eq!(meta, deserialized);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["id"], "agent-1");
        assert_eq!(value["agent_type"], "Plan");
        assert_eq!(value["description"], "Plan feature");
        assert_eq!(value["prompt"], "Design implementation");
        assert_eq!(value["status"], "running");
        assert!(value["completed_at"].is_null());
        assert!(value["result"].is_null());
    }

    #[test]
    fn test_session_with_sub_agents() {
        let ts = test_timestamp();
        let mut session = Session::new("session-with-agents", ts);

        session.add_block(Block::user_prompt("Help me explore", ts));
        session.add_block(Block::sub_agent_spawn(
            "agent-1",
            "Explore",
            "Explore codebase",
            "Find files",
            SubAgentStatus::Completed,
            ts,
        ));

        let mut meta =
            SubAgentMeta::new("agent-1", "Explore", "Explore codebase", "Find files", ts);
        meta.complete("Found 10 files", ts);
        session.sub_agents.push(meta);

        let json = serde_json::to_string_pretty(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(session, deserialized);
        assert_eq!(deserialized.sub_agents.len(), 1);
        assert_eq!(deserialized.sub_agents[0].status, SubAgentStatus::Completed);
    }

    #[test]
    fn test_session_without_sub_agents_omits_field() {
        let ts = test_timestamp();
        let session = Session::new("no-agents", ts);

        let json = serde_json::to_string(&session).unwrap();

        // The sub_agents field should be omitted when empty
        assert!(!json.contains("sub_agents"));
    }

    #[test]
    fn test_block_timestamp_sub_agent_spawn() {
        let ts = test_timestamp();
        let block = Block::sub_agent_spawn(
            "agent-1",
            "Explore",
            "Explore",
            "Find files",
            SubAgentStatus::Running,
            ts,
        );

        assert_eq!(block.timestamp(), ts);
    }
}
