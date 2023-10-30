{
  description = "Tools for working with YANG data in Nix";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.simpleFlake {
      inherit self nixpkgs;
      name = "nix-yang-tools";
      overlay = final: prev: {
        nix-yang-tools = rec {
          nix-yang-tools = final.callPackage (
            { rustPlatform, libyang }:

            rustPlatform.buildRustPackage {
              pname = "nix-yang-tools";
              version = self.shortRev or "dirty-${toString self.lastModifiedDate}";

              src = self;

              NIX_LDFLAGS = "-L ${libyang}/lib";

              cargoLock.lockFile = ./Cargo.lock;
            }
          ) {};
          defaultPackage = nix-yang-tools;
        };
      };
    };
}
