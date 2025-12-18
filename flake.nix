{
  description = "renix - NixOS Rebuild Manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        renix = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;

          buildInputs = [];
          nativeBuildInputs = [];
        };
      in {
        packages = {
          default = renix;
          renix = renix;
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            rust-analyzer
            cargo-watch
          ];
        };
      }
    );
}
