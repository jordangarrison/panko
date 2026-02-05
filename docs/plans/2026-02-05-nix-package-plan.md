# Nix Package Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Package panko as a Nix flake output with crane, overridable runtime deps, and an overlay for NixOS integration.

**Architecture:** Two-phase crane build (deps-only cache + final binary). Runtime tools (cloudflared, clipboard) wrapped into PATH via makeWrapper with overridable flags. Overlay export for system-wide installation.

**Tech Stack:** Nix, flake-parts, crane, rust-overlay, makeWrapper

---

### Task 1: Add crane flake input

**Files:**
- Modify: `flake.nix`

**Step 1: Update flake.nix inputs to add crane**

Add the crane input to `flake.nix` and pass it through to flake-parts:

```nix
{
  description = "panko - CLI tool for viewing and sharing AI coding agent sessions";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports = [
        ./nix/devshell.nix
        ./nix/package.nix
      ];
    };
}
```

**Step 2: Update flake.lock**

Run: `nix flake update crane`
Expected: flake.lock updated with crane entry, no errors.

**Step 3: Commit**

```bash
git add flake.nix flake.lock
git commit -m "build: add crane flake input"
```

---

### Task 2: Create the package module (bare build, no wrapping)

**Files:**
- Create: `nix/package.nix`

**Step 1: Write `nix/package.nix` with crane build**

This is the core package definition. First iteration: just get it compiling without runtime wrapping.

```nix
{ inputs, ... }:
{
  perSystem = { config, self', inputs', pkgs, system, lib, ... }:
    let
      rustToolchain = pkgs.rust-bin.stable.latest.default;
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

      # Source filtering — include Rust source, templates, and assets
      src = lib.cleanSourceWith {
        src = inputs.self;
        filter = path: type:
          (craneLib.filterCargoSources path type)
          || (builtins.match ".*\\.html$" path != null)
          || (builtins.match ".*\\.css$" path != null)
          || (builtins.match ".*\\.js$" path != null);
      };

      commonArgs = {
        inherit src;
        strictDeps = true;
        pname = "panko";

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        buildInputs = with pkgs; [
          openssl
        ] ++ lib.optionals pkgs.stdenv.isLinux [
          wayland
          libxkbcommon
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        pname = "panko-deps";
      });

      panko = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
      });
    in
    {
      packages = {
        default = panko;
        panko = panko;
      };
    };
}
```

**Step 2: Verify the bare build compiles**

Run: `nix build .#panko`
Expected: Builds successfully. Binary at `./result/bin/panko`.

**Step 3: Verify the binary runs**

Run: `./result/bin/panko --help`
Expected: Shows panko help text with available subcommands.

**Step 4: Commit**

```bash
git add nix/package.nix
git commit -m "build: add crane-based nix package"
```

---

### Task 3: Add runtime wrapping with overridable flags

**Files:**
- Modify: `nix/package.nix`

**Step 1: Add makeWrapper and overridable runtime deps**

Update `nix/package.nix` to wrap the panko binary with optional runtime tools. Replace the `panko` and `packages` definitions with a wrapped version:

After the `panko = craneLib.buildPackage ...` definition, add:

```nix
      wrappedPanko = {
        withCloudflared ? true,
        withNgrok ? false,
        withTailscale ? false,
        withClipboard ? true,
      }:
        let
          runtimeDeps = lib.flatten [
            (lib.optional withCloudflared pkgs.cloudflared)
            (lib.optional withNgrok pkgs.ngrok)
            (lib.optional withTailscale pkgs.tailscale)
            (lib.optionals (withClipboard && pkgs.stdenv.isLinux) [
              pkgs.wl-clipboard
              pkgs.xclip
            ])
          ];
        in
        if runtimeDeps == [] then panko
        else pkgs.symlinkJoin {
          name = "panko-${panko.version or "0.1.0"}";
          paths = [ panko ];
          nativeBuildInputs = [ pkgs.makeWrapper ];
          postBuild = ''
            wrapProgram $out/bin/panko \
              --prefix PATH : ${lib.makeBinPath runtimeDeps}
          '';
        };

      defaultPanko = wrappedPanko {};
```

Then update the packages output:

```nix
      packages = {
        default = defaultPanko;
        panko = defaultPanko;
        panko-unwrapped = panko;
      };
```

