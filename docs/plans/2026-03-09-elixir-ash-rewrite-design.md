# Panko Elixir/Ash Rewrite Design

**Date:** 2026-03-09
**Status:** Approved

## Overview

Rewrite Panko from a Rust CLI tool into an Elixir/Ash/Phoenix LiveView web application. The goal is to lower the adoption barrier from "install a Rust CLI + cloudflared" to "open a URL," while preserving the core experience of browsing and sharing AI coding agent sessions.

## Architecture

### Approach: Hybrid Local + Hosted

- **Local:** A background GenServer watches `~/.claude/projects/` for new/modified JSONL sessions and imports them into PostgreSQL. The LiveView web UI shows all discovered sessions in real-time.
- **Shared:** Users explicitly share specific sessions, generating a slug-based public URL with optional expiry. Unpublish is always available.
- **Federation-ready:** Schema uses UUID primary keys, origin tracking (`origin_id`), and clean domain boundaries to support future federation without implementing it now.

### App Structure: Flat (not umbrella)

Single Phoenix app with three Ash domains for logical separation:

```
lib/panko/
  sessions/          # Core domain
  sharing/           # Share lifecycle
  accounts/          # Stub for future auth
lib/panko_web/
  live/              # LiveView pages
  components/        # Shared UI components
```

## Data Model

### Domain: Panko.Sessions

#### Session

| Field | Type | Notes |
|-------|------|-------|
| id | :uuid | Primary key |
| external_id | :string | Original session_id from JSONL |
| source_type | SourceType enum | :claude_code, :codex, etc. |
| source_path | :string | Original file path (nullable) |
| project | :string | Project name/path (nullable) |
| title | :string | Derived from first prompt (nullable) |
| started_at | :utc_datetime | |
| user_id | :uuid | Nullable, for future auth |
| origin_id | :string | Instance identifier, for future federation (nullable) |
| inserted_at / updated_at | timestamps | |

Relationships:
- `has_many :blocks` (sorted by `position: :asc`)
- `has_many :sub_agents`

Identities:
- `[:external_id, :source_type]` for upsert

Aggregates:
- `block_count` — count of blocks
- `tool_call_count` — count of blocks where `block_type == :tool_call`
- `file_edit_count` — count of blocks where `block_type == :file_edit`
- `message_count` — count of blocks where `block_type in [:user_prompt, :assistant_response]`
- `last_activity_at` — first block timestamp descending

#### Block

| Field | Type | Notes |
|-------|------|-------|
| id | :uuid | Primary key |
| session_id | :uuid | FK to Session |
| position | :integer | Ordering within session |
| block_type | BlockType enum | :user_prompt, :assistant_response, :tool_call, :thinking, :file_edit, :sub_agent_spawn |
| content | :text | Message text / thinking content |
| metadata | :map (JSONB) | Tool-specific fields, validated by embedded resources per block_type |
| timestamp | :utc_datetime | |
| inserted_at | timestamp | |

Relationships:
- `belongs_to :session`

Indexes:
- Composite unique index on `[:session_id, :position]`

#### Block Metadata (Embedded Resources)

Each block type has a corresponding embedded Ash resource for structured JSONB validation:

- `Block.ToolCallMetadata` — `name`, `input` (map), `output` (map)
- `Block.FileEditMetadata` — `path`, `diff`
- `Block.SubAgentSpawnMetadata` — `agent_id`, `agent_type`, `description`

Validation change on Block checks metadata shape matches `block_type`.

#### SubAgent

| Field | Type | Notes |
|-------|------|-------|
| id | :uuid | Primary key |
| session_id | :uuid | FK to Session |
| external_id | :string | Tool call ID from JSONL |
| agent_type | :string | "Explore", "Plan", "Bash", etc. |
| description | :string | |
| prompt | :text | |
| status | SubAgentStatus enum | :running, :completed, :failed |
| result | :text | Nullable |
| spawned_at | :utc_datetime | |
| completed_at | :utc_datetime | Nullable |
| inserted_at | timestamp | |

Relationships:
- `belongs_to :session`

### Domain: Panko.Sharing

#### Share

