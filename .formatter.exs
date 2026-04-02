[
  import_deps: [
    :ash,
    :ash_postgres,
    :ash_phoenix,
    :ash_authentication,
    :ash_authentication_phoenix,
    :phoenix
  ],
  plugins: [Spark.Formatter, Phoenix.LiveView.HTMLFormatter],
  inputs: ["*.{heex,ex,exs}", "{config,lib,test}/**/*.{heex,ex,exs}"]
]
