# Panko - Milestone 3 Progress

## Overview

**Target Version**: v0.4.0 (multi-share & context export)
**Prerequisites**: Milestone 1 (v0.1.0-v0.2.0) and Milestone 2 (v0.3.0) complete
**Status**: In Progress

## Story Progress

| ID | Title | Status | Notes |
|----|-------|--------|-------|
| 1 | Fix branding (Agent Replay → Panko) | ✅ Complete | |
| 2 | Fix tool output rendering in web UI | ⬜ Not Started | |
| 3 | Copy context to clipboard (TUI) | ⬜ Not Started | |
| 4 | Download session file | ⬜ Not Started | |
| 5 | Parse Task tool for sub-agent tracking | ⬜ Not Started | |
| 6 | Sub-agent flow visualization (web UI) | ⬜ Not Started | Depends on Story 5 |
| 7 | Refactor sharing state for multiple shares | ⬜ Not Started | |
| 8 | Share started modal with URL | ⬜ Not Started | Depends on Story 7 |
| 9 | Shares panel widget | ⬜ Not Started | Depends on Story 7 |
| 10 | Concurrent share management | ⬜ Not Started | Depends on Stories 7, 8, 9 |
| 11 | Open draft PR for M3 | ⬜ Not Started | Depends on Stories 1-10 |

## Legend

- ⬜ Not Started
- 🟡 In Progress
- ✅ Complete
- ❌ Blocked

## Work Log

### 2026-01-30 - Story 1: Fix Branding

Updated all "Agent Replay" references to "Panko":

**Files updated:**
- `templates/session.html` - title and header
- `src/tui/app.rs` - TUI window titles (2 locations)
- `src/assets/styles.css` - comment header
- `src/assets/keyboard.js` - JSDoc comment
- `docs/agents/panko-m1/PROMPT.md` - document header
- `docs/agents/panko-m1/PROGRESS.md` - document header
- `docs/agents/panko-m2/PROMPT.md` - document header and ASCII diagram
- `docs/agents/panko-m2/PROGRESS.md` - document header

**Validation results:**
- ✅ `cargo build` - passes
- ✅ `cargo test` - 419 tests pass
- ✅ `cargo clippy` - no warnings
- ✅ `cargo fmt --check` - properly formatted
- ✅ Web UI template verified - shows "Panko" in title and header

---

## Notes

### Story Dependencies

The milestone has two parallel tracks that can be worked independently:

**Track A: Context & Download (Stories 1-4)**
- Branding fix, tool rendering, context copy, download
- No internal dependencies

**Track B: Sub-Agent Visualization (Stories 5-6)**
- Parser changes then web UI visualization
- Story 6 depends on Story 5

**Track C: Multi-Share (Stories 7-10)**
- State refactor → modal → panel → concurrent management
- Linear dependency chain

**Final: Story 11**
- PR depends on all other stories complete

### Branding Changes (Story 1)

Files to update:
- `templates/session.html` - title and header
- `src/tui/app.rs` - TUI titles
- `src/assets/styles.css` - comment header
- `src/assets/keyboard.js` - comment header
- `docs/agents/panko-m1/PROMPT.md` - header
- `docs/agents/panko-m2/PROMPT.md` - header and ASCII diagram

### Multi-Share Architecture (Stories 7-10)

Key structures to introduce:
```rust
pub struct ShareId(uuid::Uuid);

pub struct ActiveShare {
    pub id: ShareId,
    pub session_path: PathBuf,
    pub public_url: String,
    pub provider_name: String,
    pub started_at: DateTime<Utc>,
}

// App state changes
pub active_shares: Vec<ActiveShare>,
pub sharing_handles: HashMap<ShareId, SharingHandle>,
```

### Context Export Format (Story 3)

Markdown format for clipboard:
```markdown
# Session: {session_id}
**Project**: {project_path}
**Date**: {date}

## Conversation

### User
{prompt}

### Assistant
{response}

### Tool: {tool_name}
{summary or truncated result}
```

### Sub-Agent Block (Story 5)

New block variant:
```rust
Block::SubAgentSpawn {
    agent_id: String,
    agent_type: String,  // "Explore", "Plan", "Bash", etc.
    prompt: String,
    status: SubAgentStatus,  // Pending, Running, Completed
    result: Option<String>,
}
```
