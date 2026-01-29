# Agent Replay - Milestone 2 Progress

## Overview

**Target Version**: v0.3.0 (TUI browser)
**Prerequisites**: Milestone 1 complete (v0.1.0 - v0.2.0)
**Status**: Not Started

## Story Progress

| ID | Title | Status | Notes |
|----|-------|--------|-------|
| 1 | Session scanner trait and Claude implementation | ⬜ Not Started | |
| 2 | TUI application scaffold with ratatui | ⬜ Not Started | |
| 3 | Session list widget with project grouping | ⬜ Not Started | |
| 4 | Preview panel | ⬜ Not Started | |
| 5 | Layout with resizable panels | ⬜ Not Started | |
| 6 | View action integration | ⬜ Not Started | |
| 7 | Share action integration | ⬜ Not Started | |
| 8 | Copy path and open folder actions | ⬜ Not Started | |
| 9 | Fuzzy search | ⬜ Not Started | |
| 10 | Help overlay | ⬜ Not Started | |
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

### [Date]

_No work logged yet._

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
