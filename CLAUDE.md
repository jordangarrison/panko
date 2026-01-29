# Agent Replay Development

## Project Overview
Rust CLI tool for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.)

## Development Environment
- NixOS with flake + direnv
- Rust toolchain via nix devshell
- Run `direnv allow` after cloning

## Coding Standards
- Use idiomatic Rust (clippy clean, rustfmt formatted)
- Error handling with thiserror for library, anyhow for binary
- Prefer explicit types over inference in public APIs
- Tests alongside code in same file or tests/ directory

## Ralph Wiggum Loop Protocol

When working on this project:

1. **Read the PRD**: Load the current milestone's prd.json
2. **Check/Create Branch**: If not on the `branchName` from prd.json, create and checkout that branch
3. **Find Next Story**: Identify the first story where `passes: false` and all `depends_on` stories have `passes: true`
4. **Create Tasks**: Use TaskCreate for each acceptance criterion of the story
5. **Use Sub-Agents**: Spawn Explore/Plan/general-purpose agents for complex work
6. **Work Story**: Complete all acceptance criteria for that single story
7. **Validate** (REQUIRED before marking complete):
   - [ ] Code compiles: `cargo build` passes (skip if pre-Cargo.toml)
   - [ ] Tests written: Unit tests for new functionality
   - [ ] Tests pass: `cargo test` passes
   - [ ] Lints pass: `cargo clippy` has no warnings
   - [ ] Format: `cargo fmt --check` passes
   - [ ] Integration test: Manual or automated verification the feature works end-to-end
8. **Mark Complete**: Update prd.json setting `passes: true` for completed story
9. **Update Progress**: Log work in PROGRESS.md with date, summary, and validation results
10. **Commit**: Create a commit for the completed story
11. **Stop**: Do NOT proceed to next story - let user trigger next iteration

### Validation Checklist

Before ANY story can be marked `passes: true`:

```
cargo build          # Must succeed
cargo test           # All tests must pass
cargo clippy         # No warnings
cargo fmt --check    # Properly formatted
```

For stories with user-facing features, also run:
```
cargo run -- <relevant command>   # Verify it works
```

Write integration tests in `tests/` directory for complex behaviors.

### Current Milestones
- M1: `docs/agents/agent-replay-m1/` - Core CLI (parser, server, tunnels)
- M2: `docs/agents/agent-replay-m2/` - TUI Browser (depends on M1 complete)

### Sub-Agent Usage
- **Explore agents**: Codebase understanding, finding patterns
- **Plan agents**: Complex implementation design
- **general-purpose agents**: Implementation work, research

## Commit Guidelines
- No co-author attribution (no "Co-Authored-By" lines)
- No "Generated with Claude Code" in messages
- Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- One commit per completed story when reasonable
