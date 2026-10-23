{
  description = "Rust development environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      utils,
      rust-overlay,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        mingwTarget = pkgs.pkgsCross.mingwW64;
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
          targets = [ "x86_64-pc-windows-gnu" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            mingwTarget.stdenv.cc
          ];
          buildInputs = [
            rustToolchain
            mingwTarget.windows.pthreads
            mingwTarget.stdenv.cc
            mingwTarget.openssl
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          # Environment variables instructing Cargo exactly which linker to use
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${mingwTarget.stdenv.cc}/bin/${mingwTarget.stdenv.cc.targetPrefix}cc";
          TARGET_CC = "${mingwTarget.stdenv.cc}/bin/${mingwTarget.stdenv.cc.targetPrefix}cc";
          # Optional: Fixes missing native library paths during cross-compilation
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${mingwTarget.windows.pthreads}/lib";
          CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";


          shellHook = /* bash */ ''
            echo ""
            echo -e "\e[1;38;5;208m | Rust environment\e[0m"
            cargo version
          '';

        };
      }
    );
}
