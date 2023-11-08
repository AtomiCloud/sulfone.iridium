{ pkgs, atomi, atomi_classic, fenixpkgs, pkgs-2305, pkgs-sep-04-23, pkgs-nov-07-23 }:
let
  all = {
    atomipkgs_classic = (
      with atomi_classic;
      {
        inherit
          sg;
      }
    );
    atomipkgs = (
      with atomi;
      {
        inherit
          toml-cli
          cargo2junit
          infisical
          pls;
      }
    );
    nix-2305 = (
      with pkgs-2305;
      { }
    );
    fenix = (
      with fenixpkgs;
      {
        rust = with complete.toolchain; combine ([
          stable.cargo
          stable.rustc
          stable.rust-src
          stable.rust-std
          pkgs-2305.pkgconfig
          pkgs-2305.openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs-2305.darwin.Security
          pkgs-2305.darwin.libiconv
        ]);
      }
    );
    sep-04-23 = (
      with pkgs-sep-04-23;
      {
        inherit
          coreutils
          findutils
          sd
          bash;
      }
    );
    nov-07-23 = (
      with pkgs-nov-07-23;
      {
        inherit
          git
          go
          goreleaser
          nfpm
          gnused
          # lint
          treefmt
          gitlint
          shellcheck;
        npm = nodePackages.npm;
        nodejs = nodejs_20;
      }
    );
  };
in
with all;
atomipkgs //
atomipkgs_classic //
fenix //
nix-2305 //
sep-04-23 //
nov-07-23
