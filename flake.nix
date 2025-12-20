{
  description = "renix - NixOS Rebuild Manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    pre-commit-hooks,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            rustfmt.enable = true;
            clippy = {
              enable = true;
              packageOverrides.clippy = pkgs.clippy;
            };
          };
        };

        renix = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;

          buildInputs = [];
          nativeBuildInputs = [];

          meta = {
            description = "NixOS Rebuild Manager TUI";
            license = pkgs.lib.licenses.mit;
            maintainers = [
              {
                name = "Sarah Lament";
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

          shellHook = ''
            ${pre-commit-check.shellHook}
          '';
        };
      }
    )
    // {
      nixosModules = {
        default = self.nixosModules.renix;
        renix = {
          config,
          lib,
          pkgs,
          ...
        }: {
          options.programs.renix.enable = lib.mkEnableOption "renix, a NixOS host manager TUI";
          config = lib.mkIf config.programs.renix.enable {
            environment.systemPackages = [self.packages.${pkgs.system}.renix];
          };
        };
      };

      overlays.renix = self.overlays.default;
      overlays.default = final: prev: {
        renix = self.packages.${final.system}.renix;
      };
    };
}
