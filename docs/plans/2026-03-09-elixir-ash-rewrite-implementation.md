# Panko Elixir/Ash Rewrite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite Panko from a Rust CLI into an Elixir/Ash/Phoenix LiveView web app for browsing and sharing AI coding agent sessions.

**Architecture:** Flat Phoenix app with 3 Ash domains (Sessions, Sharing, Accounts stub). PostgreSQL 18 everywhere. Background GenServer imports Claude Code JSONL sessions. LiveView for browsing. Slug-based sharing with optional expiry.

**Tech Stack:** Elixir, Phoenix 1.8+, LiveView 1.1+, Ash 3.x, AshPostgres 2.x, AshPhoenix 2.x, AshAi 0.2+, Oban, Tidewave, Tailwind 4, daisyUI, PostgreSQL 18, Nix flakes

**Design doc:** `docs/plans/2026-03-09-elixir-ash-rewrite-design.md`

**Reference apps:**
- `~/dev/jordangarrison/greenlight` — Ash without DB, GenServer polling, NixOS module
- `~/dev/jordangarrison/rgd` — Ash with Postgres, upserts, custom changes, daisyUI

---

## Phase 1: Project Bootstrap

### Task 1: Clean Rust artifacts and scaffold Phoenix app

**Files:**
- Remove: `src/`, `Cargo.toml`, `Cargo.lock`, `build.rs`, `templates/`, `tests/`
- Remove: `nix/devshell.nix`, `nix/package.nix`, `flake.nix`, `flake.lock`
- Keep: `docs/`, `CLAUDE.md`, `LICENSE`, `README.md`, `.gitignore`, `PROGRESS.md`
- Create: Phoenix app scaffolding via `mix phx.new`

**Step 1: Remove Rust source files**

```bash
rm -rf src/ templates/ tests/ Cargo.toml Cargo.lock build.rs .direnv/
rm -f nix/devshell.nix nix/package.nix flake.nix flake.lock
```

**Step 2: Scaffold Phoenix app in current directory**

We need to scaffold in a temp directory then move files, since `mix phx.new` expects an empty target.

```bash
cd /tmp
mix phx.new panko --no-ecto --no-mailer --no-dashboard
```

We use `--no-ecto` because Ash manages the repo. Copy the generated files into our working directory.

```bash
cp -r /tmp/panko/{lib,config,priv,assets,test,.formatter.exs,mix.exs} \
  /home/jordangarrison/.grove/tasks/panko-elixir-ash-rewrite/panko/
rm -rf /tmp/panko
```

**Step 3: Update `.gitignore` for Elixir/Phoenix**

Replace contents with standard Phoenix + Nix ignores:

```gitignore
# Elixir/Phoenix
/_build/
/deps/
/doc/
*.ez
*.beam

# Assets
/priv/static/assets/
/priv/static/cache_manifest.json
node_modules/
/assets/node_modules/
npm-debug.log

# Database
*.db
*.db-journal

# Nix
/result
.direnv/

# Environment
.env

# MCP (generated per-port)
.mcp.json

# Grove
.grove/*
!.grove/config.json
```

**Step 4: Verify Phoenix app compiles**

```bash
mix deps.get
mix compile
```

Expected: Clean compilation with no errors.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: scaffold Phoenix app, remove Rust source"
```

---

### Task 2: Nix flake for Elixir development

**Files:**
- Create: `flake.nix`
- Create: `nix/devshell.nix`
- Create: `.envrc`

**Step 1: Write `flake.nix`**

Reference: `~/dev/jordangarrison/greenlight/flake.nix`

```nix
{
  description = "panko - web app for viewing and sharing AI coding agent sessions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports = [
        ./nix/devshell.nix
      ];
    };
}
```

**Step 2: Write `nix/devshell.nix`**

```nix
{ inputs, ... }:
{
  perSystem = { pkgs, system, ... }:
    let
      erlang = pkgs.beam.packages.erlang_28;
      elixir = erlang.elixir;
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs = [
          elixir
          erlang.erlang
          pkgs.postgresql
          pkgs.tailwindcss_4
          pkgs.nodejs
          pkgs.inotify-tools
        ];

        env = {
          MIX_TAILWIND_PATH = "${pkgs.tailwindcss_4}/bin/tailwindcss";
          LANG = "en_US.UTF-8";
          ERL_AFLAGS = "-kernel shell_history enabled";
        };

        shellHook = ''
          mix local.hex --if-missing --force
          mix local.rebar --if-missing --force
          echo "panko dev shell loaded"
          echo "Elixir: $(elixir --version | tail -1)"
          echo "PostgreSQL: $(postgres --version)"
        '';
      };
    };
}
```

**Step 3: Write `.envrc`**

```
use flake
```

**Step 4: Enter devshell and verify**

```bash
direnv allow
elixir --version
postgres --version
```

Expected: Elixir 1.18+, PostgreSQL 18.x

**Step 5: Commit**

```bash
git add flake.nix flake.lock nix/devshell.nix .envrc
git commit -m "feat: nix flake with Elixir devshell"
```

---

### Task 3: Add Ash and core dependencies

**Files:**
- Modify: `mix.exs`
- Create: `lib/panko/repo.ex`
- Modify: `lib/panko/application.ex`
- Modify: `config/config.exs`
- Modify: `config/dev.exs`
- Create: `config/runtime.exs`
- Create: `config/test.exs`

**Step 1: Update `mix.exs` with Ash dependencies**

Replace the `deps` function:

```elixir
defp deps do
  [
    # Phoenix
    {:phoenix, "~> 1.8"},
    {:phoenix_html, "~> 4.2"},
    {:phoenix_live_reload, "~> 1.5", only: :dev},
    {:phoenix_live_view, "~> 1.1"},
    {:phoenix_live_dashboard, "~> 0.8"},
    {:bandit, "~> 1.6"},
    {:telemetry_metrics, "~> 1.0"},
    {:telemetry_poller, "~> 1.0"},
    {:jason, "~> 1.4"},
    {:gettext, "~> 0.26"},
    {:dns_cluster, "~> 0.1"},

    # Ash
    {:ash, "~> 3.0"},
    {:ash_postgres, "~> 2.0"},
    {:ash_phoenix, "~> 2.0"},
    {:ash_ai, "~> 0.2"},

    # Background jobs
    {:oban, "~> 2.19"},

    # File watching
    {:file_system, "~> 1.0"},

    # Dev tools
    {:tidewave, "~> 0.5", only: :dev},
    {:esbuild, "~> 0.9", runtime: Mix.env() == :dev},
    {:tailwind, "~> 0.3", runtime: Mix.env() == :dev},
    {:floki, ">= 0.30.0", only: :test}
  ]
end
```

Add to the `project` function:

```elixir
def project do
  [
    app: :panko,
    version: "0.1.0",
    elixir: "~> 1.15",
    elixirc_paths: elixirc_paths(Mix.env()),
    start_permanent: Mix.env() == :prod,
    aliases: aliases(),
    deps: deps(),
    listeners: [Phoenix.CodeReloader]
  ]
end
```

**Step 2: Create `lib/panko/repo.ex`**

Reference: `~/dev/jordangarrison/rgd/apps/rgd_core/lib/rgd_core/repo.ex`

```elixir
defmodule Panko.Repo do
  use AshPostgres.Repo, otp_app: :panko

  def installed_extensions do
    ["uuid-ossp", "citext", "ash-functions"]
  end

  def min_pg_version do
    %Version{major: 18, minor: 0, patch: 0}
  end
end
```

**Step 3: Update `config/config.exs`**

Add Ash and Ecto configuration:

```elixir
# Ash domains
config :panko, ash_domains: [Panko.Sessions, Panko.Sharing]

# Repo
config :panko, ecto_repos: [Panko.Repo]

# Oban
config :panko, Oban,
  repo: Panko.Repo,
  queues: [default: 10, shares: 5]
```

**Step 4: Create `config/dev.exs` database config**

Add:

```elixir
config :panko, Panko.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "panko_dev",
  stacktrace: true,
  show_sensitive_data_on_connection_error: true,
  pool_size: 10
```

**Step 5: Create `config/test.exs`**

```elixir
import Config

config :panko, Panko.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "panko_test#{System.get_env("MIX_TEST_PARTITION")}",
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: System.schedulers_online() * 2

config :panko, PankoWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: String.duplicate("test", 16),
  server: false

config :panko, Oban, testing: :manual

config :logger, level: :warning
```

**Step 6: Create `config/runtime.exs`**

```elixir
import Config

config :panko,
  session_watch_paths:
    System.get_env("PANKO_WATCH_PATHS", Path.expand("~/.claude/projects")),
  api_key: System.get_env("PANKO_API_KEY"),
  default_share_expiry: System.get_env("PANKO_DEFAULT_EXPIRY", "7d"),
  instance_origin_id: System.get_env("PANKO_ORIGIN_ID", "local")

if System.get_env("PHX_SERVER") do
  config :panko, PankoWeb.Endpoint, server: true
end

if config_env() == :prod do
  database_url =
    System.get_env("DATABASE_URL") ||
      raise "DATABASE_URL not set"

  config :panko, Panko.Repo,
    url: database_url,
    pool_size: String.to_integer(System.get_env("POOL_SIZE") || "10")

  secret_key_base =
    System.get_env("SECRET_KEY_BASE") ||
      raise "SECRET_KEY_BASE not set"

  host = System.get_env("PHX_HOST") || "localhost"
  port = String.to_integer(System.get_env("PORT") || "4000")

  config :panko, :dns_cluster_query, System.get_env("DNS_CLUSTER_QUERY")

  config :panko, PankoWeb.Endpoint,
    url: [host: host, port: 443, scheme: "https"],
    http: [
      ip: {0, 0, 0, 0, 0, 0, 0, 0},
      port: port
    ],
    secret_key_base: secret_key_base
