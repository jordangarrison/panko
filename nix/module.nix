# nix/module.nix
#
# NixOS module for running Panko as a systemd service.
#
# Usage in a NixOS flake configuration:
#
#   {
#     inputs.panko.url = "github:jordangarrison/panko";
#
#     outputs = { self, nixpkgs, panko, ... }: {
#       nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
#         modules = [
#           panko.nixosModules.default
#           {
#             services.panko = {
#               enable = true;
#               host = "panko.example.com";
#               secretKeyBaseFile = "/run/secrets/panko-secret-key-base";
#               tokenSigningSecretFile = "/run/secrets/panko-token-signing-secret";
#               database.urlFile = "/run/secrets/panko-database-url";
#               sessionWatchPaths = [ "/home/user/.claude/projects" ];
#             };
#           }
#         ];
#       };
#     };
#   }
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.panko;
in
{
  options.services.panko = {
    enable = lib.mkEnableOption "Panko session viewer";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "self.packages.\${pkgs.system}.default";
      description = "The Panko package to use.";
    };

    host = lib.mkOption {
      type = lib.types.str;
      default = "localhost";
      example = "panko.example.com";
      description = "Public hostname for the application (PHX_HOST).";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 4000;
      description = "Port the application listens on.";
    };

    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = "Address to bind the HTTP server to.";
    };

    sessionWatchPaths = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/home/user/.claude/projects" "/home/user/.codex/sessions" ];
      description = "Paths to watch for AI coding agent session files.";
    };

    defaultShareExpiry = lib.mkOption {
      type = lib.types.str;
      default = "7d";
      description = "Default expiry duration for shared sessions.";
    };

    instanceOriginId = lib.mkOption {
      type = lib.types.str;
      default = "local";
      description = "Unique identifier for this Panko instance origin.";
    };

    dnsClusterQuery = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Optional DNS cluster discovery query for distributed deployments.";
    };

    # Database configuration
    database = {
      createLocally = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to create a local PostgreSQL database.";
      };

      urlFile = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = ''
          Absolute path to a file containing the DATABASE_URL.
          Must NOT be a Nix store path.
          Required when database.createLocally is false.
        '';
      };
    };

    # Secrets — per-file via LoadCredential
    secretKeyBaseFile = lib.mkOption {
      type = lib.types.str;
      description = ''
        Absolute path to a file containing the Phoenix SECRET_KEY_BASE.
        Must NOT be a Nix store path.
        Generate with: mix phx.gen.secret or openssl rand -base64 64
        Consider using sops-nix or agenix for secret management.
      '';
    };

    tokenSigningSecretFile = lib.mkOption {
      type = lib.types.str;
      description = ''
        Absolute path to a file containing the PANKO_TOKEN_SIGNING_SECRET.
        Must NOT be a Nix store path.
        Used by Ash Authentication for signing session tokens.
        Generate with: openssl rand -base64 48
        Consider using sops-nix or agenix for secret management.
      '';
    };

    apiKeyFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Absolute path to a file containing the PANKO_API_KEY.
        Must NOT be a Nix store path.
      '';
    };

    # Catch-all environment file for additional secrets/overrides
    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Absolute path to an environment file as defined in {manpage}`systemd.exec(5)`.
        Must NOT be a Nix store path.
        Secrets may be passed to the service without adding them to the
        world-readable Nix store. Format: KEY=VALUE, one per line.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the firewall for the service port.";
    };

    nginx = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether to configure an nginx virtualhost for Panko.";
      };

      enableACME = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to enable ACME (Let's Encrypt) for the nginx virtualhost.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.database.createLocally || cfg.database.urlFile != null;
        message = "services.panko: either database.createLocally must be true or database.urlFile must be set.";
      }
      {
        assertion = !(cfg.nginx.enable && cfg.host == "localhost");
        message = "services.panko: nginx is enabled but host is 'localhost'. ACME certificate provisioning will fail. Set a real public hostname.";
      }
      {
        assertion = !(cfg.nginx.enableACME && (builtins.match "^[0-9.:]+$" cfg.host != null || lib.hasSuffix ".local" cfg.host));
        message = "services.panko: ACME is enabled but host appears to be an IP address or .local domain. ACME requires a public DNS name.";
      }
      {
        assertion = !(cfg.openFirewall && cfg.nginx.enable);
        message = "services.panko: openFirewall and nginx are both enabled. openFirewall exposes port ${toString cfg.port} directly. You probably want to open ports 80/443 via nginx instead.";
      }
    ];

    # System user
    users.users.panko = {
      isSystemUser = true;
      group = "panko";
    };
    users.groups.panko = { };

    # PostgreSQL
    services.postgresql = lib.mkIf cfg.database.createLocally {
      enable = true;
      ensureDatabases = [ "panko" ];
      ensureUsers = [
        {
          name = "panko";
          ensureDBOwnership = true;
        }
      ];
    };

    # Firewall
    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];

    # Systemd service
    systemd.services.panko = {
      description = "Panko - AI Coding Agent Session Viewer";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after =
        [ "network-online.target" ]
        ++ lib.optionals cfg.database.createLocally [ "postgresql.service" ];

      environment =
        {
          PORT = toString cfg.port;
          PHX_HOST = cfg.host;
          PHX_SERVER = "true";
          PHX_SCHEME = if cfg.nginx.enable then "https" else "http";
          PHX_URL_PORT = toString (if cfg.nginx.enable then 443 else cfg.port);
          PANKO_DEFAULT_EXPIRY = cfg.defaultShareExpiry;
          PANKO_ORIGIN_ID = cfg.instanceOriginId;
          RELEASE_DISTRIBUTION = "none";
          ERL_EPMD_ADDRESS = "127.0.0.1";
          HOME = "/var/lib/panko";
          RELEASE_TMP = "/var/lib/panko/tmp";
          LANG = "en_US.UTF-8";
        }
        // lib.optionalAttrs (cfg.sessionWatchPaths != [ ]) {
          PANKO_WATCH_PATHS = lib.concatStringsSep ":" cfg.sessionWatchPaths;
        }
        // lib.optionalAttrs (cfg.dnsClusterQuery != null) {
          DNS_CLUSTER_QUERY = cfg.dnsClusterQuery;
        };

      script = ''
        # Persist release cookie so `bin/panko remote` works across restarts
        COOKIE_FILE="/var/lib/panko/.erlang.cookie"
        if [ ! -f "$COOKIE_FILE" ]; then
          tr -dc A-Za-z0-9 < /dev/urandom | head -c 20 > "$COOKIE_FILE"
          chmod 400 "$COOKIE_FILE"
        fi
        export RELEASE_COOKIE="$(< "$COOKIE_FILE")"

        # Load secrets from systemd credentials
        export SECRET_KEY_BASE="$(< $CREDENTIALS_DIRECTORY/SECRET_KEY_BASE)"

        ${lib.optionalString (cfg.database.urlFile != null) ''
          export DATABASE_URL="$(< $CREDENTIALS_DIRECTORY/DATABASE_URL)"
        ''}
        ${lib.optionalString cfg.database.createLocally ''
          export DATABASE_URL="ecto://panko@localhost/panko"
        ''}
        export PANKO_TOKEN_SIGNING_SECRET="$(< $CREDENTIALS_DIRECTORY/PANKO_TOKEN_SIGNING_SECRET)"

        ${lib.optionalString (cfg.apiKeyFile != null) ''
          export PANKO_API_KEY="$(< $CREDENTIALS_DIRECTORY/PANKO_API_KEY)"
        ''}

        exec ${cfg.package}/bin/server
      '';

      serviceConfig = {
        Type = "exec";
        User = "panko";
        Group = "panko";
        StateDirectory = "panko";
        RuntimeDirectory = "panko";
        Restart = "on-failure";
        RestartSec = 5;

        # Load secrets via systemd credentials
        LoadCredential =
          [
            "SECRET_KEY_BASE:${cfg.secretKeyBaseFile}"
            "PANKO_TOKEN_SIGNING_SECRET:${cfg.tokenSigningSecretFile}"
          ]
          ++ lib.optionals (cfg.database.urlFile != null) [
            "DATABASE_URL:${cfg.database.urlFile}"
          ]
          ++ lib.optionals (cfg.apiKeyFile != null) [
            "PANKO_API_KEY:${cfg.apiKeyFile}"
          ];

        # Optional catch-all environment file
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;

        # Security hardening
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
          "AF_INET6"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        ProtectControlGroups = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        LockPersonality = true;
        CapabilityBoundingSet = "";
        ProtectKernelLogs = true;
        UMask = "0077";
      };
    };

    # Optional nginx reverse proxy
    services.nginx = lib.mkIf cfg.nginx.enable (
      let
        urlAddr =
          if lib.hasInfix ":" cfg.listenAddress then "[${cfg.listenAddress}]" else cfg.listenAddress;
      in
      {
        enable = true;
        recommendedProxySettings = true;
        recommendedTlsSettings = true;
        recommendedOptimisation = true;
        recommendedGzipSettings = true;

        virtualHosts.${cfg.host} = {
          forceSSL = true;
          enableACME = cfg.nginx.enableACME;

          locations."/" = {
            proxyPass = "http://${urlAddr}:${toString cfg.port}";
            proxyWebsockets = true;
          };
        };
      }
    );
  };
}
