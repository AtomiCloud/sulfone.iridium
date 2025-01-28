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
    nixpkgs.url = "nixpkgs/nixos-unstable";
    nixpkgs-2411.url = "nixpkgs/nixos-24.11";
    atomipkgs.url = "github:AtomiCloud/nix-registry/v1";
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
    , nixpkgs
    , nixpkgs-2411

    } @inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        pkgs-2411 = nixpkgs-2411.legacyPackages.${system};
        atomi = atomipkgs.packages.${system};
        fenixpkgs = fenix.packages.${system};
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
              pkgs-2411
              packages;
          };
          packages = import ./nix/packages.nix
            {
              inherit
                pkgs
                atomi
                fenixpkgs
                pkgs-2411;
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