end
```

**Step 7: Update `lib/panko/application.ex`**

Add Repo and Oban to the supervision tree:

```elixir
defmodule Panko.Application do
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      PankoWeb.Telemetry,
      Panko.Repo,
      {DNSCluster, query: Application.get_env(:panko, :dns_cluster_query) || :ignore},
      {Phoenix.PubSub, name: Panko.PubSub},
      {Oban, Application.fetch_env!(:panko, Oban)},
      PankoWeb.Endpoint
    ]

    opts = [strategy: :one_for_one, name: Panko.Supervisor]
    Supervisor.start_link(children, opts)
  end

  @impl true
  def config_change(changed, _new, removed) do
    PankoWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
```

**Step 8: Update `lib/panko_web/endpoint.ex` with Tidewave + Ash AI**

Add before the `if code_reloading?` block:

```elixir
if Code.ensure_loaded?(Tidewave) do
  plug Tidewave
end
```

Inside the `if code_reloading?` block, add:

```elixir
if Code.ensure_loaded?(AshAi.Mcp.Dev) do
  plug AshAi.Mcp.Dev,
    protocol_version_statement: "2024-11-05",
    otp_app: :panko
end
```

**Step 9: Fetch deps, create database, verify compilation**

```bash
mix deps.get
mix compile
mix ecto.create
```

Expected: Compilation succeeds. Database created.

**Step 10: Commit**

```bash
git add -A
git commit -m "feat: add Ash, AshPostgres, Oban, Tidewave dependencies"
```

---

## Phase 2: Data Model

### Task 4: Ash enum types

**Files:**
- Create: `lib/panko/sessions/source_type.ex`
- Create: `lib/panko/sessions/block/type.ex`
- Create: `lib/panko/sessions/sub_agent_status.ex`
- Test: `test/panko/sessions/enums_test.exs`

**Step 1: Write tests for enum types**

```elixir
# test/panko/sessions/enums_test.exs
defmodule Panko.Sessions.EnumsTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.SourceType
  alias Panko.Sessions.Block.Type, as: BlockType
  alias Panko.Sessions.SubAgentStatus

  describe "SourceType" do
    test "has expected values" do
      assert :claude_code in SourceType.values()
      assert :codex in SourceType.values()
    end

    test "casts valid string" do
      assert {:ok, :claude_code} = Ash.Type.cast_input(SourceType, "claude_code")
    end

    test "rejects invalid value" do
      assert :error = Ash.Type.cast_input(SourceType, "invalid")
    end
  end

  describe "BlockType" do
    test "has all block types" do
      values = BlockType.values()
      assert :user_prompt in values
      assert :assistant_response in values
      assert :tool_call in values
      assert :thinking in values
      assert :file_edit in values
      assert :sub_agent_spawn in values
    end
  end

  describe "SubAgentStatus" do
    test "has expected values" do
      values = SubAgentStatus.values()
      assert :running in values
      assert :completed in values
      assert :failed in values
    end
  end
end
```

**Step 2: Run test to verify it fails**

```bash
mix test test/panko/sessions/enums_test.exs
```

Expected: FAIL — modules not defined.

**Step 3: Create enum modules**

```elixir
# lib/panko/sessions/source_type.ex
defmodule Panko.Sessions.SourceType do
  use Ash.Type.Enum, values: [:claude_code, :codex]
end
```

```elixir
# lib/panko/sessions/block/type.ex
defmodule Panko.Sessions.Block.Type do
  use Ash.Type.Enum,
    values: [
      :user_prompt,
      :assistant_response,
      :tool_call,
      :thinking,
      :file_edit,
      :sub_agent_spawn
    ]
end
```

```elixir
# lib/panko/sessions/sub_agent_status.ex
defmodule Panko.Sessions.SubAgentStatus do
  use Ash.Type.Enum, values: [:running, :completed, :failed]
end
```

**Step 4: Run tests**

```bash
mix test test/panko/sessions/enums_test.exs
```

Expected: All pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Ash enum types for sessions"
```

---

### Task 5: Session resource

**Files:**
- Create: `lib/panko/sessions/session.ex`
- Create: `lib/panko/sessions.ex` (domain)
- Test: `test/panko/sessions/session_test.exs`

**Step 1: Create the Sessions domain (minimal)**

```elixir
# lib/panko/sessions.ex
defmodule Panko.Sessions do
  use Ash.Domain

  resources do
    resource Panko.Sessions.Session
  end
end
```

**Step 2: Create the Session resource**

```elixir
# lib/panko/sessions/session.ex
defmodule Panko.Sessions.Session do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "sessions"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :external_id, :string do
      allow_nil? false
      public? true
    end

    attribute :source_type, Panko.Sessions.SourceType do
      allow_nil? false
      public? true
    end

    attribute :source_path, :string do
      allow_nil? true
      public? true
    end

    attribute :project, :string do
      allow_nil? true
      public? true
    end

    attribute :title, :string do
      allow_nil? true
      public? true
    end

    attribute :started_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :user_id, :uuid do
      allow_nil? true
      public? true
    end

    attribute :origin_id, :string do
      allow_nil? true
      public? true
    end

    timestamps()
  end

  identities do
    identity :external_id_source_type, [:external_id, :source_type]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [
        :external_id,
        :source_type,
        :source_path,
        :project,
        :title,
        :started_at,
        :user_id,
        :origin_id
      ]
    end

    read :list_recent do
      prepare build(sort: [started_at: :desc], limit: 50)
    end
  end
end
```

**Step 3: Generate and run migration**

```bash
mix ash.codegen create_sessions
mix ash.migrate
```

**Step 4: Write test**

```elixir
# test/panko/sessions/session_test.exs
defmodule Panko.Sessions.SessionTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.Session

  describe "create" do
    test "creates a session with valid attributes" do
      assert {:ok, session} =
               Session
               |> Ash.Changeset.for_create(:create, %{
                 external_id: "test-session-123",
                 source_type: :claude_code,
                 source_path: "/tmp/test.jsonl",
                 project: "my-project",
                 title: "Test session",
                 started_at: ~U[2026-03-09 12:00:00Z]
               })
               |> Ash.create()

      assert session.external_id == "test-session-123"
      assert session.source_type == :claude_code
      assert session.project == "my-project"
    end

    test "requires external_id and source_type" do
      assert {:error, _} =
               Session
               |> Ash.Changeset.for_create(:create, %{
                 started_at: ~U[2026-03-09 12:00:00Z]
               })
               |> Ash.create()
    end
  end
end
```

**Step 5: Create test support `DataCase` module**

```elixir
# test/support/data_case.ex
defmodule Panko.DataCase do
  use ExUnit.CaseTemplate

  using do
    quote do
      alias Panko.Repo
      import Panko.DataCase
    end
  end

  setup tags do
    Panko.DataCase.setup_sandbox(tags)
    :ok
  end

  def setup_sandbox(tags) do
    pid = Ecto.Adapters.SQL.Sandbox.start_owner!(Panko.Repo, shared: not tags[:async])
    on_exit(fn -> Ecto.Adapters.SQL.Sandbox.stop_owner(pid) end)
  end
end
```

Ensure `test/test_helper.exs` starts the repo:

```elixir
Panko.Repo.start_link()
Ecto.Adapters.SQL.Sandbox.mode(Panko.Repo, :manual)
ExUnit.start()
```

**Step 6: Run tests**

```bash
mix test test/panko/sessions/session_test.exs
```

Expected: All pass.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add Session resource with Ash domain"
```

---

### Task 6: Block resource with embedded metadata

**Files:**
- Create: `lib/panko/sessions/block.ex`
- Create: `lib/panko/sessions/block/tool_call_metadata.ex`
- Create: `lib/panko/sessions/block/file_edit_metadata.ex`
- Create: `lib/panko/sessions/block/sub_agent_spawn_metadata.ex`
- Modify: `lib/panko/sessions.ex` (add Block to domain)
- Modify: `lib/panko/sessions/session.ex` (add has_many + aggregates)
- Test: `test/panko/sessions/block_test.exs`

**Step 1: Create embedded metadata resources**

```elixir
# lib/panko/sessions/block/tool_call_metadata.ex
defmodule Panko.Sessions.Block.ToolCallMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :name, :string, allow_nil?: false, public?: true
    attribute :input, :map, public?: true
    attribute :output, :map, public?: true
  end
end
```

```elixir
# lib/panko/sessions/block/file_edit_metadata.ex
defmodule Panko.Sessions.Block.FileEditMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :path, :string, allow_nil?: false, public?: true
    attribute :diff, :string, public?: true
  end
end
```

```elixir
# lib/panko/sessions/block/sub_agent_spawn_metadata.ex
defmodule Panko.Sessions.Block.SubAgentSpawnMetadata do
  use Ash.Resource, data_layer: :embedded

  attributes do
    attribute :agent_id, :string, public?: true
    attribute :agent_type, :string, public?: true
    attribute :description, :string, public?: true
  end
end
```

**Step 2: Create Block resource**

```elixir
# lib/panko/sessions/block.ex
defmodule Panko.Sessions.Block do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "blocks"
    repo Panko.Repo

    custom_indexes do
      index [:session_id, :position], unique: true
    end
  end

  attributes do
    uuid_primary_key :id

    attribute :position, :integer do
      allow_nil? false
      public? true
    end

    attribute :block_type, Panko.Sessions.Block.Type do
      allow_nil? false
      public? true
    end

    attribute :content, :string do
      allow_nil? true
      public? true
      constraints max_length: nil
    end

    attribute :metadata, :map do
      allow_nil? true
      public? true
    end

    attribute :timestamp, :utc_datetime do
      allow_nil? true
      public? true
    end

    create_timestamp :inserted_at
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      allow_nil? false
      public? true
    end
  end

  identities do
    identity :session_position, [:session_id, :position]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [:position, :block_type, :content, :metadata, :timestamp, :session_id]
    end
  end
end
```

**Step 3: Update Session resource — add relationship and aggregates**

Add to `lib/panko/sessions/session.ex`:

Relationships:
```elixir
relationships do
  has_many :blocks, Panko.Sessions.Block do
    sort position: :asc
    public? true
  end
end
```

Aggregates:
```elixir
aggregates do
  count :block_count, :blocks

  count :tool_call_count, :blocks do
    filter expr(block_type == :tool_call)
  end

  count :file_edit_count, :blocks do
    filter expr(block_type == :file_edit)
  end

  count :message_count, :blocks do
    filter expr(block_type in [:user_prompt, :assistant_response])
  end

  first :last_activity_at, :blocks, :timestamp do
    sort timestamp: :desc
  end
