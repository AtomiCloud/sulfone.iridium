{ pkgs, pkgs-2411, packages }:
(pkgs.makeRustPlatform {
  cargo = packages.rust;
  rustc = packages.rust;
}).buildRustPackage {
  pname = "cyanprint";
  version = "1.8.0"; # replace
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
