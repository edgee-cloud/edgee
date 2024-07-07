{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1.*.tar.gz";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
  }: let
    overlays = [
      rust-overlay.overlays.default
      (final: prev: {
        rustToolchain = prev.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "llvm-tools"];
        };
      })
    ];
    systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f { pkgs = import nixpkgs {inherit overlays system;}; });
  in {
    packages = forAllSystems ({ pkgs }: {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "edgee";
        version = "0.1.1";
        buildInputs = with pkgs; [ 
          rustToolchain 
        ];
        src = ./.;
        cargoLock = { lockFile = ./Cargo.lock; };
      };
    });

    devShells = forAllSystems ({pkgs}: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          darwin.apple_sdk.frameworks.Security
          rustToolchain
          cargo-deny
          cargo-edit
          cargo-expand
          cargo-watch
          rust-analyzer
          rustfmt

          gh
        ];
      };
    });
  };
}