end
```

**Step 4: Update domain**

Add Block to `lib/panko/sessions.ex`:

```elixir
resources do
  resource Panko.Sessions.Session
  resource Panko.Sessions.Block
end
```

**Step 5: Generate and run migration**

```bash
mix ash.codegen create_blocks
mix ash.migrate
```

**Step 6: Write test**

```elixir
# test/panko/sessions/block_test.exs
defmodule Panko.Sessions.BlockTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.{Session, Block}

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "block-test-session",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  describe "create" do
    test "creates a user_prompt block", %{session: session} do
      assert {:ok, block} =
               Block
               |> Ash.Changeset.for_create(:create, %{
                 session_id: session.id,
                 position: 0,
                 block_type: :user_prompt,
                 content: "Hello, Claude!",
                 timestamp: ~U[2026-03-09 12:00:01Z]
               })
               |> Ash.create()

      assert block.block_type == :user_prompt
      assert block.content == "Hello, Claude!"
      assert block.position == 0
    end

    test "creates a tool_call block with metadata", %{session: session} do
      metadata = %{"name" => "Bash", "input" => %{"command" => "ls"}, "output" => %{"text" => "file.txt"}}

      assert {:ok, block} =
               Block
               |> Ash.Changeset.for_create(:create, %{
                 session_id: session.id,
                 position: 1,
                 block_type: :tool_call,
                 content: nil,
                 metadata: metadata,
                 timestamp: ~U[2026-03-09 12:00:02Z]
               })
               |> Ash.create()

      assert block.metadata["name"] == "Bash"
    end

    test "enforces unique session_id + position", %{session: session} do
      attrs = %{
        session_id: session.id,
        position: 0,
        block_type: :user_prompt,
        content: "First",
        timestamp: ~U[2026-03-09 12:00:01Z]
      }

      assert {:ok, _} = Block |> Ash.Changeset.for_create(:create, attrs) |> Ash.create()
      assert {:error, _} = Block |> Ash.Changeset.for_create(:create, attrs) |> Ash.create()
    end
  end

  describe "session aggregates" do
    test "counts blocks by type", %{session: session} do
      for {type, pos} <- [{:user_prompt, 0}, {:assistant_response, 1}, {:tool_call, 2}, {:tool_call, 3}] do
        Block
        |> Ash.Changeset.for_create(:create, %{
          session_id: session.id,
          position: pos,
          block_type: type,
          timestamp: ~U[2026-03-09 12:00:00Z]
        })
        |> Ash.create!()
      end

      session =
        Session
        |> Ash.get!(session.id, load: [:block_count, :tool_call_count, :message_count])

      assert session.block_count == 4
      assert session.tool_call_count == 2
      assert session.message_count == 2
    end
  end
end
```

**Step 7: Run tests**

```bash
mix test test/panko/sessions/block_test.exs
```

Expected: All pass.

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: add Block resource with metadata and session aggregates"
```

---

### Task 7: SubAgent resource

**Files:**
- Create: `lib/panko/sessions/sub_agent.ex`
- Modify: `lib/panko/sessions.ex` (add SubAgent to domain)
- Modify: `lib/panko/sessions/session.ex` (add has_many :sub_agents)
- Test: `test/panko/sessions/sub_agent_test.exs`

**Step 1: Create SubAgent resource**

```elixir
# lib/panko/sessions/sub_agent.ex
defmodule Panko.Sessions.SubAgent do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "sub_agents"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :external_id, :string do
      allow_nil? false
      public? true
    end

    attribute :agent_type, :string do
      allow_nil? false
      public? true
    end

    attribute :description, :string do
      allow_nil? true
      public? true
    end

    attribute :prompt, :string do
      allow_nil? true
      public? true
      constraints max_length: nil
    end

    attribute :status, Panko.Sessions.SubAgentStatus do
      allow_nil? false
      public? true
    end

    attribute :result, :string do
      allow_nil? true
      public? true
      constraints max_length: nil
    end

    attribute :spawned_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :completed_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    create_timestamp :inserted_at
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      allow_nil? false
      public? true
    end
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [
        :external_id, :agent_type, :description, :prompt,
        :status, :result, :spawned_at, :completed_at, :session_id
      ]
    end
  end
end
```

**Step 2: Update Session — add `has_many :sub_agents`**

```elixir
has_many :sub_agents, Panko.Sessions.SubAgent do
  public? true
end
```

**Step 3: Update domain — add SubAgent**

```elixir
resource Panko.Sessions.SubAgent
```

**Step 4: Generate migration and migrate**

```bash
mix ash.codegen create_sub_agents
mix ash.migrate
```

**Step 5: Write test**

```elixir
# test/panko/sessions/sub_agent_test.exs
defmodule Panko.Sessions.SubAgentTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.{Session, SubAgent}

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "subagent-test-session",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  test "creates a sub_agent", %{session: session} do
    assert {:ok, agent} =
             SubAgent
             |> Ash.Changeset.for_create(:create, %{
               session_id: session.id,
               external_id: "toolu_abc123",
               agent_type: "Explore",
               description: "Search for patterns",
               prompt: "Find all GenServer modules",
               status: :completed,
               result: "Found 3 GenServers",
               spawned_at: ~U[2026-03-09 12:00:05Z],
               completed_at: ~U[2026-03-09 12:00:10Z]
             })
             |> Ash.create()

    assert agent.agent_type == "Explore"
    assert agent.status == :completed
  end
end
```

**Step 6: Run tests**

```bash
mix test test/panko/sessions/sub_agent_test.exs
```

Expected: All pass.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add SubAgent resource"
```

---

## Phase 3: Parser

### Task 8: Parser behaviour and registry

**Files:**
- Create: `lib/panko/sessions/parsers/parser.ex`
- Create: `lib/panko/sessions/parsers/registry.ex`
- Test: `test/panko/sessions/parsers/registry_test.exs`

**Step 1: Create the Parser behaviour**

```elixir
# lib/panko/sessions/parsers/parser.ex
defmodule Panko.Sessions.Parsers.Parser do
  @moduledoc """
  Behaviour for session file parsers.

  Parsers are pure functions: file path in, session attributes out.
  The returned map must match the shape expected by Session's
  `:upsert_from_import` action.
  """

  @type session_attrs :: %{
          external_id: String.t(),
          source_type: atom(),
          source_path: String.t(),
          project: String.t() | nil,
          title: String.t() | nil,
          started_at: DateTime.t(),
          blocks: [map()],
          sub_agents: [map()]
        }

  @callback source_type() :: atom()
  @callback can_parse?(path :: String.t()) :: boolean()
  @callback parse(path :: String.t()) :: {:ok, session_attrs()} | {:error, term()}
end
```

**Step 2: Create the Registry**

```elixir
# lib/panko/sessions/parsers/registry.ex
defmodule Panko.Sessions.Parsers.Registry do
  @moduledoc """
  Finds the appropriate parser for a given file path.
  """

  @parsers [
    Panko.Sessions.Parsers.ClaudeCode
  ]

  @spec find_parser(String.t()) :: {:ok, module()} | {:error, :no_parser_found}
  def find_parser(path) do
    case Enum.find(@parsers, & &1.can_parse?(path)) do
      nil -> {:error, :no_parser_found}
      parser -> {:ok, parser}
    end
  end

  @spec parsers() :: [module()]
  def parsers, do: @parsers
end
```

**Step 3: Write test**

```elixir
# test/panko/sessions/parsers/registry_test.exs
defmodule Panko.Sessions.Parsers.RegistryTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.Parsers.Registry

  test "finds ClaudeCode parser for .jsonl files" do
    assert {:ok, Panko.Sessions.Parsers.ClaudeCode} = Registry.find_parser("/tmp/session.jsonl")
  end

  test "returns error for unknown file types" do
    assert {:error, :no_parser_found} = Registry.find_parser("/tmp/session.xml")
  end
end
```

Note: This test will fail until we create the ClaudeCode parser (Task 9). That's fine — we'll create a stub.

**Step 4: Create ClaudeCode parser stub**

```elixir
# lib/panko/sessions/parsers/claude_code.ex
defmodule Panko.Sessions.Parsers.ClaudeCode do
  @moduledoc """
  Parser for Claude Code JSONL session files.
  """

  @behaviour Panko.Sessions.Parsers.Parser

  @impl true
  def source_type, do: :claude_code

  @impl true
  def can_parse?(path), do: String.ends_with?(path, ".jsonl")

  @impl true
  def parse(_path) do
    {:error, :not_implemented}
  end
end
```

**Step 5: Run test**

```bash
mix test test/panko/sessions/parsers/registry_test.exs
```

Expected: All pass.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add parser behaviour and registry"
```

---

### Task 9: Claude Code parser — core implementation

**Files:**
- Modify: `lib/panko/sessions/parsers/claude_code.ex`
- Create: `test/fixtures/simple_session.jsonl`
- Test: `test/panko/sessions/parsers/claude_code_test.exs`

**Step 1: Create test fixture**

Create a minimal Claude Code JSONL fixture at `test/fixtures/simple_session.jsonl`. The fixture should contain:
- 1 user message (plain text)
- 1 assistant response (text)
- 1 assistant message with tool_use (Bash)
- 1 user message with tool_result
- 1 assistant response (text after tool)

Use the exact JSONL format discovered from real session files:

