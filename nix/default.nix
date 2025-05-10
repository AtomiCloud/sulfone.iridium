{ pkgs, pkgs-2411, packages }:

let buildingPkg = if pkgs-2411.stdenv.isLinux then pkgs.pkgsStatic else pkgs; in
(buildingPkg.makeRustPlatform {
  cargo = packages.rust;
  rustc = packages.rust;
}).buildRustPackage {
  pname = "cyanprint";
  version = "2.1.0"; # replace
  src = ../.;
  nativeBuildInputs = [ pkgs-2411.perl ];
  buildInputs = ([
    pkgs-2411.openssl
  ] ++ (if pkgs-2411.stdenv.isDarwin then [
    pkgs-2411.darwin.Security
    pkgs-2411.darwin.apple_sdk.frameworks.Security
    pkgs-2411.darwin.apple_sdk.frameworks.SystemConfiguration
  ] else [ ]));

  cargoLock.lockFile = ../Cargo.lock;
}
