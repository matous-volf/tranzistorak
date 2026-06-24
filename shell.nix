let
  pkgs = import <nixpkgs> {
    overlays = [ (import rust-overlay) ];
  };
  rust-overlay = fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
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