| Field | Type | Notes |
|-------|------|-------|
| id | :uuid | Primary key |
| session_id | :uuid | FK to Session (cross-domain, requires `domain` option) |
| slug | :string | Unique, URL-friendly, auto-generated via Ash change |
| is_shared | :boolean | Default true, false = unpublished |
| expires_at | :utc_datetime | Nullable, null = never expires |
| shared_at | :utc_datetime | |
| unshared_at | :utc_datetime | Nullable |
| user_id | :uuid | Nullable |
| inserted_at / updated_at | timestamps | |

Cross-domain relationship:
```elixir
belongs_to :session, Panko.Sessions.Session do
  domain Panko.Sessions
  allow_nil? false
  public? true
end
```

### Domain: Panko.Accounts (Stub)

No resources yet. Nullable `user_id` columns on Session and Share provide the schema hook for future OAuth. Optional API key configured via `PANKO_API_KEY` env var, enforced by a Plug.

### Enum Types (Ash.Type.Enum)

- `Panko.Sessions.SourceType` — `:claude_code`, `:codex`
- `Panko.Sessions.Block.Type` — `:user_prompt`, `:assistant_response`, `:tool_call`, `:thinking`, `:file_edit`, `:sub_agent_spawn`
- `Panko.Sessions.SubAgentStatus` — `:running`, `:completed`, `:failed`

## Parser Architecture

### Behaviour

```elixir
defmodule Panko.Sessions.Parser do
  @callback source_type() :: atom()
  @callback can_parse?(path :: String.t()) :: boolean()
  @callback parse(path :: String.t()) :: {:ok, map()} | {:error, term()}
end
```

Parsers are pure functions (file in, data out). They live outside the Ash domain:

```
lib/panko/sessions/parsers/
  parser.ex           # Behaviour definition
  claude_code.ex      # Claude Code JSONL implementation
  registry.ex         # Finds the right parser for a file path
```

### Claude Code Parser

- Streams JSONL line by line (not loading full file into memory)
- Maps `entry_type` to block types
- Extracts tool call name/input/output into Block metadata
- Detects Write tool -> `:file_edit` block type with diff in metadata
- Detects Task tool -> `:sub_agent_spawn` block type + SubAgent record
- Derives `Session.title` from first user prompt (truncated)
- Handles polymorphic `tool_result.content` (string or array)

### Import Flow (Ash Actions, not standalone module)

Import is a **generic action** on the Session resource, keeping everything inside Ash's action pipeline:

```elixir
# Generic action — entry point
action :import_from_file, :struct do
  constraints instance_of: __MODULE__
  argument :file_path, :string, allow_nil?: false

  run fn input, _context ->
    path = input.arguments.file_path
    with {:ok, parser} <- Panko.Sessions.Parsers.Registry.find_parser(path),
         {:ok, attrs} <- parser.parse(path) do
      __MODULE__
      |> Ash.Changeset.for_create(:upsert_from_import, attrs)
      |> Ash.create()
    end
  end
end

# Create action — handles upsert + children
create :upsert_from_import do
  accept [:external_id, :source_type, :source_path, :project, :title, :started_at]
  upsert? true
  upsert_identity :external_id_source_type
  upsert_fields [:source_path, :project, :title, :started_at]

  argument :blocks, {:array, :map}, allow_nil?: false
  argument :sub_agents, {:array, :map}, allow_nil?: true

  change manage_relationship(:blocks, :blocks, type: :direct_control)
  change manage_relationship(:sub_agents, :sub_agents, type: :direct_control)
  change relate_actor(:user)  # nil-safe, for future auth
end
```

### Domain Code Interfaces

```elixir
defmodule Panko.Sessions do
  use Ash.Domain

  resources do
    resource Panko.Sessions.Session do
      define :import_from_file, action: :import_from_file, args: [:file_path]
      define :get_session, action: :read, get_by: [:id]
      define :list_sessions, action: :list_recent
    end
    resource Panko.Sessions.Block
    resource Panko.Sessions.SubAgent
  end
end
```

## Session Watcher

`Panko.Sessions.SessionWatcher` — GenServer using `FileSystem` library:

- Watches configured paths (default: `~/.claude/projects/`)
- Debounces rapid changes (Claude Code writes frequently during active sessions)
- Filters to `.jsonl` files only
- Spawns import as a Task to avoid blocking the watcher
- Calls `Panko.Sessions.import_from_file(path)` via domain code interface

## PubSub Notifiers

Session resource uses Ash PubSub notifier for real-time LiveView updates:

