{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-lang.url = "github:mlabs-haskell/flake-lang.nix";
    pre-commit-hooks.url = "github:cachix/git-hooks.nix";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
      ];

      imports = [
        ./helper-proc-macros/build.nix
        ./stupid-dbg/build.nix
        ./pre-commit.nix
        ./settings.nix
      ];

      perSystem = { config, ... }:
        {
          packages.default = config.packages.stupid-dbg-rust;
          devShells.default = config.devShells.dev-pre-commit;
        };
    };
}
