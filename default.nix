{ pkgs ? import <nixpkgs> {} }:
pkgs.rustPlatform.buildRustPackage {
  pname = "geode-cli";
  version = "3.4.0";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = with pkgs; [ pkg-config openssl ];
  buildInputs = with pkgs; [ openssl ];
}