```elixir
pub_sub do
  module PankoWeb.Endpoint
  prefix "sessions"
  publish :upsert_from_import, ["imported"]
  publish_all :update, ["updated", :id]
end
```

LiveViews subscribe to these topics and update automatically when sessions are imported — no manual PubSub wiring needed.

## Share Lifecycle

### Actions on Panko.Sharing.Share:

- **`:create`** — accepts `session_id`, optional `expires_at`. Auto-generates slug via Ash change. Sets `shared_at` to now.
- **`:unpublish`** — sets `is_shared: false`, `unshared_at` to now.
- **`:republish`** — sets `is_shared: true`, clears `unshared_at`. Preserves original slug.

### Share Reaper (Oban)

Periodic Oban job that deactivates shares past their `expires_at`. Runs every hour.

## LiveView Pages & Routes

### Router

```elixir
scope "/", PankoWeb do
  pipe_through [:browser, :maybe_require_api_key]

  live_session :default, layout: {PankoWeb.Layouts, :app} do
    live "/", SessionsLive, :index
    live "/sessions/:id", SessionLive, :show
  end
end

# Public share routes — no API key protection
scope "/s", PankoWeb do
  pipe_through :browser

  live_session :public, layout: {PankoWeb.Layouts, :public} do
    live "/:slug", ShareLive, :show
  end
end
```

### SessionsLive (`/`) — Browse all sessions

- Lists sessions with aggregates (block count, tool call count, last activity, duration)
- Grouped by project (collapsible)
- Filtering: by project, date range, search across titles/first prompts
- Sorting: by date, message count, project name
- Share/unshare buttons per session with expiry picker
- Real-time updates via PubSub (new imports appear automatically)
- Pagination (start simple, can add infinite scroll later)

### SessionLive (`/sessions/:id`) — View a session

Full conversation thread with blocks rendered in order. Target features (built incrementally):

- Component per block type (user prompt, assistant response, tool call, thinking, file edit, sub-agent spawn)
- Syntax-highlighted code blocks
- Rendered markdown
- Expandable file diffs
- Collapsible tool call details with formatted JSON
- Sub-agent tree sidebar
- Jump-to-block navigation (sticky sidebar)
- Search within session
- Share button in header
- Download JSONL button

### ShareLive (`/s/:slug`) — Public shared view

- Same conversation thread as SessionLive
- Checks `is_shared == true` and `expires_at` — shows 404 if invalid
- No share/unshare controls, no session navigation
- Read-only public layout with "Powered by Panko" footer
- Reuses the same block components

### Components

```
lib/panko_web/components/
  blocks/
    user_prompt.ex
    assistant_response.ex
    tool_call.ex
    thinking.ex
    file_edit.ex
    sub_agent_spawn.ex
  session_card.ex        # Card for list view
  share_modal.ex         # Share URL + expiry picker
  block_nav.ex           # Jump-to-block sidebar
  search_bar.ex          # Filter/search input
```

A dispatcher component routes to the correct block renderer based on `block_type`.

## Styling

- **Tailwind CSS 4** + **daisyUI** for component primitives
- **Custom oklch theme** for Panko's identity (dark/light modes)
- Same pattern as RGD: daisyUI for structure, custom theme for distinctiveness

## Authentication

- **Now:** Optional API key via `PANKO_API_KEY` env var. A Plug checks `Authorization: Bearer <key>` or session cookie. Public share routes bypass this.
- **Later:** Full OAuth (GitHub). `user_id` columns already in schema. `change relate_actor(:user)` already in actions. Ash policies can be layered on without schema changes.

## Deployment & Development

### Development Setup

**Nix flake** with `nix/` folder:

```
nix/
  devshell.nix    # Elixir, Erlang 28, PostgreSQL 18, Node, Tailwind, inotify-tools
  package.nix     # mixRelease + fetchMixDeps + fetchNpmDeps
  module.nix      # NixOS module (systemd, nginx, secrets, optional local Postgres)
  docker.nix      # Docker image from nix-built release
```

**Devshell provides:**
- Erlang 28 + Elixir via `beam.packages.erlang_28`
- PostgreSQL 18
- Node.js (asset tooling)
- `tailwindcss_4`
- `inotify-tools` (Linux file watching)
- Shell hook: `mix local.hex --if-missing --force`, `mix local.rebar --if-missing --force`
- Sets `MIX_TAILWIND_PATH`, `ERL_AFLAGS` for shell history

