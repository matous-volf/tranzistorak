let
  pkgs = import <nixpkgs> {
    overlays = [ (import rust-overlay) ];
  };
  rust-overlay = fetchGit {
    url = "https://github.com/oxalica/rust-overlay";
    rev = "7fcafdf6f0761462d68012aba6a627be3f008d68";
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
