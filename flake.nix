{
  inputs = {
    # util
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";

    # rust
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # registry
    nixpkgs.url = "nixpkgs/d816b5ab44187a2dd84806630ce77a733724f95f";
    nixpkgs-2305.url = "nixpkgs/nixos-23.05";
    nixpkgs-sep-04-23.url = "nixpkgs/d816b5ab44187a2dd84806630ce77a733724f95f";
    nixpkgs-nov-07-23.url = "nixpkgs/85f1ba3e51676fa8cc604a3d863d729026a6b8eb";
    atomipkgs.url = "github:kirinnee/test-nix-repo/v22.2.0";
    atomipkgs_classic.url = "github:kirinnee/test-nix-repo/classic";
  };
  outputs =
    { self

      # utils
    , flake-utils
    , treefmt-nix
    , pre-commit-hooks

    , fenix

      # registries
    , atomipkgs
    , atomipkgs_classic
    , nixpkgs
    , nixpkgs-2305
    , nixpkgs-sep-04-23
    , nixpkgs-nov-07-23

    } @inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        pkgs-2305 = nixpkgs-2305.legacyPackages.${system};
        pkgs-sep-04-23 = nixpkgs-sep-04-23.legacyPackages.${system};
        pkgs-nov-07-23 = nixpkgs-nov-07-23.legacyPackages.${system};
        atomi = atomipkgs.packages.${system};
        fenixpkgs = fenix.packages.${system};
        atomi_classic = atomipkgs_classic.packages.${system};
        pre-commit-lib = pre-commit-hooks.lib.${system};
      in
      let
        out = rec {
          pre-commit = import ./nix/pre-commit.nix {
            inherit pre-commit-lib formatter packages;
          };
          formatter = import ./nix/fmt.nix {
            inherit treefmt-nix pkgs;
          };
          default = import ./nix/default.nix {
            inherit
              pkgs
              pkgs-2305
              packages;
          };
          packages = import ./nix/packages.nix
            {
              inherit
                pkgs
                atomi
                atomi_classic
                fenixpkgs
                pkgs-2305
                pkgs-sep-04-23
                pkgs-nov-07-23;
            } // { default = default; };
          env = import ./nix/env.nix {
            inherit pkgs packages;
          };
          devShells = import ./nix/shells.nix {
            inherit pkgs env packages;
            shellHook = checks.pre-commit-check.shellHook;
          };
          checks = {
            pre-commit-check = pre-commit;
            format = formatter;
          };
        };
      in
      with out;
      {
        inherit checks formatter packages devShells;
      }
      );
}