### MCP Integration

**Tidewave** + **Ash AI** dev server for AI coding assistant integration:

```elixir
# mix.exs
{:tidewave, "~> 0.5", only: :dev}
{:ash_ai, "~> 0.2"}

# endpoint.ex — before code_reloading block
if Code.ensure_loaded?(Tidewave) do
  plug Tidewave
end

# endpoint.ex — inside code_reloading block
plug AshAi.Mcp.Dev,
  protocol_version_statement: "2024-11-05",
  otp_app: :panko
```

**`.mcp.json` generation:**
- `.mcp.json` is gitignored
- `.mcp.json.example` is committed for documentation
- A mix task or devshell hook reads the configured Phoenix port and writes `.mcp.json` with the correct Tidewave URL
- Handles concurrent dev sessions on different ports

**Ash usage rules:**
```bash
mix ash_ai.gen.usage_rules .rules \
  ash ash_postgres ash_phoenix ash_ai
```

### Configuration (runtime.exs)

```elixir
config :panko,
  session_watch_paths: System.get_env("PANKO_WATCH_PATHS", "~/.claude/projects"),
  api_key: System.get_env("PANKO_API_KEY"),
  default_share_expiry: System.get_env("PANKO_DEFAULT_EXPIRY", "7d"),
  instance_origin_id: System.get_env("PANKO_ORIGIN_ID", node_name_or_hostname())
```

### Docker Compose (primary self-hosting)

```yaml
services:
  panko:
    image: ghcr.io/jordangarrison/panko:latest
    ports:
      - "4000:4000"
    volumes:
      - ~/.claude/projects:/data/sessions:ro
    environment:
      - DATABASE_URL=ecto://postgres:postgres@db/panko
      - PANKO_WATCH_PATHS=/data/sessions
      - PANKO_API_KEY=optional-secret
      - SECRET_KEY_BASE=generate-me
    depends_on:
      - db
  db:
    image: postgres:18-alpine
    volumes:
      - pgdata:/var/lib/postgresql/data
    environment:
      - POSTGRES_DB=panko
      - POSTGRES_PASSWORD=postgres
volumes:
  pgdata:
```

### NixOS Module (nix/module.nix)

Full NixOS service module following Greenlight's pattern:

```nix
services.panko = {
  enable = true;
  host = "panko.example.com";
  port = 4000;
  listenAddress = "127.0.0.1";
  sessionWatchPaths = [ "/home/user/.claude/projects" ];
  defaultShareExpiry = "7d";
  apiKey = null;
  secretKeyBaseFile = "/run/secrets/panko-secret-key";
  databaseUrlFile = "/run/secrets/panko-database-url";
  database.createLocally = true;   # Auto-provision PostgreSQL 18
  database.name = "panko";
  nginx.enable = false;
  nginx.enableACME = true;
  openFirewall = false;
};
```

Features:
- systemd service with security hardening (PrivateTmp, NoNewPrivileges, etc.)
- LoadCredential for secrets (not in nix store)
- Optional local PostgreSQL provisioning via `ensureDatabases`/`ensureUsers`
- Optional nginx reverse proxy with ACME
- ReadOnlyPaths for session watch directories
- Assertions for common misconfigurations

### Nix Flake (flake.nix)

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      imports = [ ./nix/devshell.nix ./nix/package.nix ];
      flake.nixosModules.default = import ./nix/module.nix inputs.self;
    };
}
```

## Ash Patterns Checklist

- [ ] `public? true` on all externally-used attributes
- [ ] `Ash.Type.Enum` modules for all enums (not raw atoms)
- [ ] Domain `define` entries for all public actions
- [ ] Embedded resources for Block metadata validation
- [ ] `manage_relationship type: :direct_control` for Block/SubAgent import
- [ ] `change relate_actor(:user)` on create actions (nil-safe)
- [ ] PubSub notifiers on Session for LiveView reactivity
- [ ] Aggregates for counts/timestamps instead of stored attributes
- [ ] Composite unique index on `Block [:session_id, :position]`
- [ ] Cross-domain `domain` option on Share's `belongs_to :session`
- [ ] Share slug generated via Ash change, not external logic
- [ ] Custom `read` action preparations for common query patterns
