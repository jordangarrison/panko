{ inputs, ... }:
{
  perSystem = { pkgs, system, ... }:
    let
      erlang = pkgs.beam.packages.erlang_28;
      elixir = erlang.elixir;
    in
    {
      devShells.default = pkgs.mkShell {
        buildInputs = [
          elixir
          erlang.erlang
          pkgs.postgresql_18
          pkgs.tailwindcss_4
          pkgs.nodejs
          pkgs.inotify-tools
        ];

        env = {
          MIX_TAILWIND_PATH = "${pkgs.tailwindcss_4}/bin/tailwindcss";
          LANG = "en_US.UTF-8";
          ERL_AFLAGS = "-kernel shell_history enabled";
        };

        shellHook = ''
          mix local.hex --if-missing --force
          mix local.rebar --if-missing --force

          # Local PostgreSQL setup
          export PGDATA="$PWD/.pgdata"
          export PGHOST="$PWD/.pgdata"
          export PGPORT="5432"

          if [ ! -d "$PGDATA" ]; then
            echo "Initializing local PostgreSQL database..."
            initdb --auth=trust --no-locale --encoding=UTF8 -U postgres
            echo "unix_socket_directories = '$PGDATA'" >> "$PGDATA/postgresql.conf"
            echo "listen_addresses = '''" >> "$PGDATA/postgresql.conf"
            echo "port = $PGPORT" >> "$PGDATA/postgresql.conf"
          fi

          echo "panko dev shell loaded"
          echo "Elixir: $(elixir --version | tail -1)"
          echo "PostgreSQL: $(postgres --version)"
          echo ""
          echo "Run 'pg_ctl start -l .pgdata/log' to start PostgreSQL"
          echo "Run 'mix setup' to install deps, create db, and build assets"
          echo "Run 'mix phx.server' to start the dev server"
        '';
      };
    };
}
