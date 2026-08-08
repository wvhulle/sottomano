{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.vetrata;
  themeCfg = config.services.umbra or null;
  tomlFormat = pkgs.formats.toml { };

  foregroundPalette = p: {
    bar_foreground = p.base05;
    inactive = p.base04;
    dim = p.base03;
  };
  accentPalette = p: {
    urgent = p.base08;
    warning = p.base0A;
    focus = p.base0C;
  };

  variantToml =
    modePalette:
    tomlFormat.generate "vetrata.toml" (
      lib.recursiveUpdate cfg.settings {
        palette =
          foregroundPalette themeCfg.palettes.dark
          // accentPalette modePalette
          // (cfg.settings.palette or { });
      }
    );

  radius0 = {
    top-left = 0.0;
    top-right = 0.0;
    bottom-left = 0.0;
    bottom-right = 0.0;
  };
  uniformRadius = r: {
    top-left = r;
    top-right = r;
    bottom-left = r;
    bottom-right = r;
  };
  cornerRadius = 1.0 * (cfg.settings.chrome.corner_radius_px or 8);
  pillRadius = 0.5 * (cfg.settings.chrome.outer_height_px or 22.0);
  cardShadow = {
    enable = true;
    softness = 30;
    spread = 10;
    color = "#00000070";
  };
  gradientShadow = sign: {
    enable = true;
    softness = 27;
    spread = 24;
    color = "#00000060";
    offset = {
      x = 0;
      y = sign * 3;
    };
  };
  vetrataLayerRules = [
    {
      matches = [ { namespace = "^vetrata$"; } ];
      geometry-corner-radius = radius0;
      shadow.enable = false;
    }
    {
      matches = [ { namespace = "^vetrata-blur$"; } ];
      geometry-corner-radius = uniformRadius cornerRadius;
      shadow.enable = false;
      background-effect.blur = true;
    }
    {
      matches = [ { namespace = "^vetrata-blur-pill$"; } ];
      geometry-corner-radius = uniformRadius pillRadius;
      shadow.enable = false;
      background-effect.blur = true;
    }
    {
      matches = [ { namespace = "^vetrata-backdrop-top$"; } ];
      geometry-corner-radius = radius0;
      shadow = gradientShadow (-1);
    }
    {
      matches = [ { namespace = "^vetrata-backdrop-bottom$"; } ];
      geometry-corner-radius = radius0;
      shadow = gradientShadow 1;
    }
    {
      matches = [ { namespace = "^vetrata-notifications$"; } ];
      geometry-corner-radius = radius0;
      shadow.enable = false;
    }
    {
      matches = [ { namespace = "^vetrata-tooltip$"; } ];
      geometry-corner-radius = uniformRadius cornerRadius;
      shadow = cardShadow;
    }
  ];
in
{
  options.services.vetrata = {
    enable = lib.mkEnableOption "vetrata bar";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "vetrata.packages.\${system}.default";
      description = "The vetrata package to install.";
    };

    systemdUnit = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to create a systemd user service that starts vetrata with
        graphical-session.target. Disable to spawn vetrata from a
        compositor-specific hook instead.
      '';
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          palette.bar_foreground = "#ffffff";
          chrome = {
            outer_height_px = 28.0;
            # Cast the gradient legibility shadow (opens the off-screen casters
            # and activates the niri layer-rules below).
            shadow = true;
          };
          font.name = "Inter";
        }
      '';
      description = ''
        Contents of ~/.config/vetrata.toml, written verbatim. Maps to the
        `[palette]`, `[chrome]`, `[spacing]`, `[widgets]`, `[font]`,
        `[notifications]` and `[tray]` tables in vetrata. Every key has a
        default, so set only the ones you override; leave this empty to keep
        the built-in gruvbox-dark defaults. Unknown keys are a fatal startup
        error, so stale `[layout]` sections from before the chrome/spacing/
        widgets split must be migrated. Set `chrome.shadow = true` to enable
        the legibility shadow. Floating, background blur and disconnected
        pill groups are no longer config: they are runtime toggles in the
        bar's panel-settings menu, persisted in the state database. The blur
        and pill layer-rules below activate whenever those toggles open their
        caster surfaces.
      '';
    };

    umbra.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Runtime dark/light palette switching via the umbra app registry.
        Requires services.umbra (from umbra.homeModules) in the same
        home-manager config: registers services.umbra.apps.vetrata with
        dark/light palette variants derived from services.umbra.palettes.* and
        restarts vetrata.service on mode switch. Set false for a standalone
        install with no umbra — vetrata then falls back to the static
        settings.palette written to ~/.config/vetrata.toml.
      '';
    };

    secretFiles = {
      openRouterManagement = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file containing an OpenRouter **management** key as plain
          text. A regular API key is not enough: the usage widget calls
          `/analytics/query`, which only management keys may access. When unset,
          the OpenRouter pill is hidden.
        '';
      };

      "claude-code-oauth" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to the Claude Code OAuth credentials JSON (the
          `~/.claude/.credentials.json` blob with `claudeAiOauth.accessToken`).
          Drives the Claude subscription gauge; on API-key machines (no OAuth
          token) it instead surfaces `ccusage` daily spend. When unset, the
          Claude pills are hidden.
        '';
      };

      "gh-auth-token" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file holding a bare `gho_` GitHub OAuth token. The Copilot
          usage pills read it and call `/copilot_internal/user` directly. When
          unset, the Copilot quota pills are hidden (the local-`ccusage` Copilot
          pill needs no token and still shows if `ccusage` is installed).
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = lib.optionals cfg.umbra.enable [
      {
        assertion = themeCfg != null && themeCfg ? palettes;
        message = ''
          services.vetrata.umbra.enable requires services.umbra (provided by
          umbra.homeModules.default/schema). Import it in the same home-manager
          config, or set services.vetrata.umbra.enable = false.
        '';
      }
    ];

    home.packages = [ cfg.package ];

    xdg.configFile."vetrata.toml" = lib.mkIf (!cfg.umbra.enable && cfg.settings != { }) {
      source = tomlFormat.generate "vetrata.toml" cfg.settings;
    };

    services.umbra.apps.vetrata = lib.mkIf cfg.umbra.enable {
      target = "vetrata.toml";
      dark.source = variantToml themeCfg.palettes.dark;
      light.source = variantToml themeCfg.palettes.light;
      reload.restartUnits = [ "vetrata.service" ];
    };

    services.niri.settings.layer-rules = lib.mkAfter vetrataLayerRules;

    systemd.user.services.vetrata = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "vetrata bar";
        PartOf = [ "graphical-session.target" ];
        After = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 2;
        Environment = lib.mapAttrsToList (name: value: "${name}=${toString value}") (
          lib.filterAttrs (_: value: value != null) {
            VETRATA_OPENROUTER_KEY_FILE = cfg.secretFiles.openRouterManagement;
            VETRATA_CLAUDE_CODE_OAUTH_FILE = cfg.secretFiles."claude-code-oauth";
            VETRATA_GH_AUTH_TOKEN_FILE = cfg.secretFiles."gh-auth-token";
          }
        );
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    systemd.user.paths.vetrata = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "Watch vetrata config and restart the bar on change";
        PartOf = [ "graphical-session.target" ];
      };
      Path = {
        PathModified = "%h/.config/vetrata.toml";
        Unit = "vetrata-reload.service";
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    systemd.user.services.vetrata-reload = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "Restart vetrata after a config change";
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        Type = "oneshot";
        ExecStart = "${pkgs.systemd}/bin/systemctl --user restart vetrata.service";
      };
    };
  };
}