```jsonl
{"type":"user","sessionId":"test-abc-123","uuid":"u1","timestamp":"2026-03-09T12:00:00.000Z","cwd":"/home/user/my-project","version":"2.1.56","gitBranch":"main","message":{"role":"user","content":"List the files in the current directory"}}
{"type":"assistant","sessionId":"test-abc-123","uuid":"a1","parentUuid":"u1","timestamp":"2026-03-09T12:00:01.000Z","cwd":"/home/user/my-project","version":"2.1.56","gitBranch":"main","requestId":"req_1","message":{"model":"claude-opus-4-6","id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"I'll list the files for you."},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls -la"}}],"stop_reason":"tool_use","usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"user","sessionId":"test-abc-123","uuid":"u2","parentUuid":"a1","timestamp":"2026-03-09T12:00:02.000Z","cwd":"/home/user/my-project","version":"2.1.56","gitBranch":"main","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":[{"type":"text","text":"total 4\n-rw-r--r-- 1 user user 100 file.txt"}]}]}}
{"type":"assistant","sessionId":"test-abc-123","uuid":"a2","parentUuid":"u2","timestamp":"2026-03-09T12:00:03.000Z","cwd":"/home/user/my-project","version":"2.1.56","gitBranch":"main","requestId":"req_2","message":{"model":"claude-opus-4-6","id":"msg_2","type":"message","role":"assistant","content":[{"type":"text","text":"The directory contains one file: `file.txt` (100 bytes)."}],"stop_reason":"end_turn","usage":{"input_tokens":200,"output_tokens":30}}}
```

**Step 2: Write parser tests**

```elixir
# test/panko/sessions/parsers/claude_code_test.exs
defmodule Panko.Sessions.Parsers.ClaudeCodeTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.Parsers.ClaudeCode

  @fixtures_dir Path.join([__DIR__, "../../../fixtures"])

  describe "can_parse?/1" do
    test "returns true for .jsonl files" do
      assert ClaudeCode.can_parse?("/path/to/session.jsonl")
    end

    test "returns false for other files" do
      refute ClaudeCode.can_parse?("/path/to/file.json")
      refute ClaudeCode.can_parse?("/path/to/file.txt")
    end
  end

  describe "parse/1" do
    test "parses a simple session" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      assert {:ok, attrs} = ClaudeCode.parse(path)

      assert attrs.external_id == "test-abc-123"
      assert attrs.source_type == :claude_code
      assert attrs.source_path == path
      assert attrs.project == "/home/user/my-project"
      assert attrs.title == "List the files in the current directory"
      assert %DateTime{} = attrs.started_at
    end

    test "extracts blocks in order" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      assert length(attrs.blocks) >= 3

      types = Enum.map(attrs.blocks, & &1.block_type)
      assert :user_prompt in types
      assert :assistant_response in types
      assert :tool_call in types
    end

    test "extracts tool call metadata" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      {:ok, attrs} = ClaudeCode.parse(path)

      tool_block = Enum.find(attrs.blocks, &(&1.block_type == :tool_call))
      assert tool_block != nil
      assert tool_block.metadata["name"] == "Bash"
      assert tool_block.metadata["input"] == %{"command" => "ls -la"}
    end

    test "returns error for non-existent file" do
      assert {:error, _} = ClaudeCode.parse("/nonexistent/file.jsonl")
    end
  end
end
```

**Step 3: Run tests to verify they fail**

```bash
mix test test/panko/sessions/parsers/claude_code_test.exs
```

Expected: FAIL — parse returns `{:error, :not_implemented}`

**Step 4: Implement the parser**

Replace the `parse/1` function in `lib/panko/sessions/parsers/claude_code.ex`:

```elixir
defmodule Panko.Sessions.Parsers.ClaudeCode do
  @moduledoc """
  Parser for Claude Code JSONL session files.

  Reads JSONL files line-by-line and converts them to the session
  attributes format expected by Session's import actions.
  """

  @behaviour Panko.Sessions.Parsers.Parser

  @impl true
  def source_type, do: :claude_code

  @impl true
  def can_parse?(path), do: String.ends_with?(path, ".jsonl")

  @impl true
  def parse(path) do
    case File.read(path) do
      {:ok, content} ->
        lines =
          content
          |> String.split("\n", trim: true)
          |> Enum.map(&Jason.decode!/1)

        session_id = extract_session_id(lines)
        project = extract_project(lines)
        started_at = extract_started_at(lines)
        title = extract_title(lines)

        {blocks, sub_agents} = extract_blocks_and_agents(lines)

        {:ok,
         %{
           external_id: session_id,
           source_type: :claude_code,
           source_path: path,
           project: project,
           title: title,
           started_at: started_at,
           blocks: blocks,
           sub_agents: sub_agents
         }}

      {:error, reason} ->
        {:error, {:file_read_error, reason}}
    end
  end

  defp extract_session_id(lines) do
    lines
    |> Enum.find_value(fn line -> line["sessionId"] end)
    |> Kernel.||("unknown")
  end

  defp extract_project(lines) do
    lines
    |> Enum.find_value(fn line -> line["cwd"] end)
  end

  defp extract_started_at(lines) do
    lines
    |> Enum.find_value(fn line -> line["timestamp"] end)
    |> parse_timestamp()
    |> Kernel.||(DateTime.utc_now())
  end

  defp extract_title(lines) do
    lines
    |> Enum.find(fn line ->
      line["type"] == "user" && is_binary(get_in(line, ["message", "content"]))
    end)
    |> case do
      nil -> nil
      line ->
        line
        |> get_in(["message", "content"])
        |> String.slice(0, 200)
    end
  end

  defp extract_blocks_and_agents(lines) do
    {blocks, agents, _pos} =
      lines
      |> Enum.filter(&(&1["type"] in ["user", "assistant"]))
      |> Enum.reduce({[], [], 0}, fn line, {blocks, agents, pos} ->
        {new_blocks, new_agents, new_pos} = process_line(line, pos)
        {blocks ++ new_blocks, agents ++ new_agents, new_pos}
      end)

    {blocks, agents}
  end

  defp process_line(%{"type" => "user", "message" => message} = line, pos) do
    content = message["content"]
    timestamp = parse_timestamp(line["timestamp"])

    cond do
      is_binary(content) ->
        block = %{
          position: pos,
          block_type: :user_prompt,
          content: content,
          metadata: nil,
          timestamp: timestamp
        }
        {[block], [], pos + 1}

      is_list(content) ->
        # Tool results — skip as standalone blocks, they're attached to tool_call blocks
        {[], [], pos}

      true ->
        {[], [], pos}
    end
  end

  defp process_line(%{"type" => "assistant", "message" => message} = line, pos) do
    content_parts = message["content"] || []
    timestamp = parse_timestamp(line["timestamp"])
    is_sidechain = line["isSidechain"] == true

    {blocks, agents, next_pos} =
      Enum.reduce(content_parts, {[], [], pos}, fn part, {blks, agts, p} ->
        case part["type"] do
          "text" ->
            block = %{
              position: p,
              block_type: :assistant_response,
              content: part["text"],
              metadata: nil,
              timestamp: timestamp
            }
            {blks ++ [block], agts, p + 1}

          "tool_use" ->
            {tool_block, maybe_agent} = process_tool_use(part, p, timestamp, is_sidechain)
            {blks ++ [tool_block], agts ++ maybe_agent, p + 1}

          "thinking" ->
            block = %{
              position: p,
              block_type: :thinking,
              content: part["thinking"],
              metadata: nil,
              timestamp: timestamp
            }
            {blks ++ [block], agts, p + 1}

          _ ->
            {blks, agts, p}
        end
      end)

    {blocks, agents, next_pos}
  end

  defp process_line(_line, pos), do: {[], [], pos}

  defp process_tool_use(part, pos, timestamp, _is_sidechain) do
    tool_name = part["name"]
    input = part["input"]
    tool_id = part["id"]

    {block_type, metadata} = categorize_tool(tool_name, input, tool_id)

    block = %{
      position: pos,
      block_type: block_type,
      content: nil,
      metadata: metadata,
      timestamp: timestamp
    }

    agents =
      if block_type == :sub_agent_spawn do
        [
          %{
            external_id: tool_id || "unknown",
            agent_type: input["subagent_type"] || input["type"] || "unknown",
            description: input["description"] || "",
            prompt: input["prompt"] || "",
            status: :running,
            spawned_at: timestamp
          }
        ]
      else
        []
      end

    {block, agents}
  end

  defp categorize_tool("Write", input, _id) do
    {:file_edit, %{"name" => "Write", "path" => input["file_path"], "input" => input}}
  end

  defp categorize_tool("Edit", input, _id) do
    {:file_edit, %{"name" => "Edit", "path" => input["file_path"], "input" => input}}
  end

  defp categorize_tool("Agent", input, _id) do
    {:sub_agent_spawn,
     %{
       "name" => "Agent",
       "agent_type" => input["subagent_type"] || input["type"],
       "description" => input["description"],
       "input" => input
     }}
  end

  defp categorize_tool(name, input, _id) do
    {:tool_call, %{"name" => name, "input" => input}}
  end

  defp parse_timestamp(nil), do: nil
  defp parse_timestamp(ts) when is_binary(ts) do
    case DateTime.from_iso8601(ts) do
      {:ok, dt, _offset} -> DateTime.truncate(dt, :second)
      _ -> nil
    end
  end
end
```

**Step 5: Run tests**

```bash
mix test test/panko/sessions/parsers/claude_code_test.exs
```

Expected: All pass.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement Claude Code JSONL parser"
```

---

### Task 10: Claude Code parser — tool results and edge cases

**Files:**
- Modify: `lib/panko/sessions/parsers/claude_code.ex`
- Create: `test/fixtures/complex_session.jsonl`
- Modify: `test/panko/sessions/parsers/claude_code_test.exs`

**Step 1: Create complex fixture**

Create `test/fixtures/complex_session.jsonl` with:
- Thinking blocks
- File edit (Write tool)
- Sub-agent spawn (Agent tool)
- Tool results attached to tool calls
- Progress events (should be skipped)
- File history snapshot (should be skipped)

**Step 2: Write tests for edge cases**

Add to the test file:

```elixir
describe "parse/1 complex session" do
  test "skips progress and file-history-snapshot records" do
    path = Path.join(@fixtures_dir, "complex_session.jsonl")
    {:ok, attrs} = ClaudeCode.parse(path)
    types = Enum.map(attrs.blocks, & &1.block_type)
    refute :progress in types
  end

  test "extracts file edit blocks" do
    path = Path.join(@fixtures_dir, "complex_session.jsonl")
    {:ok, attrs} = ClaudeCode.parse(path)
    edit = Enum.find(attrs.blocks, &(&1.block_type == :file_edit))
    assert edit != nil
    assert edit.metadata["path"] != nil
  end

  test "extracts sub_agent_spawn blocks and agents" do
    path = Path.join(@fixtures_dir, "complex_session.jsonl")
    {:ok, attrs} = ClaudeCode.parse(path)

    spawn_block = Enum.find(attrs.blocks, &(&1.block_type == :sub_agent_spawn))
    assert spawn_block != nil

    assert length(attrs.sub_agents) >= 1
    agent = hd(attrs.sub_agents)
    assert agent.agent_type != nil
    assert agent.status == :running
  end

  test "handles empty file" do
    path = Path.join(@fixtures_dir, "empty_session.jsonl")
    File.write!(path, "")
    assert {:ok, attrs} = ClaudeCode.parse(path)
    assert attrs.blocks == []
    File.rm!(path)
  end
