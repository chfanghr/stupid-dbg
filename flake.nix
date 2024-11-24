{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-lang.url = "github:mlabs-haskell/flake-lang.nix";
    git-hooks-nix.url = "github:cachix/git-hooks.nix";
  };

  outputs = inputs@{ flake-parts, flake-lang, git-hooks-nix, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } ({ lib, ... }: {
      systems = [
        "x86_64-linux"
      ];

      imports = [
        git-hooks-nix.flakeModule
      ];

      perSystem = { system, config, ... }:
        let
          rustFlake = flake-lang.lib.${system}.rustFlake {
            src = ./.;
            crateName = "stupid-dbg";
            devShellHook = ''
              export LC_CTYPE=C.UTF-8
              export LC_ALL=C.UTF-8
              export LANG=C.UTF-8
              ${config.pre-commit.installationScript}
            '';
          };

          inherit (lib) mkMerge;
        in
        mkMerge [
          {
            pre-commit.settings.hooks = {
              nixpkgs-fmt.enable = true;
              deadnix.enable = true;
              rustfmt.enable = true;
              typos.enable = true;
            };

            inherit (rustFlake) packages checks devShells;
          }
          {
            packages.default = rustFlake.packages.stupid-dbg-rust;
          }
        ];
    });
}
