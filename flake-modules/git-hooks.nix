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
          ast-grep scan --error
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
