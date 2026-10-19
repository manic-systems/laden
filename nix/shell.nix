{
  mkShell,
  rustc,
  cargo,
  rust-analyzer,
  rustfmt,
  clippy,
  taplo,
  rustPlatform,
  extraPackages ? [ ],
}:
mkShell {
  name = "laden";

  strictDeps = true;

  nativeBuildInputs = [
    rustc
    cargo
    rust-analyzer
    rustfmt
    clippy
    taplo
  ]
  ++ extraPackages;

  env.RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
