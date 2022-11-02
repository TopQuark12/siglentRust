{ 
  mozillaOverlay ? builtins.fetchTarball "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz",
  latestRustNightly ? false,
}:
let
  pkgs = import <nixpkgs> {
    overlays = [ (import mozillaOverlay) ];
  };
  rust = pkgs.rustChannelOfTargets "nightly" null [ "x86_64-unknown-linux-gnu" ];
in
pkgs.mkShell {
  name = "thermostat-env";
  nativeBuildInputs = with pkgs; [cmake pkg-config freetype expat fontconfig];
  buildInputs = with pkgs; [
    rust
    freetype fontconfig
  ] ++ (with python3Packages; [
    numpy matplotlib
  ]);

}
