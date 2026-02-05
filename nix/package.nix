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
        # Integration tests require fixtures via relative paths that aren't
        # available in the nix sandbox. Unit tests run; integration tests
        # are validated outside of nix via `cargo test`.
        doCheck = false;
      });
    in
    {
      packages = {
        default = panko;
        panko = panko;
      };
    };
}
