{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.sottomano;
  themeCfg = config.services.umbra or null;
  tomlFormat = pkgs.formats.toml { };

  modeToPalette = p: {
    surface = p.base00;
    surface_raised = p.base02;
    bar_foreground = p.base05;
    secondary_foreground = p.base04;
    inactive = p.base04;
    dim = p.base03;
    frame_outline = p.base03;
    urgent = p.base08;
    warning = p.base0A;
    focus = p.base0C;
    success = p.base0B;
    info = p.base0D;
  };

  variantToml =
    modePalette:
    tomlFormat.generate "sottomano.toml" (
      lib.recursiveUpdate cfg.settings {
        theme.palette = modeToPalette modePalette // (cfg.settings.theme.palette or { });
      }
    );
in
{
  options.services.sottomano = {
    enable = lib.mkEnableOption "sottomano bar";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "sottomano.packages.\${system}.default";
      description = "The sottomano package to install.";
    };

    systemdUnit = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to create a systemd user service that starts sottomano with
        graphical-session.target. Disable to spawn sottomano from a
        compositor-specific hook instead.
      '';
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      description = ''
        Contents of ~/.config/sottomano.toml, written verbatim. The running bar
        watches the file and applies changes in place. `sottomano
        default-config` prints every key with its default value.
      '';
    };

    secretFiles = {
      "openrouter-management-key" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file containing an OpenRouter **management** key as plain
          text. A regular API key is not enough: the usage widget calls
          `/analytics/query`, which only management keys may access. Symlinked
          into ~/.config/sottomano/, where the bar looks for it. When unset, the
          OpenRouter pill is hidden.
        '';
      };

      "claude-code-oauth" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to the Claude Code OAuth credentials JSON (the
          `~/.claude/.credentials.json` blob with `claudeAiOauth.accessToken`).
          Drives the Claude subscription gauge; on API-key machines (no OAuth
          token) it instead surfaces `ccusage` daily spend. Symlinked into
          ~/.config/sottomano/. When unset, the Claude pills are hidden.
        '';
      };

      "gh-auth-token" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file holding a bare `gho_` GitHub OAuth token, such as the
          output of `gh auth token` after `gh auth login` stored as an agenix
          secret. The Copilot usage pills read it and call
          `/copilot_internal/user` directly.
          Symlinked into ~/.config/sottomano/. When unset, the Copilot quota pills
          are hidden (the local-`ccusage` Copilot pill needs no token and still
          shows if `ccusage` is installed).
        '';
      };

      "unsplash-access-key" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file holding an Unsplash **access key** (the Client-ID of a
          registered application at https://unsplash.com/developers) as plain
          text. Not the Unsplash secret key, which only the OAuth flow uses and
          the bar never sends. The wallpaper pill reads it and calls
          `/photos/{id}` to turn the Unsplash photo id embedded in the current
          wallpaper's filename into a caption, photographer and place, then looks
          that place up on the Wikipedia edition matching the session locale.
          Symlinked into ~/.config/sottomano/. The pill needs no configuration: it
          shows when this key and a wallpaper daemon (`awww` or `swww`, tried in
          that order) are both present, and hides otherwise.
        '';
      };

      "caldav-password" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file holding the CalDAV password of the account in
          `settings.widget_groups.caldav` as plain text. Prefer an app password
          where the server offers one (Nextcloud: Personal settings → Security →
          Devices & sessions). The calendar group discovers the calendars through
          `/.well-known/caldav` and shows open tasks and upcoming events.
          Symlinked into ~/.config/sottomano/. When unset, or when `server_url`
          or `user` is empty, the group is hidden.
        '';
      };
    };

    umbra.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Runtime dark/light palette switching via the umbra app registry.
        Requires services.umbra (from umbra.homeModules) in the same
        home-manager config: registers services.umbra.apps.sottomano with
        dark/light palette variants derived from services.umbra.palettes.*. On
        a mode switch umbra re-points ~/.config/sottomano.toml and the running
        bar reloads it in place. Set false for a standalone
        install with no umbra — sottomano then falls back to the static
        palette in settings.
      '';
    };

    windowAgents.kitty.enable = lib.mkEnableOption ''
      kitty's socket-only remote control (at `mkDefault` priority, so an
      existing kitty config wins), which the bar's kitty window agent uses to
      tell apart kitty OS windows sharing one `--single-instance` process
    '';

  };

  config = lib.mkIf cfg.enable {
    programs.kitty.settings = lib.mkIf cfg.windowAgents.kitty.enable {
      allow_remote_control = lib.mkDefault "socket-only";
      listen_on = lib.mkDefault "unix:$XDG_RUNTIME_DIR/kitty.sock";
    };

    assertions = lib.optionals cfg.umbra.enable [
      {
        assertion = themeCfg != null && themeCfg ? palettes;
        message = ''
          services.sottomano.umbra.enable requires services.umbra (provided by
          umbra.homeModules.default/schema). Import it in the same home-manager
          config, or set services.sottomano.umbra.enable = false.
        '';
      }
    ];

    home.packages = [ cfg.package ];

    xdg.configFile = {
      "sottomano.toml" = lib.mkIf (!cfg.umbra.enable && cfg.settings != { }) {
        source = tomlFormat.generate "sottomano.toml" cfg.settings;
      };

      "sottomano/openrouter-management-key" =
        lib.mkIf (cfg.secretFiles."openrouter-management-key" != null)
          {
            source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."openrouter-management-key";
          };

      "sottomano/claude-code-oauth" = lib.mkIf (cfg.secretFiles."claude-code-oauth" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."claude-code-oauth";
      };

      "sottomano/gh-auth-token" = lib.mkIf (cfg.secretFiles."gh-auth-token" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."gh-auth-token";
      };

      "sottomano/unsplash-access-key" = lib.mkIf (cfg.secretFiles."unsplash-access-key" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."unsplash-access-key";
      };

      "sottomano/caldav-password" = lib.mkIf (cfg.secretFiles."caldav-password" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."caldav-password";
      };
    };

    services.umbra.apps.sottomano = lib.mkIf cfg.umbra.enable {
      target = "sottomano.toml";
      dark.source = variantToml themeCfg.palettes.dark;
      light.source = variantToml themeCfg.palettes.light;
    };

    services.niri.settings.includes = [
      {
        path = "${config.xdg.configHome}/sottomano/niri.kdl";
        optional = true;
      }
    ];

    systemd.user.services.sottomano = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "sottomano bar";
        PartOf = [ "graphical-session.target" ];
        After = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 2;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
