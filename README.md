# Panko

Web application for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.). Built with Elixir, Phoenix LiveView, and Ash Framework.

Panko watches your local session files (JSONL), parses conversation blocks, and serves a real-time web UI. You can share sessions publicly via unique slugs with configurable expiry.

## Features

- **Session parsing** -- reads Claude Code JSONL session files with full block structure (human turns, assistant turns, tool use/results, sub-agents)
- **Real-time file watching** -- automatically detects new and updated sessions via filesystem events
- **Live UI** -- Phoenix LiveView pages update in real time as sessions change
- **Sharing** -- publish sessions with unique slugs, set expiry, unpublish/republish at will
- **Optional API key auth** -- protect the dashboard behind an API key
- **Automatic cleanup** -- Oban cron job reaps expired shares hourly

## Quick Start

### Docker Compose

The fastest way to run Panko:

```bash
git clone https://github.com/jordangarrison/panko
cd panko

# Start PostgreSQL and Panko
docker compose up -d

# Open http://localhost:4000
```

The default `docker-compose.yml` starts a PostgreSQL database and the Panko web server. For production, replace the `SECRET_KEY_BASE` with a real secret:

```bash
mix phx.gen.secret
```

### Nix

```bash
# Run directly
nix run github:jordangarrison/panko

# Or add to a flake
nix flake show github:jordangarrison/panko
```

#### NixOS Module

Add Panko as a flake input and import the NixOS module:

```nix
# flake.nix
{
  inputs.panko.url = "github:jordangarrison/panko";

  outputs = { self, nixpkgs, panko, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        panko.nixosModules.default
        {
          services.panko = {
            enable = true;
            host = "panko.example.com";
            secretKeyBaseFile = "/run/secrets/panko-secret-key-base";
            database.createLocally = true;
            sessionWatchPaths = [ "/home/user/.claude/projects" ];
          };
        }
      ];
    };
  };
}
```

The module handles PostgreSQL setup, systemd service, optional nginx reverse proxy with ACME, and security hardening out of the box.

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | (required in prod) | PostgreSQL connection string (`ecto://user:pass@host/db`) |
| `SECRET_KEY_BASE` | (required in prod) | Phoenix secret key (generate with `mix phx.gen.secret`) |
| `PHX_HOST` | `localhost` | Public hostname for URL generation |
| `PORT` | `4000` | HTTP port |
| `PANKO_WATCH_PATHS` | `~/.claude/projects` | Colon-separated paths to watch for session files |
| `PANKO_API_KEY` | (none) | When set, requires `?api_key=` param or `x-api-key` header to access the dashboard |
| `PANKO_DEFAULT_EXPIRY` | `7d` | Default share expiry duration |
| `PANKO_ORIGIN_ID` | `local` | Unique identifier for this Panko instance |

## Development

### Prerequisites

- Elixir >= 1.15
- PostgreSQL
- Node.js (for asset tooling)

Or use Nix:

```bash
# With direnv (recommended)
direnv allow

# Or manually
nix develop
```

### Setup

```bash
# Install dependencies, create database, build assets
mix setup

# Start the dev server
mix phx.server

# Open http://localhost:4000
```

### Running Tests

```bash
mix test
```

### Full Validation

```bash
mix compile --warnings-as-errors
mix format --check-formatted
mix test
```

Or use the precommit alias which runs all checks:

```bash
mix precommit
```

### MCP Integration

In development, Panko includes [Tidewave](https://github.com/tidewave-ai/tidewave) for MCP (Model Context Protocol) integration. This lets AI coding agents interact with the running application for debugging and development. Tidewave is only included in the `:dev` environment and is not part of production builds.

## Project Structure

```
lib/
  panko/
    sessions/           # Ash domain: Session, Block, SubAgent resources
      parsers/           # JSONL session file parsers
      session_watcher.ex # GenServer for filesystem watching
    sharing/             # Ash domain: Share resource
      workers/           # Oban workers (share reaper)
    application.ex       # OTP application supervisor
    repo.ex              # Ecto repo
  panko_web/
    live/                # LiveView pages
      sessions_live.ex   # Session list (dashboard)
      session_live.ex    # Session detail with block rendering
      share_live.ex      # Public share view
    components/          # Phoenix components
    plugs/               # API key auth plug
    router.ex            # Route definitions
config/
  config.exs             # Compile-time config
  dev.exs                # Dev environment
  prod.exs               # Production environment
  runtime.exs            # Runtime config (env vars)
nix/
  devshell.nix           # Nix dev shell
  package.nix            # Nix package definition
  module.nix             # NixOS module
```

## Routes

| Path | Description |
|------|-------------|
| `/` | Session list (protected by API key if configured) |
| `/sessions/:id` | Session detail view |
| `/s/:slug` | Public share view (no auth required) |

## License

MIT
