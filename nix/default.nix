{ pkgs, pkgs-2505, pkgs-unstable, packages }:

let buildingPkg = if pkgs.stdenv.isLinux then pkgs.pkgsStatic else pkgs; in
(buildingPkg.makeRustPlatform {
  cargo = packages.rust;
  rustc = packages.rust;
}).buildRustPackage {
  pname = "cyanprint";
  version = "2.3.0"; # replace
  src = ../.;
  nativeBuildInputs = [ pkgs-2505.perl ];
  buildInputs = ([
    pkgs-2505.openssl
  ]);

  cargoLock.lockFile = ../Cargo.lock;
}
