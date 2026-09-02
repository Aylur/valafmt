{
  description = "An opinionated code formatter for Vala";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
  in {
    packages = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (nixpkgs.lib.importTOML ./Cargo.toml) package;
    in {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = package.name;
        version = package.version;

        src = nixpkgs.lib.cleanSource ./.;
        cargoLock.lockFile = ./Cargo.lock;

        meta = {
          description = "An opinionated code formatter for Vala";
          homepage = "https://github.com/Aylur/valafmt";
          license = nixpkgs.lib.licenses.mit;
          mainProgram = package.name;
        };
      };
    });

    devShells = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          rust-analyzer
          cargo-insta
        ];
      };
    });
  };
}
