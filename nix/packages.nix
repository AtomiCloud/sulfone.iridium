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
          pkgs-2411.openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
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
