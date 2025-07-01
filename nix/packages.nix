{ pkgs, atomi, fenixpkgs, pkgs-2505, pkgs-unstable }:
let
  all = {
    atomipkgs = (
      with atomi;
      {
        inherit
          atomiutils
          toml-cli
          sg
          pls;
      }
    );
    nix-unstable = (
      with pkgs-unstable;
      {
        inherit
          goreleaser
          go
          ;
      }
    );
    nix-2505 = (
      with pkgs-2505;
      {
        inherit
          infisical
          docker
          rustup

          git

          # lint
          treefmt
          gitlint
          shellcheck
          hadolint
          ;
      }
    );
    fenix = (
      with fenixpkgs;
      {
        rust = with complete.toolchain; combine ([
          stable.cargo
          stable.rustc
          stable.rust-src
          stable.rust-std
          stable.clippy
          pkgs-2505.openssl
        ]
        ++ pkgs.lib.optionals (pkgs.stdenv.isLinux && pkgs.stdenv.hostPlatform.isx86_64) [
          targets.x86_64-unknown-linux-musl.stable.rust-std
        ]
        ++ pkgs.lib.optionals (pkgs.stdenv.isLinux && pkgs.stdenv.hostPlatform.isAarch64) [
          targets.aarch64-unknown-linux-musl.stable.rust-std
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs-2505.darwin.libiconv
        ]);
      }
    );
  };
in
with all;
atomipkgs //
fenix //
nix-2505 //
nix-unstable
