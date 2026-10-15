{
  description = "PSI + rusage + EMA latency telemetry for load-aware schedulers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
    in {
      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "pulse";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        buildFeatures = ["rusage"];
      };

      checks.default = pkgs.rustPlatform.buildRustPackage {
        pname = "pulse-tests";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        buildFeatures = ["rusage"];
        doCheck = true;
      };

      devShells.default = pkgs.mkShell {
        packages = with pkgs; [cargo rustc gcc rustfmt clippy taplo];
      };
    });
}
