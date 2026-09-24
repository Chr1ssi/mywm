# mywm

mywm ist ein experimenteller, in Rust geschriebener Windowmanager für
[River](https://codeberg.org/river/river) 0.4. River übernimmt das Wayland-
Compositing und die Ein-/Ausgabe; mywm implementiert Fensteranordnung,
Workspaces, Tastenkürzel, Regeln und Sitzungssteuerung über Rivers
`river-window-management-v1`-Protokoll.

Das Projekt richtet sich derzeit an eine persönliche, monitorübergreifende
Desktop-Umgebung. Es ist benutzbar und als `v0.1.0` veröffentlicht, aber noch
nicht als universeller oder stabiler Desktop gedacht.

## Funktionen

- horizontales Scrolling-Layout mit individuell skalierbaren Spalten
- Floating-Fenster mit Mausverschiebung und Größenänderung
- unabhängige Workspaces pro Monitor sowie feste Monitorzuordnungen
- Hotplug-Unterstützung ohne Verlust der Workspace-Zuordnung
- konfigurierbare Tastenkürzel, Programmstarter und Autostart-Befehle
- Fensterregeln für App-ID, Dialoge, Workspace und Floating-Modus
- Vollbild für Wayland- und Xwayland-Anwendungen
- reservierbarer Gaming-Workspace mit automatischer App-Zuordnung
- Live-Reload für den größten Teil der TOML-Konfiguration
- Gaps, Fokusrahmen und eine gemeinsame Farbpalette
- Quickshell-Oberfläche mit Bar, Launcher, Wallpaper-Picker, Audio,
  Mediensteuerung, Systemtray und Benachrichtigungen
- Sperrbildschirm, Idle-Timer, Monitor-Standby und Sitzungsende
- Nix-Flake mit Paket, Overlay und NixOS-Modul

## Abhängigkeiten

### Laufzeit

| Komponente | Verwendung | Erforderlich |
| --- | --- | --- |
| River 0.4 oder neuer | Wayland-Compositor und Protokollserver | ja |
| Kanshi | Monitorkonfiguration | ja, für das mitgelieferte Startskript |
| Quickshell (getestet mit 0.3.1) | Bar, Launcher und Wallpaper | ja, für die mitgelieferte Oberfläche |
| `libxkbcommon` / `xkbcli` | Validierung und Kompilierung der Tastaturbelegung | ja |
| swaylock | Sperrbildschirm | ja, für das mitgelieferte Startskript |
| swayidle | Idle- und Suspend-Ereignisse | ja, für das mitgelieferte Startskript |
| wlopm | Monitor-Standby | ja, für das mitgelieferte Startskript |
| D-Bus und systemd | Sitzungsumgebung, Portale und Power-Aktionen | empfohlen |
| `xdg-desktop-portal`, GTK- und wlr-Backend | Dateiauswahl und Bildschirmfreigabe | empfohlen |
| Zenity | Auswahl einer Quelle bei Bildschirmfreigabe | optional |
| PipeWire und WirePlumber | Audioanzeige und Audiosteuerung der Bar | optional |
| Xwayland | X11-Anwendungen und viele Spiele | optional |
| Kitty | voreingestellter Terminalemulator | austauschbar |

Das NixOS-Modul installiert und konfiguriert die für die mitgelieferte Sitzung
benötigten Komponenten. Eigene Startskripte können einzelne Integrationen
ersetzen oder weglassen.

### Bauen aus dem Quellcode

- Rust mit Cargo und Edition-2024-Unterstützung
- River 0.4 einschließlich der XML-Protokolle unter
  `/usr/share/river-protocols/stable/`
- eine C-Toolchain für native Rust-Abhängigkeiten

Das separate Repository
[`mywm-shell`](https://github.com/Chr1ssi/mywm-shell) wird nur für die
Quickshell-Oberfläche benötigt. Für lokale Entwicklung werden beide Repositories
standardmäßig nebeneinander erwartet.

## Installation

### NixOS

Die Flake exportiert `packages.default`, `packages.mywm`, `overlays.default` und
`nixosModules.default`.

```nix
{
  inputs.mywm = {
    url = "github:Chr1ssi/mywm";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs: {
    nixosConfigurations.hostname = inputs.nixpkgs.lib.nixosSystem {
      modules = [
        inputs.mywm.nixosModules.default
        { programs.mywm.enable = true; }
      ];
    };
  };
}
```

Das Modul installiert mywm und River, registriert die Sitzung beim
Display-Manager und richtet Xwayland, swaylock sowie die GTK-/wlr-Portale ein.
Ein alternatives Paket lässt sich über `programs.mywm.package` auswählen.

Das Paket kann unabhängig vom Modul gebaut werden:

```sh
nix build github:Chr1ssi/mywm
```

### Aus dem Quellcode

```sh
git clone https://github.com/Chr1ssi/mywm.git
git clone https://github.com/Chr1ssi/mywm-shell.git
cd mywm
cargo build --release
```

Anschließend kann River aus einer TTY gestartet werden:

```sh
MYWM_BINARY="$PWD/target/release/mywm" river -c "$PWD/scripts/river-init"
```

Das Startskript verwaltet Kanshi, mywm, swayidle, Bar und Wallpaper als eine
Sitzung. Endet einer der Kernprozesse, werden die übrigen Prozesse aufgeräumt.
Es muss innerhalb von River mit gesetztem `WAYLAND_DISPLAY` laufen.

Für einen Login-Manager kann `config/mywm.desktop` als Vorlage verwendet werden.
Der darin enthaltene Projektpfad muss vor der Installation angepasst werden.
Das NixOS-Modul erzeugt den Desktop-Eintrag automatisch und benötigt diese
manuelle Anpassung nicht.

## Konfiguration

mywm lädt die erste vorhandene Konfiguration in dieser Reihenfolge:

1. den Pfad aus `MYWM_CONFIG`, falls gesetzt
2. `$XDG_CONFIG_HOME/mywm/config.toml`
3. `~/.config/mywm/config.toml`
4. die mitgelieferte Beispielkonfiguration beim Start über `river-init`
5. eingebaute Standardwerte beim direkten Start des Binaries

Eine vollständige Beispielkonfiguration liegt unter
[`config/mywm.toml`](config/mywm.toml). Die wichtigsten Bereiche sind:

```toml
workspaces = 9
terminal = ["kitty"]
wallpaper_directory = "/home/user/Pictures/Wallpapers"
float_dialogs = true

[workspace_outputs]
DP-1 = [1, 2, 3]
HDMI-A-1 = [4, 5, 6]

[keyboard]
layout = "de"
variant = ""
options = ""

[appearance]
gaps_inner = 4
gaps_outer = 4
border_width = 2
active_border = "#89b4fa"
inactive_border = "#45475a"

[bindings]
reload = ["Super+Shift+r"]
terminal = ["Super+Return"]
launcher = ["Super+Space"]
```

`workspace_outputs` ist optional. Wenn es gesetzt ist, muss jede Workspace-
Nummer genau einmal einem Ausgang zugeordnet sein. Ohne die Tabelle stehen die
Workspaces auf jedem Monitor unabhängig zur Verfügung.

Die mitgelieferte Kanshi-Datei unter `config/kanshi.conf` enthält eine
rechnerspezifische Monitoranordnung und sollte für das eigene System ersetzt
werden. Das NixOS-Modul erwartet sie unter
`$XDG_CONFIG_HOME/kanshi/config`; die mywm-Konfiguration liegt unter
`$XDG_CONFIG_HOME/mywm/config.toml`.

### Fensterregeln

Regeln werden beim ersten Einordnen eines Fensters angewandt. App-IDs werden im
mywm-Log ausgegeben und exakt, einschließlich Groß-/Kleinschreibung, verglichen.
Bei mehreren passenden Regeln gewinnt die letzte Angabe je Eigenschaft.

```toml
[[rules]]
app_id = "firefox"
workspace = 2

[[rules]]
app_id = "org.gnome.Calculator"
floating = true

[[rules]]
dialog = true
floating = true
```

### Standard-Tastenkürzel

| Tastenkürzel | Aktion |
| --- | --- |
| Super + Return | Terminal starten |
| Super + Space | App-Launcher öffnen |
| Super + Escape | Sitzung sperren |
| Super + Shift + W | Wallpaper-Picker öffnen |
| Super + H/L oder Pfeil links/rechts | Fenster fokussieren |
| Super + Shift + H/L | gekacheltes Fenster verschieben |
| Super + V | Floating-Modus umschalten |
| Super + linke Maustaste | Floating-Fenster verschieben |
| Super + rechte Maustaste | Fenster beziehungsweise Spalte skalieren |
| Super + Q | fokussiertes Fenster schließen |
| Super + 1…9 | Workspace wählen |
| Super + Shift + 1…9 | Fenster auf einen Workspace verschieben |
| Super + Ctrl + Pfeiltasten | Workspace des aktuellen Monitors wechseln |
| Super + M | Sitzung beenden |

Alle Bindings können geändert oder mit einer leeren Liste deaktiviert werden.
Zusätzliche Programme lassen sich unter `[program_bindings.<name>]` definieren.
Befehle werden als Argumentlisten ohne Shell-Auswertung ausgeführt.

### Live-Reload

`Super+Shift+R` validiert und lädt die Konfiguration neu. Tastenkürzel,
Programmbindings, Terminal, Launcher, Darstellung, Wallpaper-Verzeichnis und
Regeln werden live übernommen. Workspace-Anzahl und -Zuordnung,
Tastaturbelegung, Idle-Zeiten und Autostart benötigen einen Sitzungsneustart.
Bei einer ungültigen Datei bleibt die bisherige Konfiguration aktiv.

## Oberfläche und Sitzungsintegration

Die Quickshell-Oberfläche wird getrennt in `mywm-shell` entwickelt und von der
Nix-Flake in das Paket eingebunden. Bei einem manuellen Build sucht mywm im
benachbarten Verzeichnis `../mywm-shell/quickshell`. Ein anderer Ort kann über
`MYWM_SHELL_DIR` angegeben werden.

Die Oberfläche umfasst:

- Bar mit Workspaces, Uhr, Audio, Medien, Tray und Power-Menü
- App-Launcher auf Basis installierter Desktop-Einträge
- Notification-Daemon mit Toasts und Verlauf
- ein gemeinsames Wallpaper über die gesamte Monitoranordnung
- Wallpaper-Picker mit Suche und Tastatursteuerung

Es darf kein zweiter Freedesktop-Notification-Daemon wie Dunst parallel laufen.

Für Dateiauswahl verwendet die Sitzung das GTK-Portal. Bildschirmfreigabe und
Screenshots laufen über `xdg-desktop-portal-wlr`; Zenity dient als grafischer
Quellen-Chooser. `scripts/session-environment` überträgt dafür die aktuelle
Wayland-Umgebung an D-Bus und systemd.

## Entwicklung

Die grundlegenden Prüfungen sind:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Die Protokolltests starten das gebaute Binary gegen isolierte Wayland-Peers:

```sh
python3 tests/river_protocol.py
python3 tests/rules_protocol.py
python3 tests/layer_shell_protocol.py
python3 tests/appearance_protocol.py
python3 tests/session_protocol.py
```

Weitere Smoke-Tests verwenden eine verschachtelte River-Sitzung und teilweise
Quickshell, PipeWire, GTK 3, Xwayland oder Grim. Sie testen tatsächliches
Rendering und die Integration mit `mywm-shell`; Hardwareverhalten wie echte
Monitormodi, Suspend und Passwortentsperrung muss weiterhin manuell geprüft
werden.

Die Nix-Ausgaben lassen sich mit folgenden Befehlen prüfen:

```sh
nix flake check
nix build .#mywm
```

## Bekannte Einschränkungen

- Die Konfiguration und Oberfläche sind noch stark auf den ursprünglichen
  Desktop-Aufbau ausgerichtet.
- Es wird nur ein Seat verwaltet.
- Workspace-Zustand wird noch nicht über Sitzungsneustarts hinweg gespeichert.
- Floating-Fenster können noch nicht mit der Maus zwischen Monitoren gezogen
  werden.
- Es gibt noch keine Animationen, gestapelten Spalten oder Tastenwiederholung.
- mywm zeichnet Fokusrahmen, aber keine eigenen Titelleisten.

Fehlerberichte und fokussierte Beiträge sind willkommen. Wegen des frühen
Projektstands sollten größere Architekturänderungen vorab in einem Issue
abgestimmt werden.
