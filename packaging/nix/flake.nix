{
  description = "Fajar Lang -- systems programming for embedded AI + OS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "fj";
          version = "6.1.0";
          src = ../..;
          cargoLock.lockFile = ../../Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
          ];
          meta = with pkgs.lib; {
            description = "Systems programming language for embedded AI + OS";
            homepage = "https://github.com/fajarkraton/fajar-lang";
            license = licenses.mit;
            mainProgram = "fj";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.rust-analyzer
            pkgs.pkg-config
          ];
        };
      });
}
