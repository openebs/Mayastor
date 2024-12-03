{ rust ? "none"
, spdk ? "develop"
, spdk-path ? null
} @ args:
import ./shell.nix {
  inherit rust;
  inherit spdk;
  inherit spdk-path;
}
