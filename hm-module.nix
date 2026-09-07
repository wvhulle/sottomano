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

  modeToPalette = p: {
    surface = p.base00;
    surface_raised = p.base02;
    bar_foreground = p.base05;
    inactive = p.base04;
    dim = p.base03;
    urgent = p.base08;
    warning = p.base0A;
    focus = p.base0C;
  };

  variantToml =
    modePalette:
    tomlFormat.generate "vetrata.toml" (
      lib.recursiveUpdate cfg.settings {
        palette = modeToPalette modePalette // (cfg.settings.palette or { });
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
  cornerRadius = 1.0 * cfg.blurCornerRadius;
  pillRadius = cornerRadius;
  cardShadow = {
    enable = true;
    softness = 30;
    spread = 10;
    color = "#00000070";
  };
  gradientShadow = sign: {
    enable = true;
    softness = 27;
    spread = 25;
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
    # Disabled because tooltips now have their own shadows
    # {
    #   matches = [ { namespace = "^vetrata-tooltip$"; } ];
    #   geometry-corner-radius = uniformRadius cornerRadius;
    #   shadow = cardShadow;
    # }
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

    blurCornerRadius = lib.mkOption {
      type = lib.types.int;
      default = 8;
      description = ''
        Corner radius of the niri layer rules for the vetrata-blur and
        vetrata-blur-pill surfaces. The bar and its pills share one corner
        radius, which is a runtime panel setting adjusted from the settings
        menu, so the compositor cannot follow it; set this to the radius you
        usually run the bar at.

        Having vetrata push its live radius over ext-background-effect-v1
        instead would look worse, not better. That protocol's set_blur_region
        takes a wl_region, so a rounded shape is a staircase of integer
        rectangles, and niri applies it by filtering damage rects snapped to
        whole physical pixels rather than through a shader — the corners come
        out jagged. Supplying a client region also makes niri pass
        clip_to_geometry false for layer surfaces, which drops the clip that
        carries geometry-corner-radius, so the antialiased rounding in
        rounding_alpha.frag is skipped entirely. The layer rule is the only
        path to smooth blur corners. Verified against niri 26.04.
      '';
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          palette.bar_foreground = "#ffffff";
          chrome.outer_height_px = 28.0;
          font.text_size_px = 15.0;
        }
      '';
      description = ''
        Contents of ~/.config/vetrata.toml, written verbatim.
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
          into ~/.config/vetrata/, where the bar looks for it. When unset, the
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
          ~/.config/vetrata/. When unset, the Claude pills are hidden.
        '';
      };

      "gh-auth-token" = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Path to a file holding a bare `gho_` GitHub OAuth token. The Copilot
          usage pills read it and call `/copilot_internal/user` directly.
          Symlinked into ~/.config/vetrata/. When unset, the Copilot quota pills
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
          Symlinked into ~/.config/vetrata/. The pill needs no configuration: it
          shows when this key and a wallpaper daemon (`awww` or `swww`, tried in
          that order) are both present, and hides otherwise.
        '';
      };
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

    xdg.configFile = {
      "vetrata.toml" = lib.mkIf (!cfg.umbra.enable && cfg.settings != { }) {
        source = tomlFormat.generate "vetrata.toml" cfg.settings;
      };

      "vetrata/openrouter-management-key" =
        lib.mkIf (cfg.secretFiles."openrouter-management-key" != null)
          {
            source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."openrouter-management-key";
          };

      "vetrata/claude-code-oauth" = lib.mkIf (cfg.secretFiles."claude-code-oauth" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."claude-code-oauth";
      };

      "vetrata/gh-auth-token" = lib.mkIf (cfg.secretFiles."gh-auth-token" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."gh-auth-token";
      };

      "vetrata/unsplash-access-key" = lib.mkIf (cfg.secretFiles."unsplash-access-key" != null) {
        source = config.lib.file.mkOutOfStoreSymlink cfg.secretFiles."unsplash-access-key";
      };
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
