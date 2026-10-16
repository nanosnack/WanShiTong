# NixOS module for the Wan Shi Tong service (`wst serve`).
#
# Media libraries are data sources, normally created in the web UI.
# `dataSources` creates them at startup instead, unless a data source (even a
# deleted one) has the name; after that they are the UI's like any other.
# `libraryPaths` only controls what the service can see: they are bound
# read-only into its sandbox and the unit waits for them to be mounted, as it
# does for `dataSources`' roots.
# Likewise, views are created in the web UI; `viewPaths` only makes their
# directories writable.
self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.wanshitong;
  settingsFormat = pkgs.formats.toml { };

  libraryPaths = cfg.libraryPaths ++ lib.mapAttrsToList (_: source: source.rootPath) cfg.dataSources;

  # `wst` reads config.toml from a directory and only writes one if it is
  # missing, so a store path works as long as the file is always there.
  configDir = pkgs.linkFarm "wanshitong-config" {
    "config.toml" = settingsFormat.generate "config.toml" cfg.settings;
  };
in
{
  options.services.wanshitong = {
    enable = lib.mkEnableOption "Wan Shi Tong, a self-hosted media knowledge system";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "wanshitong.packages.\${system}.default";
      description = "The Wan Shi Tong package to run.";
    };

    ffprobePackage = lib.mkPackageOption pkgs "ffmpeg-headless" { };

    settings = lib.mkOption {
      type = lib.types.submodule {
        freeformType = settingsFormat.type;
        options = {
          host = lib.mkOption {
            type = lib.types.str;
            default = "127.0.0.1";
            description = "Address the HTTP server listens on.";
          };
          port = lib.mkOption {
            type = lib.types.port;
            default = 6765;
            description = "Port the HTTP server listens on.";
          };
          watch_data_sources = lib.mkOption {
            type = lib.types.bool;
            default = true;
            description = ''
              Whether to ingest a data source whenever its files change. Only
              changes made on this machine are seen, which includes those made
              through a file share it serves, but not those made on another
              machine to a network filesystem mounted here.
            '';
          };
          watch_quiet_period_secs = lib.mkOption {
            type = lib.types.ints.positive;
            default = 5;
            description = ''
              How long a data source's files must go unchanged before it's
              ingested, so that files added together, e.g. a season's
              episodes copied one after another, are ingested together. A file
              still being written holds the ingest back whatever this is.
            '';
          };
        };
      };
      default = { };
      description = ''
        Contents of `config.toml`. Secrets such as `tmdb_token` belong in
        {option}`services.wanshitong.environmentFile` instead, since this
        file is world-readable in the Nix store.
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/run/secrets/wanshitong.env";
      description = ''
        File of environment variables for the service. Any config key can be
        set as `WST__<KEY>`, e.g. `WST__TMDB_TOKEN=...` to use your own TMDB
        token instead of the shared one, which is only for noncommercial use.
      '';
    };

    dataSources = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            rootPath = lib.mkOption {
              type = lib.types.path;
              description = "Directory to ingest.";
            };
            followSymlinks = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Whether ingest follows symlinks below the root.";
            };
          };
        }
      );
      default = { };
      example = lib.literalExpression ''
        {
          Movies.rootPath = "/mnt/media/movies";
          TV.rootPath = "/mnt/media/tv";
        }
      '';
      description = ''
        Data sources to create, and start ingesting, when the service starts,
        keyed by name. One is skipped if any data source, even a deleted one,
        already has its name, so changing or removing an entry here doesn't
        change the data source; do that in the web UI. Their roots are bound
        read-only into the sandbox like {option}`services.wanshitong.libraryPaths`.
      '';
    };

    libraryPaths = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
      example = [ "/mnt/media" ];
      description = ''
        Media directories the service may read. They are bound read-only into
        its sandbox and the unit waits for them to be mounted. Add them as
        data sources in the web UI, or in
        {option}`services.wanshitong.dataSources`, to ingest them.
      '';
    };

    viewPaths = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
      example = [ "/srv/views" ];
      description = ''
        Directories the service may write views into. A view's root path can be
        one of these or anywhere below one, so a single entry can hold many
        views. Each entry must already exist, must not be (or be inside) a data
        source's root, and must be writable by the service: it runs as a
        dynamic user, so grant access through {option}`services.wanshitong.viewGroup`.

        View symlinks point at media by absolute path, so whatever reads the
        view (e.g. Jellyfin) must see the library at the same paths.
      '';
    };

    viewGroup = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "jellyfin";
      description = ''
        An extra group for the service, to give it write access to
        {option}`services.wanshitong.viewPaths`.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open {option}`services.wanshitong.settings.port` in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    services.wanshitong.settings.data_sources = lib.mkIf (cfg.dataSources != { }) (
      lib.mapAttrsToList (name: source: {
        inherit name;
        root_path = source.rootPath;
        follow_symlinks = source.followSymlinks;
      }) cfg.dataSources
    );

    systemd.services.wanshitong = {
      description = "Wan Shi Tong media knowledge system";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      unitConfig.RequiresMountsFor = libraryPaths ++ cfg.viewPaths;
      restartTriggers = [ configDir ];

      path = [ cfg.ffprobePackage ];

      # Error reports in the log carry a backtrace, as well as their causes'
      # locations and span trace. The environment file can override it.
      environment.RUST_BACKTRACE = "1";

      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          (lib.getExe cfg.package)
          "serve"
          "--config-dir"
          configDir
          "--data-dir"
          "/var/lib/wanshitong"
          "--cache-dir"
          "/var/cache/wanshitong"
        ];
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;
        Restart = "on-failure";

        DynamicUser = true;
        StateDirectory = "wanshitong";
        CacheDirectory = "wanshitong";
        BindReadOnlyPaths = libraryPaths;
        ReadWritePaths = cfg.viewPaths;
        SupplementaryGroups = lib.optional (cfg.viewGroup != null) cfg.viewGroup;

        # Hardening
        CapabilityBoundingSet = "";
        LockPersonality = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectProc = "invisible";
        ProtectSystem = "strict";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
        UMask = "0077";
      };
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.settings.port ];
  };
}
