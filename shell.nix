let
  pkgs = import <nixpkgs> {
    overlays = [ (import rust-overlay) ];
  };
  rust-overlay = fetchGit {
    url = "https://github.com/oxalica/rust-overlay";
    rev = "4ca2963243427cb86bd6b4d9ca6010db0162a619";
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
