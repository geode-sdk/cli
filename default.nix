{ pkgs ? import <nixpkgs> {} }:

let
  isDarwin = pkgs.stdenv.isDarwin;
  darwinFrameworks = with pkgs.darwin.apple_sdk.frameworks; [
    Security
    AppKit
    SystemConfiguration
  ];
in

pkgs.rustPlatform.buildRustPackage {
  pname = "geode-cli";
  version = "3.4.0";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = with pkgs; [
    pkg-config
    openssl
  ] ++ (if isDarwin then darwinFrameworks else []);

  buildInputs = with pkgs; [
    openssl
  ] ++ (if isDarwin then darwinFrameworks else []);

  postInstall = ''
    mkdir -p $out/share/bash-completion/completions
    mkdir -p $out/share/zsh/site-functions
    mkdir -p $out/share/fish/vendor_completions.d

    $out/bin/geode completions bash > $out/share/bash-completion/completions/geode
    $out/bin/geode completions zsh > $out/share/zsh/site-functions/_geode
    $out/bin/geode completions fish > $out/share/fish/vendor_completions.d/geode.fish
  '';
}
