{ pkgs, packages }:
with packages;
{
  system = [
    atomiutils
  ];

  dev = [
    pls
    git
    rust
  ];

  main = [
    toml-cli
    infisical
    goreleaser
  ];

  lint = [
    # http
    treefmt
    gitlint
    shellcheck
    hadolint
    sg
  ];

  ci = [
    rustup
  ];

  releaser = [
    sg
  ];

}
