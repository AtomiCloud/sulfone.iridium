{ pkgs, pkgs-2305, packages }:
(pkgs.makeRustPlatform {
  cargo = packages.rust;
  rustc = packages.rust;
}).buildRustPackage {
  pname = "cyanprint";
  version = "1.4.0"; # replace
  src = ../.;
  nativeBuildInputs = [ pkgs-2305.perl pkgs-2305.pkgconfig ];
  buildInputs = ([
    pkgs-2305.pkgconfig
    pkgs-2305.openssl
  ] ++ (if pkgs-2305.stdenv.isDarwin then [
    pkgs-2305.darwin.Security
    pkgs-2305.darwin.apple_sdk.frameworks.Security
    pkgs-2305.darwin.apple_sdk.frameworks.SystemConfiguration
  ] else [ ]));

  cargoLock.lockFile = ../Cargo.lock;
}
