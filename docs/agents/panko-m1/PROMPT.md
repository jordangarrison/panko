# Panko - CLI Share Tool

Build a Rust CLI tool called `panko` that allows users to view and share AI coding agent sessions (Claude Code, Codex, etc.) via a local web server with optional tunnel sharing.

## Project Context

AI coding agents like Claude Code store session transcripts as JSONL files. Users want to:
1. View these sessions in a readable, navigable format
2. Share sessions with colleagues without public hosting
3. Support multiple agent formats via a plugin architecture

## Technical Stack

- **Language**: Rust
- **CLI**: clap
- **Web Server**: axum
- **Templates**: minijinja (compiled into binary)
- **Static Assets**: rust-embed
- **Prompts/UX**: inquire
- **Clipboard**: arboard

## Architecture

```
panko/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Public API
│   ├── parser/
│   │   ├── mod.rs           # Parser trait
│   │   ├── claude.rs        # Claude Code JSONL parser
│   │   └── types.rs         # Unified session types
│   ├── server/
│   │   ├── mod.rs           # Axum server setup
│   │   ├── routes.rs        # HTTP routes
│   │   └── templates.rs     # Template rendering
│   ├── tunnel/
│   │   ├── mod.rs           # Tunnel provider trait
│   │   ├── cloudflare.rs    # Cloudflare quick tunnels
│   │   ├── ngrok.rs         # ngrok support
│   │   └── tailscale.rs     # tailscale serve support
│   └── assets/              # Embedded static files
│       ├── styles.css
│       └── htmx.min.js
└── templates/
    ├── session.html         # Main session viewer
    ├── block.html           # Individual message block partial
    └── index.html           # Session list (future)
```

## Parser Plugin Architecture

Define a trait for session parsers:

```rust
pub trait SessionParser: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_parse(&self, path: &Path) -> bool;
    fn parse(&self, path: &Path) -> Result<Session, ParseError>;
}
```

Unified session types:

```rust
pub struct Session {
    pub id: String,
    pub project: Option<String>,
    pub started_at: DateTime<Utc>,
    pub blocks: Vec<Block>,
}

pub enum Block {
    UserPrompt { content: String, timestamp: DateTime<Utc> },
    AssistantResponse { content: String, timestamp: DateTime<Utc> },
    ToolCall { name: String, input: Value, output: Option<Value>, timestamp: DateTime<Utc> },
    Thinking { content: String, timestamp: DateTime<Utc> },
    FileEdit { path: String, diff: String, timestamp: DateTime<Utc> },
}
```

## CLI Commands

### `panko view <file>`
- Parse the session file
- Start local web server on available port
- Open browser automatically
- Ctrl+C to stop

### `panko share <file>`
- Same as view, plus:
- Detect available tunnel providers
- Prompt user to select if multiple available
- Spawn tunnel subprocess
- Copy public URL to clipboard
- Display URL in terminal

### `panko config`
- Set default tunnel provider
- Configure ngrok token if needed
- Persist to `~/.config/panko/config.toml`

## Web Viewer Features

- Render session as navigable blocks
- User prompts styled distinctly from assistant responses
- Tool calls shown with name, collapsible input/output
- File edits shown with syntax-highlighted diffs
- Keyboard navigation: j/k to move between blocks
- Responsive design, works on mobile

## Stories Reference

See prd.json for detailed stories and acceptance criteria. Work through stories in priority order, marking `passes: true` when complete.

## Completion

When all stories pass their acceptance criteria, output:
<promise>COMPLETE</promise>
