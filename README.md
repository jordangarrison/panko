# Panko

CLI tool for viewing and sharing AI coding agent sessions (Claude Code, Codex, etc.)

## Installation

### From crates.io

```bash
cargo install panko
```

### From source

```bash
git clone https://github.com/jordangarrison/panko
cd panko
cargo install --path .
```

## Usage

### View a session locally

```bash
# View a Claude Code session
panko view ~/.claude/projects/myproject/session.jsonl

# Opens a web viewer in your browser
```

### Share a session

```bash
# Share via tunnel (interactive provider selection)
panko share ~/.claude/projects/myproject/session.jsonl

# Share with a specific provider
panko share --provider cloudflare session.jsonl
panko share --provider ngrok session.jsonl
panko share --provider tailscale session.jsonl
```

### Configuration

```bash
# Interactive configuration
panko config

# Set default tunnel provider
panko config set tunnel.provider cloudflare

# Show current configuration
panko config show
```

## Supported Tunnel Providers

| Provider | Auth Required | Notes |
|----------|---------------|-------|
| Cloudflare | No | Uses `cloudflared` quick tunnels |
| ngrok | Yes | Requires auth token |
| Tailscale | Yes | Requires Tailscale Serve (beta) |

## Development

This project uses Nix flakes for reproducible development environments.

```bash
# Enter dev shell (with direnv)
direnv allow

# Or manually
nix develop

# Build
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy
```

## License

MIT
