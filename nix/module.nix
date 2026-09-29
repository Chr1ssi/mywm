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
    cat > $out/share/wayland-sessions/mywm.desktop <<EOF
    [Desktop Entry]
    Name=mywm (River)
    Comment=River mit mywm und eigener Quickshell-Oberfläche
    Exec=${session}/bin/mywm-session
    Type=Application
    DesktopNames=river
    Keywords=tiling;wayland;compositor;
    EOF
  '';
in
{
  options.programs.mywm = {
    enable = lib.mkEnableOption "the mywm River session";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "inputs.mywm.packages.\${pkgs.stdenv.hostPlatform.system}.default";
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

    # Started by scripts/session-environment; pulls in graphical-session.target
    # so systemd user services bound to it run inside the River session.
    systemd.user.targets.mywm-session = {
      description = "mywm compositor session";
      bindsTo = [ "graphical-session.target" ];
      wants = [ "graphical-session-pre.target" ];
      after = [ "graphical-session-pre.target" ];
      before = [ "graphical-session.target" ];
    };
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
        chooser_type = lib.mkDefault "dmenu";
        chooser_cmd = lib.mkDefault "${pkgs.zenity}/bin/zenity --list --title='Bildschirm oder Fenster freigeben' --column='Quelle' --width=800 --height=500";
      };
    };
  };
}
