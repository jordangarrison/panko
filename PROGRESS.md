# Agent Replay Progress Log

## 2026-01-29 - Story 6: Tunnel provider trait and detection

### Summary
Implemented the tunnel provider abstraction with trait, handle struct, and detection function for installed tunnel CLIs.

### Changes
- Created `src/tunnel/mod.rs`:
  - `TunnelProvider` trait with `name()`, `display_name()`, `is_available()`, and `spawn()` methods
  - `TunnelHandle` struct holding subprocess (`Child`), public URL, and provider name
  - `TunnelHandle::stop()` for explicit termination and `Drop` impl for automatic cleanup
  - `TunnelError` enum with variants: BinaryNotFound, SpawnFailed, UrlParseFailed, ProcessExited, Timeout, NotAvailable
  - `detect_available_providers()` function that checks for cloudflared, ngrok, tailscale binaries
  - `get_provider()` function to get a provider instance by name
  - `AvailableProvider` struct for detection results
  - `binary_exists()` helper using `which` command

- Created `src/tunnel/cloudflare.rs`:
  - `CloudflareTunnel` struct implementing `TunnelProvider`
  - `is_available()` checks for cloudflared binary in PATH
  - Stub `spawn()` returning NotAvailable (full impl in Story 7)

- Created `src/tunnel/ngrok.rs`:
  - `NgrokTunnel` struct implementing `TunnelProvider`
  - `is_available()` checks for ngrok binary in PATH
  - `with_token()` constructor for authenticated usage
  - Stub `spawn()` returning NotAvailable (full impl in Story 9)

- Created `src/tunnel/tailscale.rs`:
  - `TailscaleTunnel` struct implementing `TunnelProvider`
  - `is_available()` checks for tailscale binary in PATH
  - `is_logged_in()` helper for login status (used in Story 10)
  - Stub `spawn()` returning NotAvailable (full impl in Story 10)

### Validation
```
cargo build          ✓
cargo test           ✓ (90 tests passed - 83 unit, 7 integration)
cargo clippy         ✓ (no warnings)
cargo fmt --check    ✓
```

### Acceptance Criteria
- [x] TunnelProvider trait with spawn() and is_available() methods
- [x] TunnelHandle struct that holds subprocess and public URL
- [x] Detection function that checks which tunnel CLIs are installed (cloudflared, ngrok, tailscale)
- [x] Returns list of available providers

---

## 2026-01-29 - Story 5: Local web server with view command

### Summary
Verified and documented the axum web server implementation for viewing sessions in the browser.

### Changes
- Server implementation in `src/server/mod.rs`:
  - `find_available_port()` tries ports starting from base port (default 3000)
  - `run_server()` starts axum server with graceful shutdown
  - `ServerConfig` struct for configurable port and browser opening
  - `shutdown_signal()` handles Ctrl+C for graceful termination

- Routes implementation in `src/server/routes.rs`:
  - `GET /` returns rendered session.html with parsed session data
  - `GET /assets/*` serves embedded static files (CSS, JS)
  - `AppState` holds session and template engine
  - Router tests for HTML and asset responses

- CLI integration in `src/main.rs`:
  - `view` subcommand with file path, port, and no-browser options
  - Parses session file and starts server
  - Opens browser automatically unless --no-browser flag

### Validation
```
cargo build          ✓
cargo test           ✓ (65 tests passed - 58 unit, 7 integration)
cargo clippy         ✓ (no warnings)
cargo fmt --check    ✓
agent-replay view    ✓ (end-to-end working)
```

