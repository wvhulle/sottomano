{ inputs, ... }:
{
  imports = [ inputs.git-hooks.flakeModule ];

  perSystem =
    { config, pkgs, ... }:
    let
      astGrepRuleCheck = pkgs.writeShellApplication {
        name = "check-ast-grep-rules";
        runtimeInputs = [ pkgs.ast-grep ];
        text = ''
          ast-grep test --skip-snapshot-tests
          ast-grep scan --error \
            corredo/src corredo/examples corredo/tests \
            sottomano-desktop-sources/src sottomano-widgets/src sottomano-shell/src \
            sottomano-widget-showcase/src sottomano-desktop-sinks/src sottomano-toml-config/src
        '';
      };
    in
    {
      pre-commit.settings = {
        package = pkgs.prek;
        hooks = {
          treefmt = {
            enable = true;
            packageOverrides.treefmt = config.treefmt.build.wrapper;
          };
          ast-grep-rules = {
            enable = true;
            name = "ast-grep design rules";
            entry = pkgs.lib.getExe astGrepRuleCheck;
            pass_filenames = false;
          };
        };
      };
    };
}
