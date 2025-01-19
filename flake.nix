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
          ];

          buildInputs = with pkgs; [
            openssl
          ];

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
          ];
        };
      });
}