end
```

**Step 3: Create the complex fixture file, run tests, iterate**

Build the fixture to match real JSONL format with Write, Agent, and thinking content types. Run tests and fix any parsing issues.

**Step 4: Run all parser tests**

```bash
mix test test/panko/sessions/parsers/
```

Expected: All pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: handle complex session parsing (edits, agents, thinking)"
```

---

## Phase 4: Import Pipeline

### Task 11: Ash import actions on Session

**Files:**
- Modify: `lib/panko/sessions/session.ex` (add upsert_from_import + import_from_file actions)
- Modify: `lib/panko/sessions.ex` (add domain code interfaces)
- Test: `test/panko/sessions/import_test.exs`

**Step 1: Write the import integration test**

```elixir
# test/panko/sessions/import_test.exs
defmodule Panko.Sessions.ImportTest do
  use Panko.DataCase, async: true

  @fixtures_dir Path.join([__DIR__, "../../fixtures"])

  describe "import_from_file" do
    test "imports a session from JSONL file" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")
      assert {:ok, session} = Panko.Sessions.import_from_file(path)

      assert session.external_id == "test-abc-123"
      assert session.source_type == :claude_code

      session = Ash.load!(session, [:blocks, :sub_agents, :block_count])
      assert session.block_count > 0
      assert length(session.blocks) > 0
    end

    test "upserts on reimport (same external_id)" do
      path = Path.join(@fixtures_dir, "simple_session.jsonl")

      assert {:ok, session1} = Panko.Sessions.import_from_file(path)
      assert {:ok, session2} = Panko.Sessions.import_from_file(path)

      assert session1.id == session2.id
    end

    test "returns error for unparseable file" do
      assert {:error, _} = Panko.Sessions.import_from_file("/tmp/nonexistent.jsonl")
    end
  end
end
```

**Step 2: Run test to verify it fails**

```bash
mix test test/panko/sessions/import_test.exs
```

Expected: FAIL — `import_from_file` not defined.

**Step 3: Add `upsert_from_import` create action to Session**

Add to the `actions` block in `lib/panko/sessions/session.ex`:

```elixir
create :upsert_from_import do
  accept [
    :external_id, :source_type, :source_path,
    :project, :title, :started_at
  ]

  upsert? true
  upsert_identity :external_id_source_type
  upsert_fields [:source_path, :project, :title, :started_at]

  argument :blocks, {:array, :map}, allow_nil?: false
  argument :sub_agents, {:array, :map}, default: []

  change manage_relationship(:blocks, :blocks, type: :direct_control)
  change manage_relationship(:sub_agents, :sub_agents, type: :direct_control)
end
```

**Step 4: Add `import_from_file` generic action to Session**

```elixir
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
```

**Step 5: Add domain code interfaces**

Update `lib/panko/sessions.ex`:

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

**Step 6: Run tests**

```bash
mix test test/panko/sessions/import_test.exs
```

Expected: All pass. May need to adjust Block/SubAgent create actions to accept `session_id` via relationship manager.

**Step 7: Run full test suite**

```bash
mix test
```

Expected: All pass.

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: add import_from_file Ash action with upsert support"
```

---

### Task 12: Session watcher GenServer

**Files:**
- Create: `lib/panko/sessions/session_watcher.ex`
- Modify: `lib/panko/application.ex` (add to supervision tree)
- Test: `test/panko/sessions/session_watcher_test.exs`

**Step 1: Write the SessionWatcher**

```elixir
# lib/panko/sessions/session_watcher.ex
defmodule Panko.Sessions.SessionWatcher do
  @moduledoc """
  Watches configured directories for new/modified JSONL session files
  and triggers import into the database.
  """
  use GenServer

  require Logger

  @debounce_ms 2_000

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(opts) do
    watch_paths =
      opts[:watch_paths] ||
        Application.get_env(:panko, :session_watch_paths, "~/.claude/projects")
        |> List.wrap()
        |> Enum.map(&Path.expand/1)

    # Start file watchers for each path
    watchers =
      for path <- watch_paths, File.dir?(path) do
        {:ok, pid} = FileSystem.start_link(dirs: [path])
        FileSystem.subscribe(pid)
        pid
      end

    # Initial scan
    send(self(), :initial_scan)

    {:ok, %{watchers: watchers, watch_paths: watch_paths, pending: %{}}}
  end

  @impl true
  def handle_info(:initial_scan, state) do
    Logger.info("SessionWatcher: scanning #{length(state.watch_paths)} paths")

    state.watch_paths
    |> Enum.flat_map(&find_jsonl_files/1)
    |> Enum.each(&import_file/1)

    {:noreply, state}
  end

  @impl true
  def handle_info({:file_event, _pid, {path, _events}}, state) do
    if String.ends_with?(path, ".jsonl") do
      # Debounce: schedule import after delay, reset if same file changes again
      timer = Process.send_after(self(), {:import, path}, @debounce_ms)

      state =
        case Map.get(state.pending, path) do
          nil -> state
          old_timer ->
            Process.cancel_timer(old_timer)
            state
        end

      {:noreply, put_in(state.pending[path], timer)}
    else
      {:noreply, state}
    end
  end

  @impl true
  def handle_info({:import, path}, state) do
    import_file(path)
    {:noreply, %{state | pending: Map.delete(state.pending, path)}}
  end

  @impl true
  def handle_info({:file_event, _pid, :stop}, state) do
    {:noreply, state}
  end

  defp find_jsonl_files(dir) do
    Path.wildcard(Path.join([dir, "**", "*.jsonl"]))
  end

  defp import_file(path) do
    Task.start(fn ->
      case Panko.Sessions.import_from_file(path) do
        {:ok, session} ->
          Logger.debug("Imported session #{session.external_id} from #{path}")

        {:error, reason} ->
          Logger.warning("Failed to import #{path}: #{inspect(reason)}")
      end
    end)
  end
end
```

**Step 2: Add to supervision tree**

In `lib/panko/application.ex`, add before `PankoWeb.Endpoint`:

```elixir
{Panko.Sessions.SessionWatcher, []}
```

**Step 3: Write test**

```elixir
# test/panko/sessions/session_watcher_test.exs
defmodule Panko.Sessions.SessionWatcherTest do
  use Panko.DataCase, async: false

  alias Panko.Sessions.SessionWatcher

  @tag :tmp_dir
  test "initial scan imports existing files", %{tmp_dir: tmp_dir} do
    # Write a fixture
    fixture = File.read!(Path.join(["test/fixtures", "simple_session.jsonl"]))
    jsonl_path = Path.join(tmp_dir, "session.jsonl")
    File.write!(jsonl_path, fixture)

    # Start watcher pointing at tmp dir
    {:ok, _pid} = SessionWatcher.start_link(watch_paths: [tmp_dir])

    # Give it time to scan and import
    Process.sleep(1_000)

    # Verify session was imported
    sessions = Ash.read!(Panko.Sessions.Session)
    assert length(sessions) >= 1
  end
end
```

**Step 4: Run test**

```bash
mix test test/panko/sessions/session_watcher_test.exs
```

Expected: Pass (may need to adjust timing).

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add SessionWatcher GenServer for file watching"
```

---

### Task 13: PubSub notifiers

**Files:**
- Modify: `lib/panko/sessions/session.ex` (add pub_sub notifier)
- Test: `test/panko/sessions/pubsub_test.exs`

**Step 1: Add PubSub notifier to Session resource**

Add to `lib/panko/sessions/session.ex`:

```elixir
use Ash.Resource,
  domain: Panko.Sessions,
  data_layer: AshPostgres.DataLayer,
  notifiers: [Ash.Notifier.PubSub]

pub_sub do
  module PankoWeb.Endpoint
  prefix "sessions"
  publish :upsert_from_import, ["imported"]
  publish_all :update, ["updated", :id]
  publish_all :destroy, ["destroyed", :id]
end
```

**Step 2: Write test**

```elixir
# test/panko/sessions/pubsub_test.exs
defmodule Panko.Sessions.PubSubTest do
  use Panko.DataCase, async: false

  test "broadcasts on session import" do
    PankoWeb.Endpoint.subscribe("sessions:imported")

    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, _session} = Panko.Sessions.import_from_file(path)

    assert_receive %Phoenix.Socket.Broadcast{topic: "sessions:imported"}, 1_000
  end
end
```

**Step 3: Run test**

```bash
mix test test/panko/sessions/pubsub_test.exs
```

Expected: Pass.

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add PubSub notifiers for real-time session updates"
```

---

## Phase 5: Web — Session Browsing

### Task 14: SessionsLive — list page

**Files:**
- Create: `lib/panko_web/live/sessions_live.ex`
- Modify: `lib/panko_web/router.ex`
- Create: `lib/panko_web/components/session_card.ex`
- Test: `test/panko_web/live/sessions_live_test.exs`

**Step 1: Update router**

```elixir
# lib/panko_web/router.ex
scope "/", PankoWeb do
  pipe_through :browser

  live_session :default, layout: {PankoWeb.Layouts, :app} do
    live "/", SessionsLive, :index
    live "/sessions/:id", SessionLive, :show
  end
