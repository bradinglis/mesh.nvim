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
      # ONNX Runtime is loaded at runtime (ort `load-dynamic`), so it is not a
      # build input. The version comes from the nixpkgs revision pinned in
      # flake.lock; ort needs ONNX Runtime API >= 17.
      onnxruntime = pkgs.onnxruntime;
      onnxLib =
        if onnxRuntimeOk then
          "${onnxruntime}/lib/libonnxruntime.so"
        else
          throw "onnxruntime ${onnxruntime.version} is older than the 1.17 required by ort";
      onnxRuntimeOk = pkgs.lib.versionAtLeast onnxruntime.version "1.17";
      src = pkgs.lib.fileset.toSource {
        root = ./.;
        fileset = pkgs.lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./sql
          ./lua
          ./src
          ./tests
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
          export ORT_DYLIB_PATH="${onnxLib}"
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
            dependencies = [
              pkgs.duckdb.lib
            ];
            strictDeps = false;
            cargoVendorDir = craneLib.vendorCargoDeps {
              src = ./.;
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
        pkgs.vimUtils.buildVimPlugin {
          pname = "mesh-nvim";
          inherit version src;

          postPatch = ''
            mkdir -p target/release
            cp -vf ${binary}/lib/libmesh.so target/release/libmesh.so
            cat > lua/mesh-nvim/runtime.lua <<EOF
            return {
              onnxruntime = "${onnxLib}",
              onnxruntime_version = "${onnxruntime.version}",
            }
            EOF
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

    };
}
