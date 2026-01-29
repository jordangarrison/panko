# Agent Replay - TUI Session Browser

Build a terminal UI for browsing, previewing, and sharing AI coding agent sessions. This extends the existing `agent-replay` CLI with an interactive browser mode.

## Prerequisites

This milestone assumes Milestone 1 is complete:
- Parser plugin architecture with Claude Code support
- Local web server for viewing sessions
- Tunnel providers (Cloudflare, ngrok, Tailscale)
- Share command with clipboard integration

## Project Context

Users have many sessions across multiple projects in `~/.claude/projects/`. The current CLI requires knowing the exact file path. The TUI provides:
1. Browse all sessions across all projects
2. Preview session summaries before opening
3. Quick actions: view, share, copy path
4. Fuzzy search to find specific sessions

## Technical Stack (additions to M1)

- **TUI Framework**: ratatui
- **Async Runtime**: tokio (already present)
- **Fuzzy Matching**: fuzzy-matcher
- **File Watching**: notify (optional, for live updates)

## Architecture Additions

```
src/
├── tui/
│   ├── mod.rs           # TUI app entry point
│   ├── app.rs           # Application state
│   ├── ui.rs            # Layout and rendering
│   ├── widgets/
│   │   ├── mod.rs
│   │   ├── session_list.rs    # Project/session tree
│   │   ├── preview.rs         # Session preview panel
│   │   └── help.rs            # Keyboard shortcut overlay
│   ├── events.rs        # Input event handling
│   └── actions.rs       # Action dispatch (view, share, etc.)
├── scanner/
│   ├── mod.rs           # Session discovery
│   └── claude.rs        # Scan ~/.claude/projects/
```

## TUI Layout

```
┌─ Agent Replay ─────────────────────────────────────────────────┐
│ Search: _                                            [?] Help  │
├────────────────────────────────┬───────────────────────────────┤
│ Sessions                       │ Preview                       │
│ ─────────────────────────────  │ ───────────────────────────── │
│ ⏷ ~/projects/api-server/       │ Session: abc123               │
│   ├─ abc123  2h ago   12 msgs  │ Started: 2025-01-28 10:30     │
│   └─ def456  1d ago   45 msgs  │ Messages: 12                  │
│ ⏵ ~/projects/frontend/         │                               │
│ ⏷ ~/projects/infra/            │ First prompt:                 │
│   └─ ghi789  3d ago   8 msgs   │ "Refactor the auth module     │
│                                │ to use JWT tokens instead     │
│                                │ of sessions..."               │
│                                │                               │
│                                │ Tools used:                   │
│                                │ • Edit (15x)                  │
│                                │ • Bash (8x)                   │
│                                │ • Read (23x)                  │
├────────────────────────────────┴───────────────────────────────┤
│ [v]iew  [s]hare  [c]opy path  [o]pen folder  [r]efresh  [q]uit │
└────────────────────────────────────────────────────────────────┘
```

## Session Scanner

Discover sessions without parsing full content:

```rust
pub struct SessionMeta {
    pub id: String,
    pub path: PathBuf,
    pub project_path: String,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub first_prompt_preview: Option<String>,  // First ~100 chars
}

pub trait SessionScanner: Send + Sync {
    fn scan_directory(&self, root: &Path) -> Result<Vec<SessionMeta>>;
    fn default_roots(&self) -> Vec<PathBuf>;
}
```

## Key Interactions

### Navigation
- `j/k` or `↑/↓`: Move selection
- `h/l` or `←/→`: Collapse/expand project folders
- `Tab`: Switch focus between panels
- `/`: Focus search input
- `Esc`: Clear search / close overlays

### Actions
- `v` or `Enter`: View selected session (launches web viewer)
- `s`: Share selected session (tunnel + clipboard)
- `c`: Copy session file path to clipboard
- `o`: Open containing folder in system file manager
- `r`: Refresh session list
- `?`: Toggle help overlay
- `q`: Quit

### Search
- Fuzzy matches against project path and first prompt
- Results update as you type
- `Enter` to select first match
- `Esc` to clear and show all

## Stories Reference

See prd.json for detailed stories. This milestone focuses on the TUI layer; it reuses parser/server/tunnel code from M1.

## Completion

When all stories pass their acceptance criteria, output:
<promise>COMPLETE</promise>