end
```

**Step 2: Create SessionsLive**

```elixir
# lib/panko_web/live/sessions_live.ex
defmodule PankoWeb.SessionsLive do
  use PankoWeb, :live_view

  alias Panko.Sessions.Session

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      PankoWeb.Endpoint.subscribe("sessions:imported")
    end

    sessions = load_sessions()
    {:ok, assign(socket, sessions: sessions, page_title: "Sessions")}
  end

  @impl true
  def handle_info(%Phoenix.Socket.Broadcast{topic: "sessions:imported"}, socket) do
    sessions = load_sessions()
    {:noreply, assign(socket, sessions: sessions)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8">
      <h1 class="text-3xl font-bold mb-8">Sessions</h1>

      <div :if={@sessions == []} class="text-center py-12 text-base-content/60">
        <p class="text-lg">No sessions found.</p>
        <p class="text-sm mt-2">Sessions from ~/.claude/projects/ will appear here automatically.</p>
      </div>

      <div class="grid gap-4">
        <.link
          :for={session <- @sessions}
          navigate={~p"/sessions/#{session.id}"}
          class="card bg-base-200 shadow-sm hover:shadow-md transition-shadow"
        >
          <div class="card-body">
            <h2 class="card-title text-sm font-mono">
              {session.title || "Untitled session"}
            </h2>
            <p class="text-xs text-base-content/60">{session.project}</p>
            <div class="flex gap-4 text-xs text-base-content/50 mt-2">
              <span>{session.message_count || 0} messages</span>
              <span>{session.block_count || 0} blocks</span>
              <span>{format_time(session.started_at)}</span>
            </div>
          </div>
        </.link>
      </div>
    </div>
    """
  end

  defp load_sessions do
    Session
    |> Ash.Query.sort(started_at: :desc)
    |> Ash.Query.limit(50)
    |> Ash.Query.load([:block_count, :message_count, :tool_call_count])
    |> Ash.read!()
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt) do
    Calendar.strftime(dt, "%Y-%m-%d %H:%M")
  end
end
```

**Step 3: Write test**

```elixir
# test/panko_web/live/sessions_live_test.exs
defmodule PankoWeb.SessionsLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  test "renders empty state when no sessions", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/")
    assert render(view) =~ "No sessions found"
  end

  test "renders sessions list", %{conn: conn} do
    # Import a session first
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, _session} = Panko.Sessions.import_from_file(path)

    {:ok, _view, html} = live(conn, ~p"/")
    assert html =~ "List the files"
  end
end
```

**Step 4: Run tests**

```bash
mix test test/panko_web/live/sessions_live_test.exs
```

Expected: Pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add SessionsLive page with real-time updates"
```

---

### Task 15: SessionLive — detail page with block components

**Files:**
- Create: `lib/panko_web/live/session_live.ex`
- Create: `lib/panko_web/components/blocks/user_prompt.ex`
- Create: `lib/panko_web/components/blocks/assistant_response.ex`
- Create: `lib/panko_web/components/blocks/tool_call.ex`
- Create: `lib/panko_web/components/blocks/thinking.ex`
- Create: `lib/panko_web/components/blocks/file_edit.ex`
- Create: `lib/panko_web/components/blocks/sub_agent_spawn.ex`
- Create: `lib/panko_web/components/blocks.ex` (dispatcher)
- Test: `test/panko_web/live/session_live_test.exs`

**Step 1: Create block dispatcher component**

```elixir
# lib/panko_web/components/blocks.ex
defmodule PankoWeb.Components.Blocks do
  use Phoenix.Component

  alias PankoWeb.Components.Blocks.{
    UserPrompt,
    AssistantResponse,
    ToolCall,
    Thinking,
    FileEdit,
    SubAgentSpawn
  }

  attr :block, :map, required: true

  def block(%{block: %{block_type: :user_prompt}} = assigns), do: UserPrompt.render(assigns)
  def block(%{block: %{block_type: :assistant_response}} = assigns), do: AssistantResponse.render(assigns)
  def block(%{block: %{block_type: :tool_call}} = assigns), do: ToolCall.render(assigns)
  def block(%{block: %{block_type: :thinking}} = assigns), do: Thinking.render(assigns)
  def block(%{block: %{block_type: :file_edit}} = assigns), do: FileEdit.render(assigns)
  def block(%{block: %{block_type: :sub_agent_spawn}} = assigns), do: SubAgentSpawn.render(assigns)
  def block(assigns), do: ~H""
end
```

**Step 2: Create individual block components**

Each component is a function component rendering its block type with appropriate styling. For example:

```elixir
# lib/panko_web/components/blocks/user_prompt.ex
defmodule PankoWeb.Components.Blocks.UserPrompt do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="chat chat-end mb-4">
      <div class="chat-bubble chat-bubble-primary whitespace-pre-wrap">
        {@block.content}
      </div>
      <div class="chat-footer text-xs opacity-50">
        {format_time(@block.timestamp)}
      </div>
    </div>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

```elixir
# lib/panko_web/components/blocks/assistant_response.ex
defmodule PankoWeb.Components.Blocks.AssistantResponse do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="chat chat-start mb-4">
      <div class="chat-bubble whitespace-pre-wrap prose prose-sm max-w-none">
        {@block.content}
      </div>
    </div>
    """
  end
end
```

```elixir
# lib/panko_web/components/blocks/tool_call.ex
defmodule PankoWeb.Components.Blocks.ToolCall do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="collapse collapse-arrow bg-base-200 mb-4">
      <input type="checkbox" />
      <div class="collapse-title font-mono text-sm">
        <span class="badge badge-outline badge-sm mr-2">{@block.metadata["name"]}</span>
        Tool Call
      </div>
      <div class="collapse-content">
        <pre class="text-xs overflow-x-auto"><code>{Jason.encode!(@block.metadata["input"] || %{}, pretty: true)}</code></pre>
        <div :if={@block.metadata["output"]} class="mt-2 border-t border-base-300 pt-2">
          <p class="text-xs font-semibold mb-1">Output:</p>
          <pre class="text-xs overflow-x-auto"><code>{Jason.encode!(@block.metadata["output"] || %{}, pretty: true)}</code></pre>
        </div>
      </div>
    </div>
    """
  end
end
```

Create similar components for Thinking, FileEdit, and SubAgentSpawn following the same pattern with appropriate styling (thinking: italic/dimmed, file_edit: diff display, sub_agent: badge with type).

**Step 3: Create SessionLive**

```elixir
# lib/panko_web/live/session_live.ex
defmodule PankoWeb.SessionLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    case Panko.Sessions.get_session(id) do
      {:ok, session} ->
        session = Ash.load!(session, [:blocks, :sub_agents, :block_count, :message_count])
        {:ok, assign(socket, session: session, page_title: session.title || "Session")}

      {:error, _} ->
        {:ok, push_navigate(socket, to: ~p"/")}
    end
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <div class="mb-6">
        <.link navigate={~p"/"} class="btn btn-ghost btn-sm mb-4">&larr; Back</.link>
        <h1 class="text-2xl font-bold">{@session.title || "Untitled session"}</h1>
        <p class="text-sm text-base-content/60 mt-1">{@session.project}</p>
        <div class="flex gap-4 text-xs text-base-content/50 mt-2">
          <span>{@session.message_count} messages</span>
          <span>{@session.block_count} blocks</span>
          <span>{format_time(@session.started_at)}</span>
        </div>
      </div>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>
    </div>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%Y-%m-%d %H:%M")
end
```

**Step 4: Write test**

```elixir
# test/panko_web/live/session_live_test.exs
defmodule PankoWeb.SessionLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, session} = Panko.Sessions.import_from_file(path)
    %{session: session}
  end

  test "renders session detail", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "List the files"
  end

  test "shows blocks", %{conn: conn, session: session} do
    {:ok, _view, html} = live(conn, ~p"/sessions/#{session.id}")
    assert html =~ "Bash"
  end

  test "redirects for invalid session id", %{conn: conn} do
    assert {:error, {:live_redirect, %{to: "/"}}} =
             live(conn, ~p"/sessions/#{Ash.UUID.generate()}")
  end
end
```

**Step 5: Run tests**

```bash
mix test test/panko_web/live/session_live_test.exs
```

Expected: Pass.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add SessionLive with block components"
```

---

### Task 16: Styling — daisyUI theme

**Files:**
- Modify: `mix.exs` (add daisyUI to tailwind config if needed)
- Modify: `assets/css/app.css`
- Modify: `assets/js/app.js` (theme toggle)
- Modify: `lib/panko_web/components/layouts/root.html.heex`
- Modify: `lib/panko_web/components/layouts/app.html.heex`

**Step 1: Install daisyUI via npm**

```bash
cd assets && npm install daisyui && cd ..
```

**Step 2: Configure Tailwind + daisyUI in `assets/css/app.css`**

Add daisyUI import and custom Panko themes using oklch colors. Define two themes (dark "Midnight Panko" and light "Day Panko") with a primary accent color.

**Step 3: Update root layout**

Add theme persistence script, Google Fonts link, and proper meta tags.

**Step 4: Update app layout**

Add header with Panko branding, theme toggle, and flash messages.

**Step 5: Verify visually**

```bash
mix phx.server
```

Open http://localhost:4000 and verify the theme renders correctly.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add daisyUI theme with dark/light modes"
```

---

## Phase 6: Sharing

### Task 17: Share resource and Sharing domain

**Files:**
- Create: `lib/panko/sharing.ex` (domain)
- Create: `lib/panko/sharing/share.ex` (resource)
- Create: `lib/panko/sharing/changes/generate_slug.ex`
- Test: `test/panko/sharing/share_test.exs`

**Step 1: Create the GenerateSlug change**

```elixir
# lib/panko/sharing/changes/generate_slug.ex
defmodule Panko.Sharing.Changes.GenerateSlug do
  use Ash.Resource.Change

  @slug_length 8
  @alphabet ~c"abcdefghijklmnopqrstuvwxyz0123456789"

  @impl true
  def change(changeset, _opts, _context) do
    if Ash.Changeset.get_attribute(changeset, :slug) do
      changeset
    else
      slug = generate_slug()
      Ash.Changeset.force_change_attribute(changeset, :slug, slug)
    end
  end

  defp generate_slug do
    for _ <- 1..@slug_length, into: "" do
      <<Enum.random(@alphabet)>>
    end
  end
end
```

**Step 2: Create Share resource**

```elixir
# lib/panko/sharing/share.ex
defmodule Panko.Sharing.Share do
  use Ash.Resource,
    domain: Panko.Sharing,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "shares"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :slug, :string do
      allow_nil? false
      public? true
    end

    attribute :is_shared, :boolean do
      allow_nil? false
      default true
      public? true
    end

    attribute :expires_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    attribute :shared_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :unshared_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    attribute :user_id, :uuid do
      allow_nil? true
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      domain Panko.Sessions
      allow_nil? false
      public? true
    end
  end

  identities do
    identity :unique_slug, [:slug]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [:session_id, :expires_at]
      change {Panko.Sharing.Changes.GenerateSlug, []}
      change set_attribute(:shared_at, &DateTime.utc_now/0)
    end

    update :unpublish do
      accept []
      change set_attribute(:is_shared, false)
      change set_attribute(:unshared_at, &DateTime.utc_now/0)
    end

    update :republish do
      accept []
      change set_attribute(:is_shared, true)
      change set_attribute(:unshared_at, nil)
    end

    read :by_slug do
      argument :slug, :string, allow_nil?: false
      get? true
      filter expr(slug == ^arg(:slug) and is_shared == true)
      prepare build(load: [session: [:blocks, :sub_agents]])
    end

    read :active do
      filter expr(is_shared == true)
      prepare build(
        sort: [shared_at: :desc],
        load: [:session]
      )
    end
  end
end
```

**Step 3: Create Sharing domain**

```elixir
# lib/panko/sharing.ex
defmodule Panko.Sharing do
  use Ash.Domain

  resources do
    resource Panko.Sharing.Share do
      define :create_share, action: :create, args: [:session_id]
      define :unpublish_share, action: :unpublish
      define :republish_share, action: :republish
      define :get_share_by_slug, action: :by_slug, args: [:slug]
      define :list_active_shares, action: :active
    end
  end
end
```

**Step 4: Generate migration**

```bash
mix ash.codegen create_shares
mix ash.migrate
```

**Step 5: Write test**

```elixir
# test/panko/sharing/share_test.exs
defmodule Panko.Sharing.ShareTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.Session

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "share-test",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  test "creates a share with auto-generated slug", %{session: session} do
    assert {:ok, share} = Panko.Sharing.create_share(session.id)
    assert share.slug != nil
    assert String.length(share.slug) == 8
    assert share.is_shared == true
    assert share.shared_at != nil
  end

  test "unpublish sets is_shared to false", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, unpublished} = Panko.Sharing.unpublish_share(share)
    assert unpublished.is_shared == false
    assert unpublished.unshared_at != nil
  end

  test "republish restores sharing with same slug", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, unpublished} = Panko.Sharing.unpublish_share(share)
    {:ok, republished} = Panko.Sharing.republish_share(unpublished)
    assert republished.is_shared == true
    assert republished.slug == share.slug
  end

  test "get_share_by_slug finds active share", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    assert {:ok, found} = Panko.Sharing.get_share_by_slug(share.slug)
    assert found.id == share.id
  end

  test "get_share_by_slug returns error for unpublished", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id)
    {:ok, _} = Panko.Sharing.unpublish_share(share)
    assert {:error, _} = Panko.Sharing.get_share_by_slug(share.slug)
  end
end
```

**Step 6: Run tests**

```bash
mix test test/panko/sharing/share_test.exs
```

Expected: All pass.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add Share resource with slug generation and unpublish"
```

---

### Task 18: Share/unpublish UI in SessionsLive and SessionLive

**Files:**
- Modify: `lib/panko_web/live/sessions_live.ex` (add share buttons)
- Modify: `lib/panko_web/live/session_live.ex` (add share button in header)
- Create: `lib/panko_web/components/share_modal.ex`

**Step 1: Create share modal component**

A LiveComponent that handles share creation, shows the URL, and offers an unpublish button. Includes optional expiry picker.

**Step 2: Add share event handlers to SessionsLive and SessionLive**

Handle `"share"` and `"unpublish"` events that call `Panko.Sharing.create_share/1` and `Panko.Sharing.unpublish_share/1`.

**Step 3: Test manually**

```bash
mix phx.server
```

Import a session, click share, verify slug URL is generated. Click unpublish, verify it's removed.

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add share/unpublish UI"
```

---

### Task 19: ShareLive — public share view

**Files:**
- Create: `lib/panko_web/live/share_live.ex`
- Create: `lib/panko_web/components/layouts/public.html.heex`
- Modify: `lib/panko_web/router.ex` (add public scope)
- Test: `test/panko_web/live/share_live_test.exs`

**Step 1: Add public route scope**

```elixir
# In router.ex
scope "/s", PankoWeb do
  pipe_through :browser

  live_session :public, layout: {PankoWeb.Layouts, :public} do
    live "/:slug", ShareLive, :show
  end
end
```

**Step 2: Create public layout**

A minimal layout without navigation — just the content and a "Powered by Panko" footer.

**Step 3: Create ShareLive**

```elixir
# lib/panko_web/live/share_live.ex
defmodule PankoWeb.ShareLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  @impl true
  def mount(%{"slug" => slug}, _session, socket) do
    case Panko.Sharing.get_share_by_slug(slug) do
      {:ok, share} ->
        if expired?(share) do
          {:ok, assign(socket, :error, :expired)}
        else
          session = share.session
          {:ok, assign(socket, share: share, session: session, page_title: session.title || "Shared Session")}
        end

      {:error, _} ->
        {:ok, assign(socket, :error, :not_found)}
    end
  end

  @impl true
  def render(%{error: :not_found} = assigns) do
    ~H"""
    <div class="flex items-center justify-center min-h-screen">
      <div class="text-center">
        <h1 class="text-4xl font-bold mb-4">404</h1>
        <p class="text-base-content/60">This share link is not available.</p>
      </div>
    </div>
    """
  end

  def render(%{error: :expired} = assigns) do
    ~H"""
    <div class="flex items-center justify-center min-h-screen">
      <div class="text-center">
        <h1 class="text-4xl font-bold mb-4">Expired</h1>
        <p class="text-base-content/60">This shared session has expired.</p>
      </div>
    </div>
    """
  end

  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <h1 class="text-2xl font-bold mb-2">{@session.title || "Shared Session"}</h1>
      <p class="text-sm text-base-content/60 mb-6">{@session.project}</p>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>

      <footer class="text-center text-xs text-base-content/40 mt-12 py-4 border-t border-base-300">
        Shared with <a href="https://github.com/jordangarrison/panko" class="link">Panko</a>
      </footer>
    </div>
    """
  end

  defp expired?(%{expires_at: nil}), do: false
  defp expired?(%{expires_at: expires_at}) do
    DateTime.compare(DateTime.utc_now(), expires_at) == :gt
  end
end
```

**Step 4: Write test**

```elixir
# test/panko_web/live/share_live_test.exs
defmodule PankoWeb.ShareLiveTest do
  use PankoWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  setup do
    path = Path.join(["test/fixtures", "simple_session.jsonl"])
    {:ok, session} = Panko.Sessions.import_from_file(path)
    {:ok, share} = Panko.Sharing.create_share(session.id)
    %{session: session, share: share}
  end

  test "renders shared session", %{conn: conn, share: share} do
    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ "List the files"
    assert html =~ "Panko"
  end

  test "shows 404 for invalid slug", %{conn: conn} do
    {:ok, _view, html} = live(conn, ~p"/s/nonexistent")
    assert html =~ "404"
  end

  test "shows expired for expired share", %{conn: conn, share: share} do
    # Manually expire the share
    share
    |> Ash.Changeset.for_update(:update, %{expires_at: ~U[2020-01-01 00:00:00Z]})
    |> Ash.update!()

    {:ok, _view, html} = live(conn, ~p"/s/#{share.slug}")
    assert html =~ "Expired"
  end
end
```

**Step 5: Run tests**

```bash
mix test test/panko_web/live/share_live_test.exs
```

Expected: Pass.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add ShareLive for public share viewing"
```

---

### Task 20: Oban share reaper job

**Files:**
- Create: `lib/panko/sharing/workers/share_reaper.ex`
- Modify: `config/config.exs` (add Oban cron)
- Test: `test/panko/sharing/workers/share_reaper_test.exs`

**Step 1: Create the worker**

```elixir
# lib/panko/sharing/workers/share_reaper.ex
defmodule Panko.Sharing.Workers.ShareReaper do
  use Oban.Worker, queue: :shares

  alias Panko.Sharing.Share

  @impl Oban.Worker
  def perform(_job) do
    now = DateTime.utc_now()

    expired_shares =
      Share
      |> Ash.Query.filter(is_shared == true and not is_nil(expires_at) and expires_at < ^now)
      |> Ash.read!()

    for share <- expired_shares do
      Panko.Sharing.unpublish_share(share)
    end

    :ok
  end
end
```

**Step 2: Add Oban cron to config**

In `config/config.exs`, update the Oban config:

```elixir
config :panko, Oban,
  repo: Panko.Repo,
  queues: [default: 10, shares: 5],
  plugins: [
    {Oban.Plugins.Cron,
     crontab: [
       {"0 * * * *", Panko.Sharing.Workers.ShareReaper}
     ]}
  ]
```

**Step 3: Write test**

```elixir
# test/panko/sharing/workers/share_reaper_test.exs
defmodule Panko.Sharing.Workers.ShareReaperTest do
  use Panko.DataCase, async: true
  use Oban.Testing, repo: Panko.Repo

  alias Panko.Sharing.Workers.ShareReaper

  setup do
    {:ok, session} =
      Panko.Sessions.Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "reaper-test",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  test "deactivates expired shares", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id, %{expires_at: ~U[2020-01-01 00:00:00Z]})
    assert share.is_shared == true

    assert :ok = perform_job(ShareReaper, %{})

    {:error, _} = Panko.Sharing.get_share_by_slug(share.slug)
  end

  test "leaves non-expired shares alone", %{session: session} do
    {:ok, share} = Panko.Sharing.create_share(session.id, %{expires_at: ~U[2099-01-01 00:00:00Z]})

    assert :ok = perform_job(ShareReaper, %{})

    {:ok, found} = Panko.Sharing.get_share_by_slug(share.slug)
    assert found.is_shared == true
  end
end
```

**Step 4: Run test**

```bash
mix test test/panko/sharing/workers/share_reaper_test.exs
```

Expected: Pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Oban share reaper for expired shares"
```

---

## Phase 7: Auth + MCP

### Task 21: API key plug

**Files:**
- Create: `lib/panko_web/plugs/api_key_auth.ex`
- Modify: `lib/panko_web/router.ex` (add to pipeline)
- Test: `test/panko_web/plugs/api_key_auth_test.exs`

**Step 1: Create the plug**

```elixir
# lib/panko_web/plugs/api_key_auth.ex
defmodule PankoWeb.Plugs.ApiKeyAuth do
  @moduledoc """
  Optional API key authentication plug.

  If PANKO_API_KEY is set, requests must provide it via:
  - Authorization: Bearer <key> header
  - ?api_key=<key> query parameter

  If PANKO_API_KEY is not set, all requests pass through.
  """
  import Plug.Conn

  def init(opts), do: opts

  def call(conn, _opts) do
    case Application.get_env(:panko, :api_key) do
      nil -> conn
      "" -> conn
      expected_key -> verify_key(conn, expected_key)
    end
  end

  defp verify_key(conn, expected_key) do
    provided =
      get_bearer_token(conn) ||
        conn.query_params["api_key"] ||
        get_session(conn, :api_key)

    if provided == expected_key do
      conn
    else
      conn
      |> put_resp_content_type("text/plain")
      |> send_resp(401, "Unauthorized")
      |> halt()
    end
  end

  defp get_bearer_token(conn) do
    case get_req_header(conn, "authorization") do
      ["Bearer " <> token] -> token
      _ -> nil
    end
  end
end
```

**Step 2: Add to router pipeline**

```elixir
pipeline :maybe_require_api_key do
  plug PankoWeb.Plugs.ApiKeyAuth
end
```

Add to the default scope:

```elixir
scope "/", PankoWeb do
  pipe_through [:browser, :maybe_require_api_key]
  # ... live routes
end
```

The `/s/` public scope does NOT use this pipeline.

**Step 3: Write test**

```elixir
# test/panko_web/plugs/api_key_auth_test.exs
defmodule PankoWeb.Plugs.ApiKeyAuthTest do
  use PankoWeb.ConnCase, async: true

  alias PankoWeb.Plugs.ApiKeyAuth

  test "passes through when no API key configured", %{conn: conn} do
    Application.put_env(:panko, :api_key, nil)
    conn = ApiKeyAuth.call(conn, [])
    refute conn.halted
  end

  test "blocks when API key configured but not provided", %{conn: conn} do
    Application.put_env(:panko, :api_key, "secret123")
    conn = ApiKeyAuth.call(conn, [])
    assert conn.halted
    assert conn.status == 401
    Application.put_env(:panko, :api_key, nil)
  end

  test "passes with correct bearer token", %{conn: conn} do
    Application.put_env(:panko, :api_key, "secret123")
    conn =
      conn
      |> put_req_header("authorization", "Bearer secret123")
      |> ApiKeyAuth.call([])
    refute conn.halted
    Application.put_env(:panko, :api_key, nil)
  end
end
```

**Step 4: Run test**

```bash
mix test test/panko_web/plugs/api_key_auth_test.exs
```

Expected: Pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add optional API key authentication plug"
```

---

### Task 22: MCP integration setup

**Files:**
- Create: `.mcp.json.example`
- Create: `lib/mix/tasks/panko.gen.mcp.ex`
- Modify: `.gitignore` (ensure .mcp.json is ignored)

**Step 1: Create `.mcp.json.example`**

```json
{
  "mcpServers": {
    "tidewave": {
      "type": "http",
      "url": "http://localhost:PORT/tidewave/mcp"
    }
  }
}
```

**Step 2: Create mix task to generate `.mcp.json`**

```elixir
# lib/mix/tasks/panko.gen.mcp.ex
defmodule Mix.Tasks.Panko.Gen.Mcp do
  @moduledoc """
  Generates .mcp.json with the correct port for the running Phoenix server.

  ## Usage

      mix panko.gen.mcp
      mix panko.gen.mcp --port 4001
  """
  use Mix.Task

  @impl Mix.Task
  def run(args) do
    {opts, _, _} = OptionParser.parse(args, strict: [port: :integer])

    port =
      opts[:port] ||
        get_in(Application.get_all_env(:panko), [PankoWeb.Endpoint, :http, :port]) ||
        4000

    content =
      Jason.encode!(
        %{
          "mcpServers" => %{
            "tidewave" => %{
              "type" => "http",
              "url" => "http://localhost:#{port}/tidewave/mcp"
            }
          }
        },
        pretty: true
      )

    File.write!(".mcp.json", content)
    Mix.shell().info("Generated .mcp.json for port #{port}")
  end
end
```

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: add .mcp.json generation for Tidewave integration"
```

---

### Task 23: Ash usage rules

**Files:**
- Modify: `CLAUDE.md` (update for Elixir/Ash project)

**Step 1: Generate usage rules**

```bash
mix ash_ai.gen.usage_rules .rules ash ash_postgres ash_phoenix ash_ai
```

**Step 2: Update CLAUDE.md**

Replace the Rust-focused instructions with Elixir/Ash instructions. Key sections:
- Project overview (now Elixir/Ash/Phoenix)
- Ash-first development guidance
- Development commands (`mix test`, `mix compile`, `mix phx.server`)
- Nix devshell setup
- Reference to `.rules` file for Ash-specific guidance
- Commit guidelines (same as before)

**Step 3: Commit**

```bash
git add -A
git commit -m "docs: update CLAUDE.md and generate Ash usage rules"
```

---

## Phase 8: Deployment

### Task 24: Dockerfile and Docker Compose

**Files:**
- Create: `Dockerfile`
- Create: `docker-compose.yml`
- Create: `rel/overlays/bin/server`
- Create: `rel/overlays/bin/migrate`
- Create: `lib/panko/release.ex`

**Step 1: Create release module**

```elixir
# lib/panko/release.ex
defmodule Panko.Release do
  @app :panko

  def migrate do
    load_app()
    for repo <- repos() do
      {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :up, all: true))
    end
  end

  def rollback(repo, version) do
    load_app()
    {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :down, to: version))
  end

  defp repos, do: Application.fetch_env!(@app, :ecto_repos)
  defp load_app, do: Application.ensure_all_started(@app)
