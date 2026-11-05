{
  pkgs ? import <nixpkgs> { },
}:
let
  manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
  nightlyToolchain = pkgs.rust-bin.selectLatestNightlyWith (env: env.default);

  # Create a custom rustPlatform with the nightly toolchain
  rustPlatform = pkgs.makeRustPlatform {
    cargo = nightlyToolchain;
    rustc = nightlyToolchain;
  };
in
rustPlatform.buildRustPackage {
  pname = manifest.name;
  version = manifest.version;

  # Tells Nix where your source code is
  src = pkgs.lib.cleanSource ./.;
  nativeBuildInputs = [
    rustPlatform.bindgenHook
    pkgs.duckdb.lib
  ];
  buildInputs = [
    pkgs.duckdb.lib
  ];

  # Tracks your Rust dependencies deterministically
  cargoLock.lockFile = ./Cargo.lock;
}
