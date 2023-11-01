{
  description = "Tools for working with YANG data in Nix";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }: {
    overlays.default = final: prev: rec {
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
      default = nix-yang-tools;
    };
  } //
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = builtins.attrValues self.overlays;
      };
    in {
      packages = {
        inherit (pkgs) nix-yang-tools default;
      };
    });
}
