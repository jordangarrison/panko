# nix/package.nix
#
# Panko Elixir/Phoenix package built with mixRelease.
#
# Builds the panko Elixir release with esbuild and tailwind asset compilation.
{
  lib,
  beamPackages,
  esbuild,
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
    hash = "sha256-LL6lNNOKJ2/+OR3/JPSqdJLVgWH3piwzGzG9QwIbuAg=";
  };
in
mixRelease {
  inherit pname version src mixFodDeps;

  # Asset tools configured via env vars, read by config.exs
  MIX_ESBUILD_PATH = "${esbuild}/bin/esbuild";
  MIX_TAILWIND_PATH = "${tailwindcss_4}/bin/tailwindcss";

  postBuild = ''
    mix do deps.loadpaths --no-deps-check, assets.deploy
  '';
}
