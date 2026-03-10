# nix/package.nix
#
# Panko Elixir/Phoenix package built with mixRelease.
#
# STATUS: Scaffolding only — placeholder hashes need to be computed.
#
# To make this buildable:
#   1. Run `nix build` and let the mix deps fetch fail
#   2. Copy the correct hash from the error message into mixFodDeps.hash
#   3. Repeat for any other hash mismatches
#
# Alternatively, use mix2nix to generate a deps.nix lockfile.
{
  lib,
  beamPackages,
  tailwindcss_4,
  mixRelease ? beamPackages.mixRelease,
  fetchMixDeps ? beamPackages.fetchMixDeps,
}:

let
  pname = "panko";
  version = "0.1.0";
  src = ./..;

  mixFodDeps = fetchMixDeps {
    pname = "${pname}-mix-deps";
    inherit version src;
    # TODO: Compute correct hash by running `nix build` and copying the expected hash
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
in
mixRelease {
  inherit pname version src mixFodDeps;

  # Tailwind is configured via MIX_TAILWIND_PATH in config.exs
  MIX_TAILWIND_PATH = "${tailwindcss_4}/bin/tailwindcss";

  postBuild = ''
    mix do deps.loadpaths --no-deps-check, assets.deploy
  '';
}
