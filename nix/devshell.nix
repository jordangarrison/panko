{ inputs, ... }:
{
  perSystem = { config, self', inputs', pkgs, system, ... }:
    let
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "rust-src" "rust-analyzer" ];
      };
    in
    {
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ inputs.rust-overlay.overlays.default ];
        config.allowUnfree = true;
      };

      devShells.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          # Rust toolchain
          rustToolchain
          pkg-config
          openssl

          # Tunnel providers for testing
          cloudflared
          ngrok
          # tailscale already on system

          # Dev tools
          cargo-watch
          cargo-edit
        ];

        RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

        shellHook = ''
          echo "panko dev shell loaded"
          echo "Rust: $(rustc --version)"
        '';
      };
    };
}
