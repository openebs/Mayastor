{ stdenv
, lib
, pkgs
, git
, tag
, sourcer
, rustFlags
}:
let
  versionDrv = import ../../lib/version.nix { inherit lib stdenv git tag sourcer; };
  versions = {
    "version" = "${versionDrv}";
    "long" = builtins.readFile "${versionDrv.long}";
    "tag_or_long" = builtins.readFile "${versionDrv.tag_or_long}";
  };
  project-builder = { cargoBuildFlags ? [ ], pname }: pkgs.callPackage ./cargo-package.nix { inherit versions cargoBuildFlags rustFlags pname; };
  components = { build }: {
    io-engine = (project-builder { cargoBuildFlags = [ "--bin io-engine" ]; pname = "io-engine"; }).${build};
    io-engine-cli = (project-builder { cargoBuildFlags = [ "--bin io-engine-cli" ]; pname = "io-engine-client"; }).${build};
    casperf = (project-builder { cargoBuildFlags = [ "--bin casperf" ]; pname = "casperf"; }).${build};
    custom = { cargoBuildFlags }: (project-builder { cargoBuildFlags = [ cargoBuildFlags ]; pname = "io-engine"; }).${build};
  };
in
{
  cargoDeps = (project-builder { pname = ""; }).cargoDeps;
  release = components { build = "release"; };
  debug = components { build = "debug"; };
}
