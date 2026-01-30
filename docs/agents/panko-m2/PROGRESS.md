# Agent Replay - Milestone 2 Progress

## Overview

**Target Version**: v0.3.0 (TUI browser)
**Prerequisites**: Milestone 1 complete (v0.1.0 - v0.2.0)
**Status**: In Progress

## Story Progress

| ID | Title | Status | Notes |
|----|-------|--------|-------|
| 1 | Session scanner trait and Claude implementation | ✅ Complete | |
| 2 | TUI application scaffold with ratatui | ✅ Complete | |
| 3 | Session list widget with project grouping | ✅ Complete | |
| 4 | Preview panel | ✅ Complete | Added 2025-01-30 |
| 5 | Layout with resizable panels | ✅ Complete | |
| 6 | View action integration | ✅ Complete | |
| 7 | Share action integration | ✅ Complete | |
| 8 | Copy path and open folder actions | ✅ Complete | |
| 9 | Fuzzy search | ✅ Complete | |
| 10 | Help overlay | ✅ Complete | Added 2026-01-30 |
| 11 | Refresh and auto-refresh | ⬜ Not Started | |
| 12 | Sorting options | ⬜ Not Started | |
| 13 | Multiple agent support in scanner | ⬜ Not Started | |
| 14 | Session deletion with confirmation | ⬜ Not Started | |

## Legend

- ⬜ Not Started
- 🟡 In Progress
- ✅ Complete
- ❌ Blocked

## Work Log

### 2026-01-30

**Story 10: Help overlay** - ✅ Complete

Implemented the help overlay widget that displays all keyboard shortcuts grouped by category.

**Changes:**
- Created `src/tui/widgets/help.rs` with `HelpOverlay` widget
- Shortcuts grouped into 4 categories: Navigation, Search, Actions, General
- `?` key toggles the overlay
- Any key or Esc closes the overlay
- Help hint `[?] Help` already shown in header
- Semi-transparent effect via Clear widget (clears area behind popup)

**Validation Results:**
- `cargo build` ✅
- `cargo test` ✅ (327 tests passed)
- `cargo clippy` ✅ (no warnings)
- `cargo fmt --check` ✅

---

### 2025-01-30

**Story 4: Preview panel** - ✅ Complete

Implemented the preview panel widget for displaying session details in the TUI.

**Changes:**
- Created `src/tui/widgets/preview.rs` with `PreviewPanel` widget
- Added `tool_usage: Option<HashMap<String, usize>>` field to `SessionMeta`
- Updated Claude scanner to extract tool usage from assistant messages
- Updated `app.rs` to display two-column layout (session list + preview)
- Preview panel shows: session ID, updated timestamp, full path, message count, tool usage summary, and first prompt preview

**Validation Results:**
- `cargo build` ✅
- `cargo test` ✅ (228 tests passed)
- `cargo clippy` ✅ (no warnings)
- `cargo fmt --check` ✅

---

## Notes

### Ratatui Patterns

Key patterns to follow:
- Stateful widgets for lists (maintain selection state)
- Immediate mode rendering (rebuild UI each frame)
- Separate input handling from rendering
- Use `Frame::render_stateful_widget` for stateful components

### Terminal Suspension

When launching web viewer:
1. Disable raw mode
2. Show cursor
3. Clear alternate screen
4. Run external process
5. Wait for completion
6. Re-enter TUI mode

Use `crossterm::terminal::disable_raw_mode()` and `enable_raw_mode()`.

### Session Scanning Performance

`~/.claude/projects/` can have many sessions. Strategies:
- Scan in background thread
- Show loading indicator
- Cache results, invalidate on refresh
- Only parse first few lines for preview

### Integration with M1

M1 code should be refactored to expose:
- `view_session(path: &Path, open_browser: bool) -> Result<ServerHandle>`
- `share_session(path: &Path, provider: TunnelProvider) -> Result<ShareHandle>`

TUI calls these and manages the handles.

### Future Considerations (v0.4.0+)

- Session tagging/favoriting
- Export multiple sessions
- Diff between sessions
- Session notes/annotations
- Cloud sync of session metadata
