# Milestone 5: Daemon-Based Sharing Architecture

## Problem Statement

Current sharing architecture:
- Shares are managed in-memory in the TUI process
- Background threads spawn from TUI, communicate via channels
- **All shares die when TUI exits**
- No way to reconnect to existing shares

**Desired UX**: Shares persist after TUI exits. User can close panko, reopen it, and see their active shares still running.

## Solution: Daemon Architecture

Separate sharing into a daemon process (`panko serve`) that:
- Runs independently of the TUI
- Manages all shares lifecycle
- Persists state to SQLite
- Communicates with TUI via Unix socket IPC

```
┌─────────────────────────────────────────────────────────────────┐
│                          TUI Process                             │
│  ┌──────────────────┐     ┌──────────────────────────────────┐  │
│  │   App State      │     │   DaemonClient                   │  │
│  │   (UI only)      │────▶│   - Unix socket connection       │  │
│  └──────────────────┘     └──────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                        │ Unix Socket IPC
                                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Daemon Process                             │
│                    `panko serve`                                 │
│  ┌──────────────────┐     ┌──────────────────────────────────┐  │
│  │   ShareService   │────▶│   SQLite State                   │  │
│  │   - Manage shares│     │   ~/.local/share/panko/state.db  │  │
│  └──────────────────┘     └──────────────────────────────────┘  │
│           │                                                      │
│           ├───────────────────┬───────────────────┐             │
│           ▼                   ▼                   ▼             │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │
│  │   Share 1   │     │   Share 2   │     │   Share N   │       │
│  │  Server+    │     │  Server+    │     │  Server+    │       │
│  │  Tunnel     │     │  Tunnel     │     │  Tunnel     │       │
│  └─────────────┘     └─────────────┘     └─────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

## Files to Create

```
src/
├── daemon/
│   ├── mod.rs              # Module exports
│   ├── protocol.rs         # IPC message types (DaemonRequest/Response)
│   ├── server.rs           # Daemon main loop, IPC handling
│   ├── client.rs           # DaemonClient for TUI
│   ├── share_service.rs    # Share lifecycle management
│   └── db.rs               # SQLite operations

docs/agents/panko-m5/
├── prd.json                # Stories
├── PROMPT.md               # This document
└── PROGRESS.md             # Work log
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod daemon;` |
| `src/main.rs` | Add `serve`, `serve-stop`, `serve-status` commands |
| `src/tui/app.rs` | Replace thread spawning with DaemonClient calls |
| `src/tui/sharing.rs` | Simplify to thin wrapper over DaemonClient |
| `Cargo.toml` | Add `rusqlite`, `uuid` dependencies |

## Dependencies to Add

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

## SQLite Schema

```sql
CREATE TABLE IF NOT EXISTS shares (
    id TEXT PRIMARY KEY,              -- UUID
    session_path TEXT NOT NULL,
    session_name TEXT NOT NULL,
    public_url TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    local_port INTEGER NOT NULL,
    started_at TEXT NOT NULL,         -- ISO8601
    status TEXT NOT NULL DEFAULT 'active'
);

CREATE TABLE IF NOT EXISTS daemon_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

## IPC Protocol (JSON over Unix Socket)

Request format:
```json
{"method": "StartShare", "params": {"session_path": "/path/to/session.jsonl", "provider": "cloudflare"}}
```

Response format:
```json
{"status": "ShareStarted", "data": {"share_id": "uuid", "public_url": "https://...", "local_port": 8080}}
```

## Auto-start Daemon

When TUI starts:
1. Try to connect to socket
2. If fails, spawn `panko serve` as detached process
3. Wait up to 5s for socket to appear
4. Connect and continue

## Error Handling

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Daemon not running | Socket connect fails | Auto-start or show error |
| Daemon crashes | IPC error mid-operation | Show error, allow manual restart |
| TUI crashes | N/A | Shares persist in daemon |
| Tunnel dies | Daemon health check | Mark share as error |

## Verification

### Per-Story Validation
```bash
cargo build          # Must succeed
cargo test           # All tests pass
cargo clippy         # No warnings
cargo fmt --check    # Properly formatted
```

### End-to-End Test (after Story 8)
```bash
# Terminal 1: Start daemon
panko serve --foreground

# Terminal 2: Start TUI, create share
panko
# Press 's' on a session, select provider
# Note the share URL
# Press 'q' to quit TUI

# Terminal 2: Restart TUI
panko
# Press Shift+S to see shares panel
# Verify share is still there and URL works

# Terminal 2: Stop share from TUI
# Press 'd' on share in panel

# Terminal 1: Stop daemon
# Ctrl+C or `panko serve-stop`
```
