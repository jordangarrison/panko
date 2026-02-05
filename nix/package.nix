{ inputs, ... }:
{
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
        # Integration tests require fixtures via relative paths that aren't
        # available in the nix sandbox. Unit tests run; integration tests
        # are validated outside of nix via `cargo test`.
        doCheck = false;
      });

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

      defaultPanko = lib.makeOverridable wrappedPanko {};
    in
    {
      packages = {
        default = defaultPanko;
        panko = defaultPanko;
        panko-unwrapped = panko;
      };

      apps.default = {
        type = "app";
        program = "${defaultPanko}/bin/panko";
      };
    };
}
