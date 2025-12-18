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

          meta = {
            description = "NixOS Rebuild Manager TUI";
            license = pkgs.lib.licenses.mit;
            maintainers = [
              {
                name = "Sarah";
                email = "sarah@lament.gay";
                github = "sarahlament";
                githubId = 4612427;
              }
            ];
            mainProgram = "renix";
          };
        };
      in {
        packages = {
          default = renix;
          renix = renix;
        };

        formatter = pkgs.alejandra;

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            rust-analyzer
            cargo-watch
          ];
        };
      }
    )
    // {
      nixosModules.default = import ./nix/module.nix;

      overlays.renix = self.overlays.default;
      overlays.default = final: prev: {
        renix = final.callPackage ({
          lib,
          rustPlatform,
        }:
          rustPlatform.buildRustPackage {
            pname = "renix";
            version = "0.1.0";

            src = lib.cleanSource ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            buildInputs = [];

            meta = {
              description = "NixOS Rebuild Manager TUI";
              license = lib.licenses.mit;
              maintainers = [
                {
                  name = "Sarah";
                  email = "sarah@lament.gay";
                  github = "sarahlament";
                  githubId = 4612427;
                }
              ];
              mainProgram = "renix";
            };
          }) {};
      };
    };
}
