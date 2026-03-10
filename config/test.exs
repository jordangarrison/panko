import Config

config :panko, Panko.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "panko_test#{System.get_env("MIX_TEST_PARTITION")}",
  socket_dir: System.get_env("PGDATA"),
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: System.schedulers_online() * 2

config :panko, PankoWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: String.duplicate("test", 16),
  server: false

config :panko, Oban, testing: :manual

config :logger, level: :warning

config :phoenix, :plug_init_mode, :runtime

config :phoenix_live_view,
  enable_expensive_runtime_checks: true

config :phoenix,
  sort_verified_routes_query_params: true
