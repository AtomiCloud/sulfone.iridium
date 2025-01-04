{ pkgs, packages }:
with packages;
{
  system = [
    coreutils
    sd
    bash
    findutils
    gnused
  ];

  dev = [
    pls
    git
  ];

  main = [
    toml-cli
    infisical
    nfpm
    goreleaser
    go
    rust
  ];

  lint = [
    # http
    treefmt
    gitlint
    shellcheck
    hadolint
    sg
  ];

  releaser = [
    sg
  ];

}
