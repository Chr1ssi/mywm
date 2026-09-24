{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.mywm;

  sessionEnvironment = pkgs.writeShellScript "mywm-session-environment"
    (builtins.readFile ../scripts/session-environment);

  session = pkgs.writeShellApplication {
    name = "mywm-session";
    runtimeInputs = with pkgs; [
      river
      cfg.package
      kanshi
      quickshell
      swaylock
      swayidle
      wlopm
      dbus
      systemd
      coreutils
      bash
      zenity
    ];
    text = ''
      export XDG_CURRENT_DESKTOP=river
      export XDG_SESSION_DESKTOP=mywm
      export XDG_SESSION_TYPE=wayland
      export MYWM_BINARY=${cfg.package}/bin/mywm
      export MYWM_SHELL_DIR=${cfg.package}/share/mywm/quickshell
      export MYWM_POLKIT_AGENT=${pkgs.polkit_gnome}/libexec/polkit-gnome-authentication-agent-1
      export MYWM_SESSION_ENVIRONMENT=${sessionEnvironment}
      export MYWM_CONFIG="''${XDG_CONFIG_HOME:-$HOME/.config}/mywm/config.toml"
      export MYWM_MONITOR_CONFIG="''${XDG_CONFIG_HOME:-$HOME/.config}/kanshi/config"

      exec river -c ${cfg.package}/share/mywm/scripts/river-init
    '';
  };

  sessionPackage = pkgs.runCommand "mywm-wayland-session" {
    passthru.providedSessions = [ "mywm" ];
  } ''
    mkdir -p $out/share/wayland-sessions
    substitute ${../config/mywm.desktop} $out/share/wayland-sessions/mywm.desktop \
      --replace-fail "/usr/bin/env XDG_CURRENT_DESKTOP=river XDG_SESSION_DESKTOP=mywm XDG_SESSION_TYPE=wayland /usr/bin/river -c /home/chris/Projects/mywm/scripts/river-init" \
      "${session}/bin/mywm-session"
  '';
in
{
  options.programs.mywm = {
    enable = lib.mkEnableOption "the mywm River session";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "inputs.mywm.packages.\${pkgs.system}.default";
      description = "The mywm package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [{
      assertion = lib.versionAtLeast pkgs.river.version "0.4";
      message = "mywm requires River >= 0.4 (not river-classic).";
    }];

    environment.systemPackages = [ session cfg.package pkgs.river ];
    services.displayManager.sessionPackages = [ sessionPackage ];
    security.pam.services.swaylock = { };
    programs.xwayland.enable = true;

    xdg.portal = {
      enable = true;
      wlr.enable = true;
      extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
      config.river = {
        default = [ "gtk" ];
        "org.freedesktop.impl.portal.FileChooser" = [ "gtk" ];
        "org.freedesktop.impl.portal.ScreenCast" = [ "wlr" ];
        "org.freedesktop.impl.portal.Screenshot" = [ "wlr" ];
      };
      wlr.settings.screencast = {
        chooser_type = "dmenu";
        chooser_cmd = "${pkgs.zenity}/bin/zenity --list --title='Bildschirm oder Fenster freigeben' --column='Quelle' --width=800 --height=500";
      };
    };
  };
}
