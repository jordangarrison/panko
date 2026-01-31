# Panko - Multi-Share & Context Export

Build enhanced sharing capabilities, context export features, and UI improvements for the Panko session viewer. This milestone adds support for multiple concurrent shares, sub-agent visualization, and session export functionality.

## Prerequisites

This milestone assumes Milestones 1 and 2 are complete:
- Parser plugin architecture with Claude Code support
- Local web server for viewing sessions
- Tunnel providers (Cloudflare, ngrok, Tailscale)
- Share command with clipboard integration
- TUI session browser with search and preview
- Session scanner with multi-agent support

## Project Context

Users want to:
1. Share multiple sessions simultaneously without stopping previous shares
2. Export session context for reuse in new Claude Code sessions
3. Download raw session files from the web viewer
4. Visualize sub-agent spawns and their outputs
5. Have consistent "Panko" branding throughout the application

## Technical Stack (additions to M1/M2)

No new dependencies required. This milestone builds on existing:
- **Clipboard**: arboard (for context copy)
- **TUI**: ratatui (for new widgets)
- **Web**: axum + minijinja (for download endpoint)

## Architecture Additions

```
src/
├── parser/
│   └── types.rs         # Add SubAgentMeta, Block::SubAgentSpawn
├── server/
│   └── routes.rs        # Add download endpoint
├── tui/
│   ├── app.rs           # ActiveShare, multi-share state
│   └── widgets/
│       ├── share_modal.rs    # Share started popup
│       └── shares_panel.rs   # Active shares list
└── export/
    └── context.rs       # Session context formatting
```

## Key Features

### Context Export (TUI)
- `Shift+C`: Copy session context as markdown
- Formats user prompts, assistant responses, key results
- Excludes verbose tool outputs
- Shows token estimate

### Session Download
- Download button in web UI header
- `Shift+D` in TUI saves to ~/Downloads
- Works for both local and shared sessions

### Sub-Agent Visualization
- Parse Task tool calls for sub-agent spawning
- Track agent type (Explore, Plan, Bash, etc.)
- Nested/indented display in web UI
- Expandable details with connector lines

### Multiple Concurrent Shares
- Start new shares without stopping existing ones
- `Shift+S` opens shares panel showing all active
- Each share tracked with unique ID
- Configurable max limit (default: 5)

### Share Modal
- Modal popup with public URL when share starts
- Auto-dismisses after 5 seconds
- `c` copies URL to clipboard
- Shows session name and provider

## Stories Reference

See prd.json for detailed stories and acceptance criteria. Work through stories in priority order, marking `passes: true` when complete.

## Completion

When all stories pass their acceptance criteria, output:
<promise>COMPLETE</promise>
