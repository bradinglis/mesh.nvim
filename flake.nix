{
  description = "Rust development environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      # Change this to your system architecture if not on x86_64 Linux
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      nightlyToolchain = pkgs.rust-bin.selectLatestNightlyWith (env: env.default);

      # Create a custom rustPlatform with the nightly toolchain
      rustPlatform = pkgs.makeRustPlatform {
        cargo = nightlyToolchain;
        rustc = nightlyToolchain;
      };
      nativeBuildInputs = [
        rustPlatform.bindgenHook
        pkgs.duckdb.lib
      ];
      buildInputs = [
        pkgs.duckdb.lib
      ];
      version = manifest.version;
      src = pkgs.lib.fileset.toSource {
        root = ./.;
        fileset = pkgs.lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./sql
          ./lua
          ./src
        ];
      };
    in
    {
      packages.${system}.default = (
        let
          binary = rustPlatform.buildRustPackage {
            pname = "mesh-nvim-lib";
            inherit
              nativeBuildInputs
              buildInputs
              version
              src
              ;
            dependencies = [
              pkgs.duckdb.lib
            ];
            cargoLock.lockFile = ./Cargo.lock;
          };
        in
        pkgs.vimUtils.buildVimPlugin {
          pname = "mesh-nvim";
          inherit version src;

          postPatch = ''
            mkdir -p target/release
            cp -vf ${binary}/lib/libmesh.so target/release/libmesh.so
          '';

          nvimSkipModule = [
            "empty_config"
          ];

          dependencies = [
            pkgs.duckdb.lib
            pkgs.vimPlugins.snacks-nvim
          ];
        }
      );

      devShells.default = pkgs.mkShell {
        inherit nativeBuildInputs buildInputs;
        shellHook = /* bash */ ''
          echo ""
          echo -e "\e[1;38;5;208m | Rust environment\e[0m"
          cargo version
        '';

      };
    };
}
