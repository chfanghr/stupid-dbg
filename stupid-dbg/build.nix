{ inputs, ... }: {
  perSystem = { system, config, ... }:
    let
      rustFlake = inputs.flake-lang.lib.${system}.rustFlake {
        src = ./.;
        crateName = "stupid-dbg";
        devShellHook = config.settings.defaultShellHook;
      };
    in
    {
      inherit (rustFlake) packages checks devShells;
    };
}
