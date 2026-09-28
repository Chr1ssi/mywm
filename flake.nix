{
  description = "mywm – a custom River window manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    mywm-shell = {
      url = "github:Chr1ssi/mywm-shell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, mywm-shell, ... }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      packageFor = pkgs: pkgs.callPackage ./nix/package.nix {
        shellSrc = mywm-shell;
      };
    in
    {
      packages = forAllSystems (system:
        let package = packageFor nixpkgs.legacyPackages.${system};
        in {
          default = package;
          mywm = package;
        });

      checks = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          package = packageFor pkgs;
          dbusDaemon = pkgs.writeShellScript "mywm-test-dbus-daemon" ''
            args=()
            for arg in "$@"; do
              [[ "$arg" == --session ]] || args+=("$arg")
            done
            exec ${pkgs.dbus}/bin/dbus-daemon \
              --config-file=${pkgs.dbus}/share/dbus-1/session.conf "''${args[@]}"
          '';
          testEnvironment = with pkgs; [
            dbus
            grim
            kanshi
            libxkbcommon
            pipewire
            python3
            quickshell
            river
            wayland-scanner
          ];
        in {
          protocol = pkgs.runCommand "mywm-protocol-tests" {
            nativeBuildInputs = testEnvironment;
          } ''
            export HOME="$TMPDIR/home"
            mkdir -p "$HOME"
            export MYWM_TEST_BINARY=${package}/bin/mywm
            export RIVER_PROTOCOL_DIR=${pkgs.river.src}/protocol
            export WAYLAND_PROTOCOL_FILE=${pkgs.wayland-scanner}/share/wayland/wayland.xml
            cp -r ${self} ./source
            chmod -R u+w source
            cd source
            for test in tests/*_protocol.py tests/launcher_command.py; do
              python3 "$test"
            done
            touch "$out"
          '';

          shell = pkgs.runCommand "mywm-shell-smoke-tests" {
            nativeBuildInputs = testEnvironment;
          } ''
            export HOME="$TMPDIR/home"
            mkdir -p "$HOME"
            cp -r ${mywm-shell} ./shell-source
            chmod -R u+w shell-source
            export MYWM_BINARY=${package}/bin/mywm
            export MYWM_SOURCE_DIR=${self}
            cd shell-source
            dbus-run-session --dbus-daemon=${dbusDaemon} -- bash -c '
              python3 tests/launcher_smoke.py
              python3 tests/bar_smoke.py
              python3 tests/wallpaper_smoke.py
            '
            touch "$out"
          '';
        });

      overlays.default = final: _prev: {
        mywm = packageFor final;
      };

      nixosModules.default = import ./nix/module.nix { inherit self; };
      nixosModules.mywm = self.nixosModules.default;
    };
}
