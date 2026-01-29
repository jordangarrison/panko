# Agent Replay - Milestone 1 Progress

## Overview

**Target Version**: v0.1.0 (core functionality) → v0.2.0 (collapsible blocks)
**Status**: In Progress

## Story Progress

| ID | Title | Status | Notes |
|----|-------|--------|-------|
| 1 | Project scaffolding and CLI structure | ✅ Complete | |
| 2 | Parser trait and unified types | ⬜ Not Started | |
| 3 | Claude Code JSONL parser | ⬜ Not Started | |
| 4 | Embedded web assets and templates | ⬜ Not Started | |
| 5 | Local web server with view command | ⬜ Not Started | |
| 6 | Tunnel provider trait and detection | ⬜ Not Started | |
| 7 | Cloudflare quick tunnel implementation | ⬜ Not Started | |
| 8 | Share command with tunnel and clipboard | ⬜ Not Started | |
| 9 | ngrok tunnel implementation | ⬜ Not Started | |
| 10 | Tailscale serve implementation | ⬜ Not Started | |
| 11 | Configuration file support | ⬜ Not Started | |
| 12 | Keyboard navigation in viewer | ⬜ Not Started | |

## Legend

- ⬜ Not Started
- 🟡 In Progress
- ✅ Complete
- ❌ Blocked

## Work Log

### 2026-01-29

**Story 1: Project scaffolding and CLI structure** - COMPLETE

- Created Cargo.toml with all required dependencies (clap, axum, tokio, serde, minijinja, rust-embed, inquire, arboard, thiserror, anyhow, chrono, webbrowser, pulldown-cmark)
- Implemented basic CLI with clap derive macros
- Added `view` and `share` subcommands, each accepting a file path argument
- Verified `--help` and `--version` work correctly
- Set up source directory structure: src/parser/, src/server/, src/tunnel/

**Validation Results:**
- `cargo build` - PASS
- `cargo test` - PASS (0 tests, no failures)
- `cargo clippy` - PASS (no warnings)
- `cargo fmt --check` - PASS

---

## Notes

### Claude Code JSONL Format

Location: `~/.claude/projects/<project-path>/`

Files are JSONL with message objects containing:
- `type`: "human" or "assistant"
- `content`: array of content blocks
- `timestamp`: ISO datetime
- Tool calls in assistant messages have `type: "tool_use"`
- Tool results appear as separate messages with `type: "tool_result"`

### Tunnel Provider Priority

Default detection order:
1. Cloudflare (no auth required for quick tunnels)
2. Tailscale (if logged in)
3. ngrok (may require auth for longer sessions)

### Future Considerations (v0.2.0+)

- Collapsible thinking blocks
- Collapsible tool call details
- Syntax highlighting for code in responses
- Search within session
- Export to self-contained HTML