end
```

**Step 2: Create Dockerfile (multi-stage)**

Standard Phoenix Dockerfile: build stage with Elixir + Node for assets, release stage with slim Debian runtime.

**Step 3: Create `docker-compose.yml`**

As specified in the design doc (Panko + PostgreSQL 18 Alpine).

**Step 4: Create release overlays**

```bash
# rel/overlays/bin/server
#!/bin/sh
bin/panko eval "Panko.Release.migrate()"
bin/panko start

# rel/overlays/bin/migrate
#!/bin/sh
bin/panko eval "Panko.Release.migrate()"
```

**Step 5: Test Docker build**

```bash
docker compose build
docker compose up -d
```

Verify the app starts, migrations run, and http://localhost:4000 is accessible.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add Dockerfile and Docker Compose for self-hosting"
```

---

### Task 25: Nix package and NixOS module

**Files:**
- Create: `nix/package.nix`
- Create: `nix/module.nix`
- Create: `nix/docker.nix`
- Modify: `flake.nix` (add package + module outputs)

**Step 1: Create `nix/package.nix`**

Follow Greenlight's `mixRelease` + `fetchMixDeps` + `fetchNpmDeps` pattern. Will need to generate dependency hashes.

**Step 2: Create `nix/module.nix`**

Follow Greenlight's module pattern with Panko-specific options:
- `sessionWatchPaths`
- `defaultShareExpiry`
- `apiKey` (via LoadCredential)
- `database.createLocally`
- `databaseUrlFile`
- `secretKeyBaseFile`
- nginx reverse proxy option
- systemd security hardening

