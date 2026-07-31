{
  description = "Filesystem-native snippet library and agent-friendly CLI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      # nixpkgs 26.11 dropped x86_64-darwin, so evaluating it throws.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      cargoVersion = (nixpkgs.lib.importTOML ./Cargo.toml).package.version;
      packageFor =
        pkgs:
        (pkgs.callPackage ./nix/package.nix { }).overrideAttrs {
          version = cargoVersion;
          src = self;
          cargoDeps = pkgs.rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
        };
    in
    {
      packages = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          package = packageFor pkgs;
        in
        {
          default = package;
          sniplab = package;
        }
      );

      apps = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = {
            type = "app";
            program = pkgs.lib.getExe self.packages.${system}.default;
            meta = self.packages.${system}.default.meta;
          };
        }
      );

      overlays.default = final: _prev: {
        sniplab = packageFor final;
      };

      devShells = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer
              gitMinimal
            ];
          };
        }
      );

      checks = eachSystem (system: {
        default = self.packages.${system}.default;
      });
    };
}
