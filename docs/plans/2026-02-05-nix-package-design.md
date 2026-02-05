# Panko Nix Package Design

## Goal

Package panko as a Nix flake output so it can be:
- Run ad-hoc via `nix run github:jordangarrison/panko`
- Installed system-wide via NixOS config as a flake input

## Flake Structure

Add `crane` as a flake input. Create `nix/package.nix` alongside existing `nix/devshell.nix`.

### Outputs

- `packages.default` / `packages.panko` — panko binary with default runtime deps
- `overlays.default` — for `pkgs.panko` in NixOS configs

### Overridable Runtime Dependencies

```nix
panko.override {
  withCloudflared = true;   # default: true
  withNgrok = false;        # default: false
  withTailscale = false;    # default: false
  withClipboard = true;     # default: true (wl-copy + xclip, Linux only)
}
```

## Build Strategy

Uses **crane** for two-phase Rust builds:

1. `cargoArtifacts` — dependency-only build, cached across source changes
2. `panko` — final binary using cached artifacts

### Native Build Inputs (compile time)

- `pkg-config`
- `openssl`
- `wayland` + `libxkbcommon` (Linux only, for `arboard` clipboard)

### Runtime Wrapping

`makeWrapper` adds optional tools to the binary's `PATH`:
- **Tunnel providers**: `cloudflared`, `ngrok`, `tailscale` (based on `with*` flags)
- **Clipboard**: `wl-copy` + `xclip` (Linux only, macOS uses native `NSPasteboard`)

### Platform Notes

- `rusqlite` uses `features = ["bundled"]` — no runtime SQLite dep
- macOS clipboard works natively, `withClipboard` is a no-op on Darwin
- Wayland/X11 build deps are Linux-only with Darwin guard

## Integration

### NixOS system config

```nix
# flake.nix inputs:
panko.url = "github:jordangarrison/panko";

# NixOS config:
nixpkgs.overlays = [ panko.overlays.default ];
environment.systemPackages = [ pkgs.panko ];

# With customization:
environment.systemPackages = [
  (pkgs.panko.override { withNgrok = true; })
];
```

### Ad-hoc

```bash
nix run github:jordangarrison/panko
```

## File Changes

- `flake.nix` — add crane input, import `nix/package.nix`
- `nix/package.nix` — new file defining package, overlay, and override logic
- `nix/devshell.nix` — unchanged
