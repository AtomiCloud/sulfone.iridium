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

  infra = [
  ];

  main = [
    toml-cli
    nfpm
    goreleaser
    go
    rust
    cargo2junit
  ];

  lint = [
    # http
    treefmt
    infisical
    gitlint
    shellcheck
    sg
  ];

  ci = [

  ];

  releaser = [
    nodejs
    sg
    npm
  ];

}
