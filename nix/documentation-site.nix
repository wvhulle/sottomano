{ pkgs, readmeRecordings }:
let
  inherit (pkgs) lib;
in
pkgs.stdenvNoCC.mkDerivation {
  pname = "sottomano-documentation-site";
  version = "0";
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../book.toml
      ../docs
      ../README.md
      ../logo.svg
    ];
  };

  nativeBuildInputs = [
    pkgs.mdbook
    pkgs.mdbook-mermaid
  ];
  nativeInstallCheckInputs = [ pkgs.lychee ];

  postPatch = ''
    mdbook-mermaid install .
    shopt -s globstar
    for markdown in README.md docs/**/*.md; do
      substituteInPlace "$markdown" \
        --replace-quiet https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/ /recordings/ \
        --replace-quiet https://codeberg.org/wvhulle/sottomano/raw/branch/main/logo.svg /logo.svg \
        --replace-quiet https://sottomano.work/ /
    done
  '';

  buildPhase = ''
    runHook preBuild
    mdbook build . --dest-dir site
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mv site $out
    rm $out/book.toml $out/theme/favicon.svg
    ln -s ${readmeRecordings}/recordings $out/recordings
    cp logo.svg $out/logo.svg
    runHook postInstall
  '';

  dontFixup = true;

  doInstallCheck = true;
  SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
  installCheckPhase = ''
    runHook preInstallCheck
    if grep -rl --include='*.html' 'raw/branch/' $out; then
      echo "hotlinks to the Codeberg raw branches remain in the pages above"
      exit 1
    fi
    lychee --no-progress --offline --include-fragments --root-dir $out "$out/**/*.html"
    runHook postInstallCheck
  '';
}
