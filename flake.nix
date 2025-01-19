{
  description = "A flake to install the Geode CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        isDarwin = system == "x86_64-darwin" || system == "aarch64-darwin";
        darwinFrameworks = with pkgs.darwin.apple_sdk.frameworks; [
          Security
          AppKit
          SystemConfiguration
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "geode-cli";
          version = "3.4.0";

          src = self;

          cargoInstallFlags = [ "--release" ];
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

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

          meta = with pkgs.lib; {
            description = "Geode CLI";
            homepage = "https://github.com/geode-sdk/cli";
            license = licenses.boost; # Boost License
          };
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/geode";
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            pkg-config
            openssl
          ] ++ (if isDarwin then darwinFrameworks else []);
        };
      });
}
