{ inputs, ... }: {
  perSystem = { system, config, ... }:
    let
      rustFlake = inputs.flake-lang.lib.${system}.rustFlake {
        src = ./.;
        crateName = "stupid-dbg";
        devShellHook = config.settings.defaultShellHook;
        extraSources = [
          config.packages.helper-proc-macros-rust-src
        ];
      };
    in
    {
      inherit (rustFlake) packages checks devShells;
    };
}
