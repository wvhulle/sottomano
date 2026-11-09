{ pkgs }:
let
  inherit (pkgs) lib;
in
pkgs.stdenvNoCC.mkDerivation {
  pname = "sottomano-documentation-site";
  version = "0";
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../docs
      ../README.md
    ];
  };
  nativeBuildInputs = [ pkgs.mdbook ];
  buildPhase = ''
    runHook preBuild
    mdbook build docs --dest-dir "$out"
    rm "$out/book.toml"
    rm "$out/theme/favicon.svg"
    runHook postBuild
  '';
  dontInstall = true;
}
