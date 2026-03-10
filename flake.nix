{
  description = "panko - web app for viewing and sharing AI coding agent sessions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      imports = [
        ./nix/devshell.nix
      ];

      perSystem = { pkgs, system, ... }:
        let
          erlang = pkgs.beam.packages.erlang_28;
        in
        {
          packages = {
            panko = pkgs.callPackage ./nix/package.nix {
              beamPackages = erlang;
            };
            default = self.packages.${system}.panko;
          };
        };

      flake = {
        nixosModules.default = import ./nix/module.nix self;
      };
    };
}
