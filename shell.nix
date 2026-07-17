# Dev shell for the Odin→iced spike. Provides the Rust toolchain plus the Wayland /
# xkb / GPU libraries iced_layershell + wgpu need at build and run time.
#
#   nix-shell --run 'cargo run'            # launch the bar (inside a niri session)
#   nix-shell --run 'cargo run -- --smoke' # headless data-layer test
{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.cargo
    pkgs.rustc
  ];
  buildInputs = [
    pkgs.wayland
    pkgs.libxkbcommon
    pkgs.libGL
    pkgs.vulkan-loader
  ];
  # wgpu / winit dlopen these at runtime; they must be on the loader path.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
    pkgs.wayland
    pkgs.libxkbcommon
    pkgs.libGL
    pkgs.vulkan-loader
  ];
  # Point the script runner at Odin's reused Nu scripts.
  ODIN_SCRIPTS_DIR = "/home/wvhulle/Code/odin/scripts";
}
