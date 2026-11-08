{
  description = "Rust development environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
    }:
    let
      # Change this to your system architecture if not on x86_64 Linux
      system = "x86_64-linux";
      overlays = [ (import rust-overlay) ];
      pkgs = import nixpkgs { inherit system overlays; };
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
      rustToolchainFor = p: p.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
      rustToolchain = rustToolchainFor pkgs;

      craneLib = (crane.mkLib pkgs).overrideToolchain (rustToolchainFor);
      # Create a custom rustPlatform with the nightly toolchain
      nativeBuildInputs = [
        pkgs.duckdb.lib
      ];
      buildInputs = [
        pkgs.duckdb.lib
        rustToolchain
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
      devShells.${system}.default = pkgs.mkShell {
        inherit nativeBuildInputs buildInputs;
        dependencies = [
          pkgs.duckdb.lib
          pkgs.vimPlugins.snacks-nvim
        ];
        shellHook = /* bash */ ''
          echo ""
          echo -e "\e[1;38;5;208m | Rust environment\e[0m"
          cargo version
        '';
      };
      packages.${system}.default = (
        let
          commonArgs = {
            inherit
              src
              nativeBuildInputs
              buildInputs
              version
              ;
            strictDeps = false;
            cargoVendorDir = craneLib.vendorMultipleCargoDeps {
              inherit (craneLib.findCargoFiles src) cargoConfigs;
              cargoLockList = [
                ./Cargo.lock

                # Unfortunately this approach requires IFD (import-from-derivation)
                # otherwise Nix will refuse to read the Cargo.lock from our toolchain
                # (unless we build with `--impure`).
                #
                # Another way around this is to manually copy the rustlib `Cargo.lock`
                # to the repo and import it with `./path/to/rustlib/Cargo.lock` which
                # will avoid IFD entirely but will require manually keeping the file
                # up to date!
                "${rustToolchain.passthru.availableComponents.rust-src}/lib/rustlib/src/rust/library/Cargo.lock"
              ];
            };

          };

          binary = craneLib.buildPackage (
            commonArgs
            // {
              pname = "mesh-nvim-lib";
              cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            }
          );
        in
          binary
      );

    };
}
