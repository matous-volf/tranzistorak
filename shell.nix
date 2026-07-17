let
  pkgs = import <nixpkgs> {
    overlays = [ (import rust-overlay) ];
  };
  rust-overlay = fetchGit {
    url = "https://github.com/oxalica/rust-overlay";
    rev = "068175006cfb69d5b541a140ed93e361488c9e53";
    ref = "master";
  };
  toolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;
in
pkgs.mkShell {
  packages = [
    toolchain
    pkgs.alsa-lib
    pkgs.cmake
    pkgs.pkg-config
    pkgs.oniguruma
    pkgs.openssl
  ];
  env = {
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
  };
}
