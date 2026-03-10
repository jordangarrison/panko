# Panko Development

## Project Overview
Elixir/Phoenix web application for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.), built on Ash Framework 3.x.

## Development Environment
- NixOS with flake + direnv
- Elixir/Erlang toolchain via nix devshell
- PostgreSQL for data persistence
- Run `direnv allow` after cloning

## Ash Framework Guidelines

This project uses Ash 3.x as its core data layer. See `.rules` for comprehensive
Ash development patterns and pitfalls.

Key principles:
- **Resources** use `Ash.Resource` with `domain:` and `data_layer:` options
- **Domains** (`Ash.Domain`) define the public API via code interfaces — always call
  domain functions from LiveViews and controllers, never raw `Ash.*` calls
- **Prefer keyword filter syntax**: `Ash.Query.filter(field: value)` over expression
  syntax with pin operators
- **`constraints max_length: nil` is NOT valid** in Ash 3.x — omit constraints for
  unlimited strings
- **PubSub notifiers** on resources for real-time updates
- **`manage_relationship` with `type: :direct_control`** for managing related records
  in a single action

### Common Ash Operations
```elixir
# Creating via changeset
Resource |> Ash.Changeset.for_create(:action, params) |> Ash.create()

# Reading
Ash.read!(Resource)
Ash.get!(Resource, id)

# Loading relationships
Ash.load!(record, [:blocks, :sub_agents])

# Domain code interface (preferred)
Panko.Sessions.list_sessions()
Panko.Sessions.get_session(id)
Panko.Sessions.import_from_file(file_path)
```

### Migrations
```bash
mix ash.codegen <description>   # Generate migration
mix ash.migrate                 # Run migrations (dev)
MIX_ENV=test mix ash.migrate   # Run migrations (test)
```

## Coding Standards
- Use idiomatic Elixir (credo-clean, mix format)
- Pattern match liberally; use `with` for multi-step operations
- Resources in domain-specific namespaces (e.g., `Panko.Sessions.Session`)
- LiveView components for UI, domain functions for data access
- Tests in `test/` directory mirroring `lib/` structure

## Development Commands
```bash
mix setup                        # Full project setup (deps, DB, assets)
mix phx.server                   # Start dev server
mix test                         # Run tests
mix compile --warnings-as-errors # Compile with strict warnings
mix format                       # Format code
mix precommit                    # Full precommit check (compile, format, test)
mix ash.codegen <name>           # Generate Ash migration
mix ash.migrate                  # Run Ash migrations
```

## Project Structure
```
lib/
  panko/
    sessions/             # Sessions domain resources
      session.ex          # Session resource (Ash.Resource)
      block.ex            # Block resource (session content blocks)
      sub_agent.ex        # SubAgent resource
      parsers/            # Parser behaviour and implementations
      session_watcher.ex  # GenServer for file system watching
    sessions.ex           # Sessions domain (Ash.Domain)
    sharing/              # Sharing domain resources
      share.ex            # Share resource
      changes/            # Custom Ash changes
      workers/            # Oban workers
    sharing.ex            # Sharing domain (Ash.Domain)
    repo.ex               # Ecto/AshPostgres repo
  panko_web/
    live/                 # LiveView pages
    components/           # Phoenix components
    router.ex             # Routes
    endpoint.ex           # Phoenix endpoint
```

## Ralph Wiggum Loop Protocol

When working on this project:

1. **Read the PRD**: Load the current milestone's prd.json
2. **Check/Create Branch**: If not on the `branchName` from prd.json, create and checkout that branch
3. **Find Next Story**: Identify the first story where `passes: false` and all `depends_on` stories have `passes: true`
4. **Create Tasks**: Use TaskCreate for each acceptance criterion of the story
5. **Use Sub-Agents**: Spawn Explore/Plan/general-purpose agents for complex work
6. **Work Story**: Complete all acceptance criteria for that single story
7. **Validate** (REQUIRED before marking complete):
   - [ ] Code compiles: `mix compile --warnings-as-errors` passes
   - [ ] Tests written: Unit tests for new functionality
   - [ ] Tests pass: `mix test` passes
   - [ ] Format: `mix format --check-formatted` passes
   - [ ] Integration test: Manual or automated verification the feature works end-to-end
8. **Mark Complete**: Update prd.json setting `passes: true` for completed story
9. **Update Progress**: Log work in PROGRESS.md with date, summary, and validation results
10. **Commit**: Create a commit for the completed story
11. **Stop**: Do NOT proceed to next story - let user trigger next iteration

### Validation Checklist

Before ANY story can be marked `passes: true`:

```
mix compile --warnings-as-errors   # Must succeed
mix test                           # All tests must pass
mix format --check-formatted       # Properly formatted
```

For stories with user-facing features, also run:
```
mix phx.server   # Start server and verify manually
```

Write tests in `test/` directory mirroring `lib/` structure.

### Current Milestones
- M1: `docs/agents/panko-m1/` - Core CLI (parser, server, tunnels) — Rust, completed
- M2: `docs/agents/panko-m2/` - TUI Browser — Rust, completed
- M3: `docs/agents/panko-m3/` - Elixir/Ash rewrite scaffold
- M4: `docs/agents/panko-m4/` - Session viewing and sharing
- M5: `docs/agents/panko-m5/` - Polish, deployment, documentation

### Sub-Agent Usage
- **Explore agents**: Codebase understanding, finding patterns
- **Plan agents**: Complex implementation design
- **general-purpose agents**: Implementation work, research

## Commit Guidelines
- No co-author attribution (no "Co-Authored-By" lines)
- No "Generated with Claude Code" in messages
- Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- One commit per completed story when reasonable

## Nix Development Shell

The flake exports a devshell with Elixir, Erlang, and development tools.

### Key files
- `flake.nix` — inputs (nixpkgs, flake-parts)
- `nix/devshell.nix` — development shell (Elixir, Erlang, PostgreSQL tools)

### Dev commands
```bash
nix develop          # Enter devshell
direnv allow         # Auto-activate with direnv
```
