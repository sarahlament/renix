{
  description = "renix - NixOS Rebuild Manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";
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

        # Build only the cargo dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly {
          src = craneLib.cleanCargoSource ./.;
        };

        renix = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
          inherit cargoArtifacts;

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
          renix-deps = cargoArtifacts;
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

            #nix.settings = {
              #substituters = ["https://attic.lament.gay/athena"];
              #trusted-public-keys = ["athena:iBycXk8bKCemxVvAjbfxzDFh/kDBNmn/iExsjWb2jH8="];
            #};
          };
        };
      };

      overlays.renix = self.overlays.default;
      overlays.default = final: prev: {
        renix = self.packages.${final.system}.renix;
      };
    };
}
