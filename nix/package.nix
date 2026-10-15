{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "pulse";
  version = "1.0.0";
  src = let
    fs = lib.fileset;
    s = ../.;
  in
    fs.toSource {
      root = s;
      fileset = fs.unions [
        (s + /src)

        (s + /Cargo.toml)
        (s + /Cargo.lock)
      ];
    };

  cargoLock.lockFile = "${finalAttrs.src}/Cargo.lock";
})