The `override` pattern is provided by making `defaultPanko` overridable. Users call:
```nix
panko.packages.${system}.panko.override { withNgrok = true; }
```

Wait — `symlinkJoin` doesn't natively support `.override`. Instead, we'll use `lib.makeOverridable`:

```nix
      defaultPanko = lib.makeOverridable wrappedPanko {};
```

This gives `defaultPanko.override { withNgrok = true; }` for free.

**Step 2: Verify wrapped build**

Run: `nix build .#panko`
Expected: Builds successfully. Binary at `./result/bin/panko`.

**Step 3: Verify cloudflared is in PATH**

Run: `./result/bin/panko share --help` (just verify it can find cloudflared concept — the binary should have cloudflared on its PATH)

Alternatively, check the wrapper script:
Run: `cat ./result/bin/panko | head -5`
Expected: Shows a bash wrapper script with PATH containing cloudflared.

**Step 4: Verify unwrapped variant exists**

Run: `nix build .#panko-unwrapped && ./result/bin/panko --help`
Expected: Builds and runs, no wrapper script.

**Step 5: Commit**

```bash
git add nix/package.nix
git commit -m "build: add overridable runtime dependency wrapping"
```

---

### Task 4: Add overlay export

**Files:**
- Modify: `nix/package.nix`

**Step 1: Add overlay to the flake-parts module**

Add a `flake` output (not `perSystem`) to `nix/package.nix` for the overlay. Add this block alongside the `perSystem` block:

```nix
  flake = {
    overlays.default = final: prev: {
      panko = final.callPackage (
        { lib, stdenv, cloudflared, ngrok, tailscale, wl-clipboard, xclip, makeWrapper }:
        let
          system = final.stdenv.hostPlatform.system;
          pankoPackages = inputs.self.packages.${system};
        in
        pankoPackages.default
      ) {};
    };
  };
```

**Step 2: Verify overlay evaluates**

Run: `nix flake show`
Expected: Shows `overlays.default` in the output alongside `packages.*`.

**Step 3: Commit**

```bash
git add nix/package.nix
git commit -m "build: add default overlay for NixOS integration"
```

---

### Task 5: Add `nix run` app output

**Files:**
- Modify: `nix/package.nix`

**Step 1: Add apps output to perSystem**

Inside the `perSystem` block, add:

```nix
      apps.default = {
        type = "app";
        program = "${defaultPanko}/bin/panko";
      };
```

**Step 2: Verify `nix run` works**

Run: `nix run . -- --help`
Expected: Shows panko help output.

**Step 3: Commit**

```bash
git add nix/package.nix
git commit -m "build: add nix run app output"
```

---

### Task 6: Smoke test the full setup

**Step 1: Clean build from scratch**

Run: `nix build .#panko --rebuild`
Expected: Full build succeeds.

**Step 2: Test nix run**

Run: `nix run . -- --help`
Expected: Help text.

**Step 3: Test nix run with a real session (if available)**

Run: `nix run . -- tui`
Expected: TUI launches (can quit with `q`).

**Step 4: Verify flake outputs**

Run: `nix flake show`
Expected output includes:
```
├───apps
│   ├───aarch64-darwin
│   │   └───default
│   ├───aarch64-linux
│   │   └───default
│   ├───x86_64-darwin
│   │   └───default
│   └───x86_64-linux
│       └───default
├───overlays
│   └───default
└───packages
    ├───aarch64-darwin
    │   ├───default
    │   ├───panko
    │   └───panko-unwrapped
    ... (same for other systems)
```

**Step 5: Verify devShell still works**

Run: `nix develop -c bash -c "rustc --version"`
Expected: Prints rustc version, devshell unchanged.

**Step 6: Commit (if any fixups needed)**

```bash
git add -A
git commit -m "build: fixup nix package smoke test issues"
```

Only commit if changes were needed. If everything passed, no commit needed.

---

### Task 7: Final commit and PR prep

**Step 1: Verify all changes**

Run: `git log --oneline main..HEAD`
Expected: Shows commits from tasks 1-6.

**Step 2: Verify clean state**

Run: `git status`
Expected: Clean working tree.

**Step 3: Push branch**

Run: `pu`
Expected: Branch pushed to origin.
