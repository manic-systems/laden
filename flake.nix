{
  inputs.nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.zst";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forEachSystem = nixpkgs.lib.genAttrs systems;
    pkgsFor = system: nixpkgs.legacyPackages.${system};
  in {
    packages = forEachSystem (system: let
      pkgs = pkgsFor system;
    in {
      laden = pkgs.callPackage ./nix/package.nix {};
      default = self.packages.${system}.laden;
    });

    devShells = forEachSystem (system: let
      pkgs = pkgsFor system;
    in {
      default = pkgs.callPackage ./nix/shell.nix {};
    });

    formatter = forEachSystem (system: (pkgsFor system).alejandra);
  };
}
