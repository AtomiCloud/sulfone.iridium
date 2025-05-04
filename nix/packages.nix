{ pkgs, atomi, fenixpkgs, pkgs-2411 }:
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
      with pkgs;
      {
        inherit
          goreleaser
          go
          ;
      }
    );
    nix-2411 = (
      with pkgs-2411;
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
          pkgs-2411.openssl
        ]
        ++ pkgs.lib.optionals (pkgs.stdenv.isLinux && pkgs.stdenv.hostPlatform.isx86_64) [
          targets.x86_64-unknown-linux-musl.stable.rust-std
        ]
        ++ pkgs.lib.optionals (pkgs.stdenv.isLinux && pkgs.stdenv.hostPlatform.isAarch64) [
          targets.aarch64-unknown-linux-musl.stable.rust-std
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs-2411.darwin.Security
          pkgs-2411.darwin.libiconv
        ]);
      }
    );
  };
in
with all;
atomipkgs //
fenix //
nix-2411 //
nix-unstable