### Acceptance Criteria
- [x] axum server starts on an available port (try 3000, increment if busy)
- [x] GET / returns rendered session.html with parsed session data
- [x] GET /assets/* serves embedded static files
- [x] Server prints URL to terminal on startup
- [x] Browser opens automatically (uses webbrowser crate)
- [x] Ctrl+C gracefully shuts down server
- [x] `agent-replay view <file>` works end-to-end

---

## 2026-01-29 - Story 4: Embedded web assets and templates

### Summary
Implemented embedded web assets and template rendering using rust-embed and minijinja.

### Changes
- Created `src/server/assets.rs`:
  - `StaticAssets` struct using rust-embed to embed `src/assets/` directory
  - `content_type()` helper function to determine MIME types
  - Unit tests verifying embedded files are accessible

- Created `src/server/templates.rs`:
  - `Templates` struct using rust-embed to embed `templates/` directory
  - `TemplateEngine` struct for minijinja template rendering
  - `SessionView` and `BlockView` view models for template rendering
  - `markdown_to_html()` function using pulldown-cmark for markdown rendering
  - Unit tests for template loading, markdown conversion, and session rendering

- Updated `src/server/mod.rs`:
  - Added `assets` and `templates` modules
  - Re-exports key types for public API

- Templates already in place:
  - `templates/session.html` - main session viewer template
  - `templates/block.html` - block partial with conditionals for each block type
  - `src/assets/styles.css` - dark theme styling (~270 lines)
  - `src/assets/htmx.min.js` - embedded for future interactivity

### Validation
```
cargo build          ✓
cargo test           ✓ (59 tests passed - 52 unit, 7 integration)
cargo clippy         ✓ (no warnings)
cargo fmt --check    ✓
```

### Acceptance Criteria
- [x] session.html template renders a full session with all block types
- [x] block.html partial templates for each block type
- [x] Minimal CSS for readable styling (not fancy, just functional)
- [x] htmx.min.js embedded for future interactivity
- [x] rust-embed configured to include assets/ and templates/ directories
- [x] Templates compile and render correctly with minijinja

---

## 2026-01-29 - Story 3: Claude Code JSONL parser

### Summary
Implemented the ClaudeParser for parsing Claude Code session JSONL files.

### Changes
- Created `src/parser/claude.rs`:
  - `ClaudeParser` struct implementing `SessionParser` trait
  - `can_parse()` detects JSONL files by extension
  - `parse()` reads JSONL line-by-line and extracts:
    - User prompts from user messages (filtering out meta/command messages)
    - Assistant text responses
    - Thinking blocks from thinking content
    - Tool calls with matched tool results
    - File edits from Edit/Write/NotebookEdit tool calls
  - Handles pending tool calls that get resolved when tool_result arrives
  - Extracts session metadata (id, project path, start timestamp)

- Created `tests/fixtures/sample_claude_session.jsonl`:
  - Sample session with user prompts, assistant responses, thinking, tool calls
  - Covers Edit and Write file operations

- Created `tests/claude_parser_integration.rs`:
  - 7 integration tests covering all block types
  - Validates metadata extraction
  - Verifies tool call results are matched correctly

### Validation
```
cargo build          ✓
cargo test           ✓ (43 tests passed - 36 unit, 7 integration)
cargo clippy         ✓ (no warnings)
cargo fmt --check    ✓
```

### Acceptance Criteria
- [x] ClaudeParser implements SessionParser trait
- [x] Correctly identifies Claude JSONL files via can_parse()
- [x] Parses user messages into UserPrompt blocks
- [x] Parses assistant messages into AssistantResponse blocks
- [x] Parses tool_use and tool_result into ToolCall blocks
- [x] Parses thinking content into Thinking blocks
- [x] Handles file edit tool calls and extracts diffs into FileEdit blocks
- [x] Integration test with sample Claude JSONL file

---

## 2026-01-29 - Story 2: Parser trait and unified types

### Summary
Implemented the parser plugin architecture with trait and common session types.

### Changes
- Created `src/parser/types.rs`:
  - `Session` struct with id, project, started_at, and blocks fields
  - `Block` enum with variants: UserPrompt, AssistantResponse, ToolCall, Thinking, FileEdit
  - Helper methods for creating blocks and accessing timestamps
  - Full serde serialization support with tagged enum variants

- Created `src/parser/error.rs`:
  - `ParseError` enum with variants: IoError, UnsupportedFormat, JsonError, MissingField, InvalidValue, InvalidTimestamp, EmptySession
  - Constructor methods for each error variant
  - Uses thiserror for derive(Error) implementation

- Updated `src/parser/mod.rs`:
  - `SessionParser` trait with name(), can_parse(), and parse() methods
  - Re-exports types and errors for public API

### Validation
```
cargo build          ✓
cargo test           ✓ (23 tests passed)
cargo clippy         ✓ (no warnings)
cargo fmt --check    ✓
```

### Acceptance Criteria
- [x] SessionParser trait defined with name(), can_parse(), and parse() methods
- [x] Session struct with id, project, started_at, and blocks fields
- [x] Block enum with variants: UserPrompt, AssistantResponse, ToolCall, Thinking, FileEdit
- [x] ParseError type with appropriate error variants
- [x] Unit tests for type serialization/deserialization
