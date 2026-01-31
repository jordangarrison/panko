# Milestone 4: Sharing UX Polish

## Problem Statement

User report:
1. After sharing and copying URL, app doesn't return to normal key handling
2. "Copied url to clipboard" message persists at bottom
3. Pressing Escape kills ALL shares
4. Controls feel non-deterministic when sharing is active

**Desired UX**: Normal controls work while sharing. Share management (stopping individual shares) only happens in the Shares Panel (Shift+S).

## Root Cause Analysis

The codebase has **two parallel sharing systems**:

1. **Legacy single-share system**: `SharingState` enum (Inactive/SelectingProvider/Starting/Active/Stopping)
2. **Modern multi-share system**: `ShareManager` with `active_shares: Vec<ActiveShare>` and per-share controls

**The Problem** (in `handle_key_event()` around line 350):
```rust
// Current routing order (BROKEN):
if self.sharing_state.is_active() {
    return self.handle_sharing_key(key_event);  // LOCKS OUT EVERYTHING
}
// ... shares panel check comes AFTER - UNREACHABLE when sharing!
```

When `sharing_state.is_active() == true`:
- `handle_sharing_key()` intercepts ALL keys
- Only allows: Esc (kills all), q, j/k navigation, Tab, Shift+S, s
- Blocks: view (Enter), copy path (c), open folder (o), delete (d), refresh (r), help (?), search (/)
- The shares panel with individual share controls is UNREACHABLE

## Analysis Documents

See `analysis/` directory for detailed exploration:
- `key_routing_diagram.txt` - Visual flow of key event routing
- `implementation_summary.txt` - Complete architecture analysis
- `code_examples.txt` - Relevant code snippets with line numbers

## Implementation Plan

### Step 1: Remove Restrictive Key Handler Routing

**File:** `src/tui/app.rs` (~line 350)

Remove the `sharing_state.is_active()` check that routes to `handle_sharing_key()`:

```rust
// DELETE THIS BLOCK:
if self.sharing_state.is_active() {
    return self.handle_sharing_key(key_event);
}
```

### Step 2: Block Deleting Shared Sessions in Normal Handler

**File:** `src/tui/app.rs` (~line 460, delete handler)

Add check before allowing delete:

```rust
KeyCode::Char('d') => {
    if let Some(session) = self.selected_session() {
        // Don't allow deleting sessions that are being shared
        if self.share_manager.is_session_shared(&session.path) {
            self.set_status_message("Cannot delete: session is being shared");
        } else {
            self.pending_action = Action::RequestDelete(session.path.clone());
        }
    }
}
```

### Step 3: Add `is_session_shared()` Helper

**File:** `src/tui/sharing.rs` - Add to `ShareManager`:

```rust
impl ShareManager {
    pub fn is_session_shared(&self, path: &Path) -> bool {
        self.active_shares.iter().any(|s| s.session_path == path)
    }
}
```

### Step 4: Remove or Simplify `handle_sharing_key()`

Either delete the function entirely (since it's no longer called) or keep a minimal version for edge cases.

## Files to Modify

| File | Action |
|------|--------|
| `src/tui/app.rs` | Remove sharing key routing, update delete handler |
| `src/tui/sharing.rs` | Add `is_session_shared()` helper |

## Verification

```bash
cargo build && cargo test && cargo clippy && cargo fmt --check
```

### Manual Test

```bash
RUST_LOG=panko=debug cargo run
```

1. Start a share (press `s`)
2. Dismiss URL modal
3. Verify ALL normal controls work:
   - Navigate sessions (j/k)
   - View session (Enter)
   - Copy path (c)
   - Open folder (o)
   - Refresh (r)
   - Help (?)
   - Search (/)
4. Try to delete shared session → should be blocked with message
5. Try to delete non-shared session → should work
6. Press Esc → should NOT kill shares
7. Open shares panel (Shift+S)
8. Stop individual share from panel (d on selected share)
9. Verify session can now be deleted

## Expected Behavior After Fix

| Action | Before (Broken) | After (Fixed) |
|--------|-----------------|---------------|
| Esc when sharing | Kills ALL shares | Does nothing (or closes modal) |
| Normal navigation | Blocked | Works normally |
| View session (Enter) | Blocked | Works normally |
| Delete shared session | Blocked (silently) | Blocked with message |
| Delete other session | Blocked | Works normally |
| Stop individual share | Unreachable | Via Shares Panel only |
