# This file is responsible for configuring your application
# and its dependencies with the aid of the Config module.
#
# This configuration file is loaded before any dependency and
# is restricted to this project.

# General application configuration
import Config

config :panko,
  generators: [timestamp_type: :utc_datetime]

# Ash domains
config :panko, ash_domains: [Panko.Sessions, Panko.Sharing, Panko.Accounts]

# Repo
config :panko, ecto_repos: [Panko.Repo]

# Oban
config :panko, Oban,
  repo: Panko.Repo,
  queues: [default: 10, shares: 5],
  plugins: [
    {Oban.Plugins.Cron,
     crontab: [
       {"0 * * * *", Panko.Sharing.Workers.ShareReaper}
     ]}
  ]

# Configure the endpoint
config :panko, PankoWeb.Endpoint,
  url: [host: "localhost"],
  adapter: Bandit.PhoenixAdapter,
  render_errors: [
    formats: [html: PankoWeb.ErrorHTML, json: PankoWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: Panko.PubSub,
  live_view: [signing_salt: "aJo/QBvS"]

# Configure esbuild (the version is required)
# On NixOS, use the nix-provided binary via MIX_ESBUILD_PATH env var
config :esbuild,
  version: "0.25.4",
  path: System.get_env("MIX_ESBUILD_PATH"),
  panko: [
    args:
      ~w(js/app.js --bundle --target=es2022 --outdir=../priv/static/assets/js --external:/fonts/* --external:/images/* --alias:@=.),
    cd: Path.expand("../assets", __DIR__),
    env: %{"NODE_PATH" => [Path.expand("../deps", __DIR__), Mix.Project.build_path()]}
  ]

# Configure tailwind
# On NixOS, use the nix-provided binary via MIX_TAILWIND_PATH env var
config :tailwind,
  version: "4.2.1",
  path: System.get_env("MIX_TAILWIND_PATH"),
  panko: [
    args: ~w(
      --input=assets/css/app.css
      --output=priv/static/assets/css/app.css
    ),
    cd: Path.expand("..", __DIR__)
  ]

# Configure Elixir's Logger
config :logger, :default_formatter,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id]

# Use Jason for JSON parsing in Phoenix
config :phoenix, :json_library, Jason

# Import environment specific config. This must remain at the bottom
# of this file so it overrides the configuration defined above.
import_config "#{config_env()}.exs"
