{
  description = "sottomano — iced + iced_exwlshell shell for niri, sway and Hyprland";

  nixConfig = {
    extra-substituters = [ "https://wvhulle.cachix.org" ];
    extra-trusted-public-keys = [
      "wvhulle.cachix.org-1:heXx8DZMiRsKUx6l1TxNoF+Nmtmz66QEdsonQzc1ir0="
    ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    import-tree.url = "github:vic/import-tree";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    phosphor-core = {
      url = "github:phosphor-icons/core/2b75f3ad12b420c9504ef05df8d2564a28f8500e";
      flake = false;
    };
    simple-icons = {
      url = "github:simple-icons/simple-icons/16.35.0";
      flake = false;
    };
    niri-hm = {
      url = "git+https://codeberg.org/wvhulle/niri-hm?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    umbra = {
      url = "git+https://codeberg.org/wvhulle/umbra";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];
      imports = [ (inputs.import-tree ./flake-modules) ];
    };
}