**Step 3: Create `nix/docker.nix`**

Build a minimal Docker image from the nix-built release.

**Step 4: Update `flake.nix`**

```nix
imports = [
  ./nix/devshell.nix
  ./nix/package.nix
];

flake.nixosModules.default = import ./nix/module.nix inputs.self;
```

**Step 5: Verify nix build**

```bash
nix build .#panko
nix flake show
```

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add nix package and NixOS module"
```

---

### Task 26: Final documentation and cleanup

**Files:**
- Modify: `README.md`
- Verify: All tests pass
- Verify: `mix compile --warnings-as-errors`
- Verify: `mix format --check-formatted`

**Step 1: Update README.md**

Cover:
- What Panko is (web app for AI session viewing/sharing)
- Quick start with Docker Compose
- Quick start with Nix
- Development setup
- Configuration options
- MCP integration

**Step 2: Run full validation**

```bash
mix compile --warnings-as-errors
mix format --check-formatted
mix test
```

All must pass.

**Step 3: Commit**

```bash
git add -A
git commit -m "docs: update README for Elixir rewrite"
```

---

## Dependency Graph

```
Phase 1 (Bootstrap)
  Task 1 → Task 2 → Task 3

Phase 2 (Data Model) - depends on Task 3
  Task 4 → Task 5 → Task 6 → Task 7

Phase 3 (Parser) - depends on Task 4
  Task 8 → Task 9 → Task 10

Phase 4 (Import) - depends on Tasks 7 + 10
  Task 11 → Task 12 → Task 13

Phase 5 (Web) - depends on Task 11
  Task 14 → Task 15 → Task 16

Phase 6 (Sharing) - depends on Task 13
  Task 17 → Task 18 → Task 19 → Task 20

Phase 7 (Auth + MCP) - depends on Task 3
  Task 21 (can run in parallel with Phase 5/6)
  Task 22 (can run in parallel)
  Task 23 (can run in parallel)

Phase 8 (Deployment) - depends on all above
  Task 24 → Task 25 → Task 26
```

## Parallelization Opportunities

Within a phase, tasks are sequential. Across phases:
- **Tasks 21-23** (Auth + MCP) can run in parallel with Phases 5-6
- **Task 16** (Styling) can be started as soon as Task 14 is done
- **Task 20** (Oban reaper) is independent once Task 17 is done
