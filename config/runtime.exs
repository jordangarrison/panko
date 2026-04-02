import Config

config :panko,
  session_watch_paths: System.get_env("PANKO_WATCH_PATHS", Path.expand("~/.claude/projects")),
  api_key: System.get_env("PANKO_API_KEY"),
  default_share_expiry: System.get_env("PANKO_DEFAULT_EXPIRY", "7d"),
  instance_origin_id: System.get_env("PANKO_ORIGIN_ID", "local")

if System.get_env("PHX_SERVER") do
  config :panko, PankoWeb.Endpoint, server: true
end

config :panko, PankoWeb.Endpoint, http: [port: String.to_integer(System.get_env("PORT", "4000"))]

if config_env() == :prod do
  database_url =
    System.get_env("DATABASE_URL") ||
      raise "DATABASE_URL not set"

  config :panko, Panko.Repo,
    url: database_url,
    pool_size: String.to_integer(System.get_env("POOL_SIZE") || "10")

  secret_key_base =
    System.get_env("SECRET_KEY_BASE") ||
      raise "SECRET_KEY_BASE not set"

  host = System.get_env("PHX_HOST") || "localhost"

  config :panko, :dns_cluster_query, System.get_env("DNS_CLUSTER_QUERY")

  config :panko, PankoWeb.Endpoint,
    url: [host: host, port: 443, scheme: "https"],
    http: [
      ip: {0, 0, 0, 0, 0, 0, 0, 0}
    ],
    secret_key_base: secret_key_base

  config :panko, :token_signing_secret,
    System.get_env("PANKO_TOKEN_SIGNING_SECRET") ||
      raise "PANKO_TOKEN_SIGNING_SECRET env var is required in production"
end
