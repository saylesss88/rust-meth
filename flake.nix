{
  description = "rust-meth: Discover methods available on any Rust type with fuzzy filtering, inline documentation, interactive selection, and go-to-definition into standard library source code.";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "rust-meth";
          version = "0.1.5";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
            mold
          ];
          meta = with pkgs.lib; {
            description = "Discover methods available on any Rust type with fuzzy filtering, inline documentation, interactive selection, and go-to-definition into standard library source code.";
            license = with licenses; [ mit asl20 ];
            mainProgram = "rust-meth";
          };
        };
        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            pkg-config
            mold
            cargo
            rustc
            rust-analyzer
            clippy
            rustfmt
          ];
          shellHook = ''
            export RUST_BACKTRACE=1
            export CARGO_TERM_COLOR=always
            echo "rust-meth dev shell loaded"
          '';
        };
      }
    );
}
