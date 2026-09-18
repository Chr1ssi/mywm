# mywm

Ein experimenteller Windowmanager in Rust für River 0.4 und dessen
`river-window-management-v1`-Protokoll. River übernimmt das Compositing,
mywm die Fensteranordnung und Tastenkürzel.

## Layout und Bedienung

Die Projektkonfiguration verteilt neun feste Workspaces auf drei Monitore, jeweils mit einem horizontalen Fensterstreifen:

- Ein Fenster nutzt die gesamte Monitorfläche.
- Ab zwei Fenstern bekommt jedes die halbe Monitorbreite und volle Höhe.
- Neue Fenster werden rechts angehängt und fokussiert.
- Beim Fokuswechsel scrollt der Ausschnitt nur so weit wie nötig.
- Navigation endet am ersten bzw. letzten Fenster und wechselt keinen Monitor.
- Jeder Workspace behält seine Fensterreihenfolge, seinen Fokus und Scrollversatz. Neue Fenster erscheinen
  auf dem Monitor unter dem Mauszeiger, ersatzweise auf dem aktiven/ersten Monitor.

| Tastenkürzel | Aktion |
| --- | --- |
| Super + Return | Kitty starten |
| Super + Escape | Sitzung sperren |
| Super + Shift + W | Wallpaper-Picker öffnen |
| Super + Space | Quickshell-App-Launcher öffnen |
| Super + H / Pfeil links | Linkes Fenster fokussieren |
| Super + L / Pfeil rechts | Rechtes Fenster fokussieren |
| Super + Shift + H / Pfeil links | Fenster nach links verschieben |
| Super + Shift + L / Pfeil rechts | Fenster nach rechts verschieben |
| Super + V | Fokussiertes Fenster zwischen Scrolling und Floating umschalten |
| Super + linke Maustaste | Floating-Fenster unter dem Mauszeiger verschieben |
| Super + rechte Maustaste | Floating-Fenster von der nächstgelegenen Ecke skalieren |
| Super + Q | Fokussiertes Fenster zum Schließen auffordern |
| Super + 1…9 | Globalen Workspace und dessen Monitor auswählen |
| Super + Shift + 1…9 | Fokussiertes Fenster auf den Ziel-Workspace verschieben, auch monitorübergreifend |
| Super + M | River-Sitzung einschließlich mywm beenden; zurück zur startenden TTY |

## Tastaturlayout

mywm konfiguriert alle Tastaturen über Rivers XKB-Schnittstelle, einschließlich
später angeschlossener Geräte. Standard ist Deutsch (QWERTZ):

```toml
[keyboard]
layout = "de"
variant = ""
options = ""
```

`variant` erlaubt beispielsweise `nodeadkeys`, `options` zusätzliche XKB-Optionen.
Die leere Variante verwendet das normale deutsche Layout mit Akzenttasten.
Änderungen gelten nach WM-Neustart. Zum Kompilieren der Keymap benötigt mywm
`xkbcli` (Arch/CachyOS: Paket `libxkbcommon`); ungültige Layouts werden vor dem
Verbindungsaufbau zu River abgewiesen. Die Einstellung gilt auch für den Launcher
und andere Anwendungen, unabhängig von deren Sprache.

## Gaps und Rahmen

In `config/mywm.toml` lassen sich Abstände und Fokusfarben einstellen:

```toml
[appearance]
gaps_inner = 8
gaps_outer = 8
border_width = 2
active_border = "#89b4fa"
inactive_border = "#45475a"
background = "#1e1e2e"
surface = "#313244"
text = "#cdd6f4"
muted_text = "#a6adc8"
```

Die Werte sind logische Pixel. `gaps_inner` ist der Abstand zwischen gekachelten
Fensterrahmen, `gaps_outer` der Abstand zur nutzbaren Monitorfläche (einschließlich
reservierter Bar-Flächen). Ein einzelnes gekacheltes Fenster nutzt die ganze
Fläche abzüglich Außenabstand und Rahmen. Ab zwei Fenstern passen zwei gleich
breite Rahmen mit dem konfigurierten Zwischenraum in den sichtbaren Ausschnitt.

Floating-Fenster erhalten ebenfalls Rahmen; Außen-/Innenabstände gelten für das
Scrolling-Layout. Ihre gespeicherte Geometrie bezeichnet die Fläche inklusive
Rahmen. Anwendungsseitige Titelleisten bleiben erhalten. Wenn eine Shell-Oberfläche
Tastaturfokus hat, zeigen alle Fenster die inaktive Rahmenfarbe.

Gaps erlauben Werte von 0 bis 128, Rahmenbreiten von 0 bis 32. Farben verwenden
`#RRGGBB`. Mit `0` lassen sich Gaps und Rahmen abschalten. Auf sehr kleinen
Flächen werden Abstände/Rahmen begrenzt, sodass die Inhaltsgröße positiv bleibt.
Änderungen gelten nach einem Neustart.

## Floating

`Super+V` löst ein Fenster aus dem Scrolling-Layout. Beim ersten Umschalten wird
es mit ungefähr zwei Dritteln der Monitorbreite und -höhe zentriert. Floating-
Fenster zählen nicht zur Spaltenaufteilung: Bleibt ein gekacheltes Fenster übrig,
bekommt es die volle Monitorfläche. Floating-Fenster liegen über gekachelten
Fenstern; das zuletzt fokussierte Floating-Fenster liegt oben.

Verschieben und Skalieren mit der Maus funktionieren nur für Floating-Fenster.
Auch entsprechende Anfragen von anwendungseigenen Titelleisten und Fensterkanten
werden unterstützt. Die rechte Maustaste skaliert von der Ecke, die dem Zeiger
am nächsten liegt. Position und Größe bleiben auf den zugeordneten Monitor
begrenzt; Ziehen auf einen anderen Monitor ist noch nicht implementiert.

Floating-Fenster gehören weiterhin zu ihrem Workspace. Größe und Position werden
beim Wechseln und Verschieben zwischen Workspaces desselben Monitors sowie beim erneuten Umschalten
auf Floating wiederhergestellt (bei kleinerem Monitor begrenzt). `Super+H/L`
navigiert durch alle Fenster in Workspace-Reihenfolge. `Super+Shift+H/L` ordnet
nur gekachelte Fenster um. Beim Zurückschalten ins Scrolling-Layout bleibt die
bisherige Position im Fensterstreifen erhalten.

In `[bindings]` lässt sich `toggle_floating = ["Super+v"]` ändern.
`pointer_modifiers = "Super"` legt die Modifier für beide Mausaktionen fest.
Dialogfenster mit einem Elternfenster starten standardmäßig auf Floating; Rules können dies überschreiben.

## Workspaces und TOML-Konfiguration

Jeder Monitor zeigt unabhängig einen Workspace an. Beim Wechsel wird dessen
letztes fokussiertes Fenster wieder fokussiert; leere Workspaces erhalten keinen
Fensterfokus. `Super+Shift+1…9` verschiebt das fokussierte Fenster, ohne zum Ziel
zu wechseln. Auf dem Ziel wird es rechts angehängt und beim nächsten Wechsel
fokussiert. Neue Fenster erscheinen auf dem aktiven Workspace des Monitors
unter dem Mauszeiger. Ohne Mausposition wird der aktive/erste Monitor verwendet.

Die Projektkonfiguration ordnet 1–3 DP-3, 4–6 HDMI-A-1 und 7–9 DP-1 zu.
`Super+1…9` wählt den Workspace auf seinem zugeordneten Monitor; beim
Monitorwechsel folgt der Mauszeiger, damit neue Anwendungen dort erscheinen.
Die Bar zeigt jeweils nur die zugeordneten Nummern.

Wird ein Monitor entfernt, werden seine Workspaces auf den ersten verbleibenden
Monitor übernommen und behalten ihre Nummer. Mit konfigurierter Monitorzuordnung
wandern sie beim Wiederanschließen automatisch zurück. Wenn alle Monitore entfernt
werden, bleibt die Workspace-Zuordnung bis zum nächsten angeschlossenen Monitor
erhalten. Über einen Neustart hinweg wird der Sitzungszustand noch nicht gespeichert.

[`config/mywm.toml`](config/mywm.toml) enthält die bearbeitbaren Standardwerte.
Die Konfiguration wird beim Start geladen:

1. `MYWM_CONFIG`, falls gesetzt (ein falscher Pfad ist ein Fehler).
2. `$XDG_CONFIG_HOME/mywm/config.toml`, sonst `~/.config/mywm/config.toml`.
3. Ohne persönliche Datei verwendet das Startskript die Projektdatei;
   ein direkt gestartetes Binary verwendet eingebaute Standardwerte.

Beispiel für die globale Monitorzuordnung und zusätzliche Terminalargumente:

```toml
workspaces = 9
terminal = ["kitty", "--single-instance"]

[workspace_outputs]
DP-3 = [1, 2, 3]
HDMI-A-1 = [4, 5, 6]
DP-1 = [7, 8, 9]

[bindings]
focus_left = ["Super+h", "Super+Left"]
focus_right = ["Super+l", "Super+Right"]
workspace_modifiers = "Super"
move_to_workspace_modifiers = "Super+Shift"
```

Bei gesetztem `workspace_outputs` muss jede Workspace-Nummer genau einmal
zugeordnet sein. Ohne diese Tabelle stehen die Nummern wie bisher auf jedem
Monitor unabhängig zur Verfügung. Änderungen benötigen einen Sitzungsneustart.

Fehlende Werte behalten ihre Standards. Workspace-Anzahl: 1 bis 9. Tastenkürzel
verwenden `Super`, `Shift`, `Ctrl`/`Control` und `Alt`, kombiniert mit Buchstaben
(a–z), Ziffern (0–9), `Return`/`Enter`, `Left`, `Right`, `Up`, `Down`, `Space`,
`Tab` oder `Escape`/`Esc`. Die Schreibweise ist unabhängig von Groß-/Kleinschreibung;
für Shift muss ausdrücklich `Shift` angegeben werden. Leere Listen deaktivieren
eine Aktion. Workspace-Kürzel werden automatisch aus den Modifiern und den
Ziffern 1 bis zur konfigurierten Anzahl gebildet.

Unbekannte Optionen, doppelte Tastenkürzel und ungültige Werte brechen den Start
mit einer Fehlermeldung ab. Terminalargumente werden direkt übergeben, ohne
Shell-Auswertung. Änderungen gelten nach einem Neustart; Live-Reload ist noch
nicht implementiert. Physische Monitore werden weiterhin separat durch
`config/kanshi.conf` konfiguriert.

## Fensterregeln

Regeln stehen als `[[rules]]`-Blöcke in der TOML-Datei. Die mitgelieferte Datei
enthält auskommentierte Beispiele; es sind keine App-Zuordnungen voreingestellt.

```toml
[[rules]]
app_id = "firefox"
workspace = 2

[[rules]]
app_id = "org.gnome.Calculator"
floating = true

[[rules]]
app_id = "my.application"
dialog = true
floating = false
```

`app_id` vergleicht die vollständige App-ID, einschließlich Groß-/Kleinschreibung,
ohne Wildcards oder reguläre Ausdrücke. `dialog = true` trifft Fenster mit einem
Elternfenster, `false` solche ohne Elternfenster. Sind beide Selektoren angegeben,
müssen beide passen. Mindestens ein Selektor und eine Aktion sind erforderlich.
Bei mehreren passenden Regeln gewinnt die letzte Angabe **je Eigenschaft**;
ein fehlendes Feld überschreibt keine vorherige Angabe.

`workspace` verwendet Nummern von 1 bis zur konfigurierten Anzahl und bezieht sich
bei gesetztem `workspace_outputs` auf den dort zugeordneten Monitor, sonst auf
den bisherigen Zielmonitor. Ein Fenster auf einem inaktiven Workspace erscheint
im Hintergrund; die Regel wechselt weder Workspace noch Tastaturfokus.

Dialoge übernehmen zunächst Monitor und Workspace ihres Elternfensters. Die
Option `float_dialogs = true` auf oberster TOML-Ebene (vor `[bindings]`) aktiviert
das automatische Floating; `false` deaktiviert es. Regeln können sowohl Floating
als auch die geerbte Workspace-Nummer überschreiben. Beim Fokussieren eines
Dialogs scrollt das Layout dessen gekacheltes Elternfenster ebenfalls ins Bild.

Regeln werden **einmal beim ersten Einordnen** angewandt, mit den bis dahin
bekannten App-ID-/Parent-Angaben. Spätere App-ID-, Titel- oder Parent-Änderungen
lösen keine erneute Regelanwendung aus. Manuelle Änderungen mit Tastenkürzeln
bleiben dadurch erhalten. Fenster, deren Dialogbeziehung erst später bekannt
wird, können weiterhin mit `Super+V` umgeschaltet werden. Änderungen an der
Regeldatei gelten nach einem Neustart für neu eingeordnete Fenster.

mywm protokolliert empfangene App-IDs als `Window … app_id: …`; damit lässt sich
der exakte Bezeichner einer Anwendung ermitteln. Ungültige Workspace-Nummern,
unbekannte Optionen und Regeln ohne Selektor/Aktion werden beim Start abgewiesen.

## Bauen

Benötigt Rust/Cargo, River 0.4, Kitty und die River-Protokollbeschreibungen unter
`/usr/share/river-protocols/stable/`. Die XML-Dateien werden beim Build eingelesen.

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
python tests/river_protocol.py
python tests/rules_protocol.py
python tests/layer_shell_protocol.py
python tests/appearance_protocol.py
python tests/launcher_command.py
python ../mywm-shell/tests/launcher_smoke.py
python ../mywm-shell/tests/bar_smoke.py
python tests/session_protocol.py
python tests/session_smoke.py
python ../mywm-shell/tests/wallpaper_smoke.py
```

Der Python-Test verbindet das gebaute Binary mit einem isolierten Protokollpeer
und prüft die River-Anfragen für Sichtbarkeit, Fokus, Workspace-Wechsel,
Verschieben, Monitorwechsel, Floating, Mausoperationen und Sitzungsende. Er steuert keine Desktop-Sitzung;
Rendering und echte Tastatureingaben müssen weiterhin unter River geprüft werden.

Das Binary `target/debug/mywm` muss innerhalb einer River-Sitzung mit passendem
`WAYLAND_DISPLAY` laufen. Dort darf noch kein anderer Windowmanager verbunden
sein. Die Desktop-Startkonfiguration wird von diesem Projekt nicht verändert.

## Monitore und River starten

Die bearbeitbare Datei [`config/kanshi.conf`](config/kanshi.conf) enthält die
Monitoranordnung. Kanshi konfiguriert Rivers Ausgänge; mywm erhält deren neue
Positionen und Größen automatisch über das River-Protokoll.

| Ausgang | Native Auflösung / Bildrate | Position | Drehung |
| --- | --- | --- | --- |
| HDMI-A-1 (oben) | 2560×1080 @ 60 Hz | 0,0 | normal |
| DP-3 (Hauptmonitor, unten) | 2560×1440 @ 143,97 Hz | 0,1080 | normal |
| DP-1 (rechts) | 2560×1440 @ 59,95 Hz | 2560,0 | 270° |

Alle Monitore verwenden Skalierung 1. DP-1 hat nach der Drehung eine logische
Fläche von 1440×2560 und steht oben bündig mit HDMI-A-1. Die Bildraten und die
Drehrichtung stammen aus der vorhandenen Hyprland-Konfiguration. „Hauptmonitor“
bezeichnet hier die Anordnung; eine bevorzugte Start-Fokusauswahl ist noch nicht
implementiert.

Einmalig Kanshi installieren:

```sh
sudo pacman -S --needed kanshi
```

Danach im Projekt bauen und River aus einer TTY starten:

```sh
cargo build
river -c /home/chris/Projects/mywm/scripts/river-init
```

Xwayland ist aktiviert, damit unter anderem Steam verwendet werden kann.
Native Wayland-Anwendungen laufen weiterhin direkt unter Wayland. Für einen
Test ohne Xwayland kann River mit `-no-xwayland` gestartet werden.

Das Startskript startet Kanshi, mywm, swayidle, Quickshell-Bar und Wallpaper gemeinsam.
Endet Kanshi, mywm oder swayidle, werden die übrigen Prozesse beendet. Ein Fehler der Bar beendet
dagegen nicht die Fensterverwaltung. Rivers Beenden räumt alle Prozesse auf. Es prüft
zuvor, ob Binary und Konfiguration vorhanden sind. Bei einem Umzug des Projekts
muss der Pfad im River-Aufruf angepasst werden.

Zum Ändern des Layouts `position`, `mode`, `scale` oder `transform` in
`config/kanshi.conf` bearbeiten und die River-Sitzung neu starten. Das Profil
passt, wenn alle drei benannten Ausgänge angeschlossen sind. In einer
verschachtelten River-Sitzung unter Hyprland heißen die virtuellen Ausgänge
anders; dort wird dieses Hardwareprofil nicht angewandt. Ohne passendes Profil
bleibt Rivers bestehende Ausgangskonfiguration erhalten.

Optional akzeptiert das Startskript `MYWM_BINARY` und `MYWM_MONITOR_CONFIG` als
Umgebungsvariablen mit alternativen Dateipfaden, etwa für einen Release-Build.

## Start über den Login-Manager

Im Noctalia-Greeter (greetd) steht **mywm (River)** als Sitzung zur Verfügung.
Der installierte Eintrag `/usr/share/wayland-sessions/mywm.desktop` startet River
mit unserem `scripts/river-init`, damit WM, Monitore, Bar, Wallpaper und Idle
zusammen starten. Logout oder Super+M kehrt dann zum Login-Manager zurück.
Ein River-Start direkt aus einer TTY kehrt weiterhin zu dieser TTY zurück.

Die Vorlage liegt in `config/mywm.desktop`. Sie enthält den absoluten Projektpfad;
nach einem Umzug muss dieser angepasst und der Eintrag erneut installiert werden:

```sh
sudo install -o root -g root -m 644 config/mywm.desktop /usr/share/wayland-sessions/mywm.desktop
```

Der bestehende River-/Hyprland-Eintrag und die Standardauswahl werden nicht verändert.
Es wird das bereits gebaute `target/debug/mywm` verwendet; nach Codeänderungen
vor dem nächsten Login `cargo build` ausführen.

## Eigene Quickshell-Oberfläche

Ziel ist eine eigene Quickshell-Shell für Bar, Launcher, Benachrichtigungen und
Wallpaper. mywm übernimmt Fenster, Workspaces, Layout und Regeln.

Die Grundlage auf WM-Seite ist implementiert:

- `river-layer-shell-v1` wird gebunden, damit River Layer-Shell-Panels zulässt.
- Reservierte Panelbereiche begrenzen Scrolling, Floating, Clipping und Mausoperationen.
- Layer-Shell-Launcher können exklusiven oder bedarfsweisen Tastaturfokus erhalten.
  Beim Schließen wird der gemerkte Fensterfokus wiederhergestellt; auf leeren
  Workspaces wird der Fensterfokus gelöscht.
- Panels ohne expliziten Monitor erhalten den Monitor unter dem Mauszeiger bzw.
  ersatzweise den aktiven/ersten Monitor als Standard. Eine Bar sollte pro
  Quickshell-Screen eine eigene Panel-Instanz mit explizitem Screen erzeugen.

### Launcher

Quickshell 0.3.1 wird benötigt (`sudo pacman -S --needed quickshell`).
`Super+Space` öffnet den Launcher auf dem Monitor unter dem Mauszeiger.
Tippen filtert installierte Desktop-Anwendungen nach Namen, Beschreibung und
Schlüsselwörtern. Pfeiltasten oder Tab/Shift+Tab wählen aus; Enter oder ein
Mausklick startet die App. Escape schließt den Launcher. Mehrfaches Öffnen
erzeugt keine zusätzlichen Instanzen.

Terminal-Anwendungen verwenden den konfigurierten `terminal`-Befehl mit `-e`;
der Terminalemulator muss diese Option unterstützen. Argumente und Arbeitsordner
aus Desktop-Einträgen bleiben erhalten. Es findet keine Shell-Auswertung statt.

Die gemeinsame Palette steht in `[appearance]`: `background`, `surface`, `text`,
`muted_text`, dazu `active_border` als Akzentfarbe und `inactive_border` als
Trennfarbe. mywm übergibt diese Farben an Quickshell; `../mywm-shell/quickshell/Theme.qml`
bündelt sie für die Oberfläche und spätere Komponenten. Änderungen benötigen
aktuell einen WM-Neustart. Direkt gestartetes QML verwendet die Systempalette.

Der Launcher läuft nur während der Benutzung. Sein Standardpfad wird beim Build
aus dem Projektverzeichnis übernommen; nach einem Umzug neu bauen. Optional kann
auf oberster TOML-Ebene `launcher = ["qs", "-p", "/pfad/shell.qml", "--no-duplicate"]`
gesetzt werden. Unter `[bindings]` ist `launcher = ["Super+Space"]` das Tastenkürzel.

`tests/launcher_command.py` prüft Tastenkürzel und Übergabe von Palette, Terminal
und Monitorposition. `../mywm-shell/tests/launcher_smoke.py` startet eine isolierte Headless-
River-Sitzung mit echtem Quickshell und temporären Test-Anwendungen. Geprüft werden
Suche, Auswahl, Start inklusive Terminal und Arbeitsordner sowie Schließen und
Wiederöffnen. Optional erzeugt `MYWM_LAUNCHER_SCREENSHOT=/tmp/launcher.png`
mit installiertem `grim` einen Screenshot. Echte Tastatureingaben und die
Monitorwahl auf der Hardware sind weiterhin im normalen Testbetrieb zu prüfen.

### Topbar

Das River-Startskript startet `../mywm-shell/quickshell/bar.qml` automatisch auf jedem Monitor.
Die Bar reserviert oben 36 logische Pixel, damit Fenster sie nicht verdecken.

- Links: Workspaces dieses Monitors. Klick wechselt; die Akzentfarbe markiert den
  aktiven Workspace, ein Punkt kennzeichnet belegte Workspaces.
- Mitte: deutsches Datum und Uhrzeit im 24-Stunden-Format.
- Rechts: Lautstärkeregler (0–100 %) für den aktuellen PipeWire-Standardausgang;
  Klick auf die Prozentanzeige schaltet stumm. Ohne Ausgang ist die Bedienung deaktiviert.
- Power: Logout, Reboot und Shutdown, jeweils mit Bestätigung. Escape schließt
  das Menü. Logout beendet diese River-Sitzung über mywm; Reboot/Shutdown verwenden
  `systemctl reboot`/`systemctl poweroff`. Die Berechtigungen der Sitzung gelten;
  bei einem Fehler bleibt das Menü mit einer Fehlermeldung geöffnet.

Die Bar verwendet dieselbe TOML-Palette wie Launcher und WM. Innerhalb eines von
mywm gestarteten Terminals lässt sie sich mit `target/debug/mywm --bar` erneut
starten; `MYWM_SOCKET` muss auf die Sitzung zeigen. Das Startskript setzt diese
Variable automatisch. Mehrfachstarts erzeugen keine zweite Bar.

`../mywm-shell/tests/bar_smoke.py` prüft drei virtuelle Monitore mit je drei Workspaces, Workspace-Wechsel und
Wiederherstellung nach Bar-Neustart, Lautstärke/Stummschaltung auf einem separaten
PipeWire-Testserver sowie Logout in der isolierten River-Sitzung. Reboot/Shutdown
werden durch harmlose Testbefehle ersetzt und inklusive Bestätigung geprüft.
Der Test benötigt zusätzlich `pipewire`, `pw-metadata` und `pw-dump`.

Benachrichtigungen folgen.
Architektur und nächste Schritte: [docs/quickshell.md](docs/quickshell.md).

## Wallpaper und Picker

**Super+Shift+W** öffnet den Picker auf dem Monitor unter dem Mauszeiger.
Vorschaubilder lassen sich anklicken oder mit Pfeiltasten und Enter auswählen;
Tippen filtert Dateinamen, Escape schließt ohne Änderung. Das gemeinsame Theme
gilt auch für den Picker.

Die Sammlung wird auf oberster TOML-Ebene konfiguriert:

```toml
wallpaper_directory = "/home/chris/Bilder/Wallpaper"
```

JPG/JPEG, PNG, WebP und BMP im angegebenen Ordner werden angezeigt; Unterordner
werden nicht durchsucht. Bei jedem Öffnen wird die Liste aktualisiert. Die
Originalbilder bleiben unverändert. Vor der ersten Auswahl wird das erste Bild
in Dateinamen-Reihenfolge angezeigt.

Das Wallpaper ist **ein durchgehendes Bild über die gesamte Monitoranordnung**.
Quickshell berechnet deren gemeinsame Begrenzungsfläche in logischen Koordinaten
und skaliert das Bild proportional, bis diese vollständig gefüllt ist. Jeder
Monitor zeigt den Ausschnitt an seiner Position, einschließlich Höhenversatz,
Hochkant und negativer Koordinaten. Nicht von Monitoren bedeckte Bereiche zeigen
keinen Bildausschnitt; ein anderes Seitenverhältnis führt zu zentriertem Zuschnitt.
Physische Bildschirmränder werden dabei nicht kompensiert.

Die Auswahl wird atomar unter `$XDG_STATE_HOME/mywm/wallpaper.json` gespeichert
(standardmäßig `~/.local/state/mywm/wallpaper.json`) und beim Sitzungsstart geladen.
Nach Monitoränderungen wird die gemeinsame Fläche neu berechnet. Fehlt ein
gewähltes Bild später, zeigt der betroffene Hintergrund die Theme-Farbe; der
Picker bietet eine neue Auswahl an. Wallpaper und Picker laufen gemeinsam in
`../mywm-shell/quickshell/wallpaper.qml`, unabhängig von Bar und Launcher. Bei Bedarf lässt sich
der Prozess mit `target/debug/mywm --wallpaper` aus einem mywm-Terminal neu starten.

`../mywm-shell/tests/wallpaper_smoke.py` prüft Suche, Sonderzeichen, Speichern/Wiederherstellen
und den tatsächlichen gerenderten Bildverlauf auf versetzten/gedrehten virtuellen
Monitoren. Es schreibt ausschließlich temporäre Testbilder und Zustandsdateien.

## Sperrbildschirm und Idle

Benötigt `swaylock`, `swayidle` und `wlopm` (bereits installiert). Das Startskript
prüft diese Abhängigkeiten. **Super+Escape** oder **Power → Sperren** sperrt sofort.
Alternativ funktioniert in einem mywm-Terminal `target/debug/mywm --lock`.
Entsperrt wird mit dem Benutzerpasswort über swaylock/PAM; der WM und Quickshell
lesen keine Passwörter. Der Locker verwendet die gemeinsame Farbpalette.

```toml
[idle]
lock_after_seconds = 300
monitor_off_after_seconds = 600
```

Nach fünf Minuten Inaktivität wird gesperrt, nach insgesamt zehn Minuten werden
die Monitore über wlopm ausgeschaltet. Aktivität schaltet sie wieder ein; die
Sperre bleibt bestehen. `0` deaktiviert einen Timer. Monitor-Standby erfordert
einen früheren aktivierten Sperr-Timer. Beide Werte dürfen höchstens 86400 sein.
Änderungen gelten nach Sitzungsneustart. Automatisches Suspend ist nicht aktiviert.
Wayland-Idle-Inhibitoren, etwa von Videoplayern, werden von swayidle berücksichtigt.

Vor einem über logind ausgelösten Suspend und bei `loginctl lock-session` wird
ebenfalls gesperrt, unabhängig von den Idle-Timern. Nach Resume werden die Monitore
eingeschaltet. Der Sperrbefehl wartet auf Rivers bestätigtes `session_locked`;
Monitor-Standby läuft nur nach erfolgreicher Bestätigung. swayidle hält während
des before-sleep-Befehls einen logind-Delay-Inhibitor. Dessen systemseitiges Zeitlimit
gilt weiterhin: ein fehlgeschlagener oder zu langsamer Locker kann einen extern
angeforderten Suspend nicht dauerhaft verhindern.

Während der Sperre deaktiviert mywm seine Tastatur-/Mausbindings und weist
Workspace-/Logout-Befehle über IPC ab. Wiederholte Sperraufrufe erzeugen keine
weiteren Locker. Der Locker verwendet das echte Wayland-Session-Lock-Protokoll,
kein Vollbildfenster. Ein Locker-Absturz entsperrt die Sitzung nicht automatisch.

Die Tests prüfen echtes Sperren in einer separaten River-Sitzung, Idle-Abfolge,
Blockieren von Logout und Wiederherstellen der Bindings. Monitor-Power-Befehle
werden dort aufgezeichnet, und das Entsperren erfolgt ausschließlich im Test per
Signal an dessen eigenen Locker. Passwortentsperrung, echter Suspend/Resume und
Monitor-Standby auf der Hardware müssen daher einmal in deiner River-Sitzung
praktisch geprüft werden.

## Aktuelle Grenzen

Noch keine Animationen, konfigurierbaren Spaltenbreiten, gestapelten
Spalten oder Tastenwiederholung. Es wird ein Seat verwaltet. Reservierte
Panelbereiche werden über River Layer Shell berücksichtigt. Größen sind
Vorschläge an Anwendungen; Anwendungen dürfen davon
abweichen und werden auf ihre vorgesehene Fläche beschnitten (Protokollversion 2+).
Bei ungerader Monitorbreite bleibt bei zwei Fenstern ein Pixel frei.

### Titelleisten

mywm fordert serverseitige Dekorationen an und zeichnet nur die konfigurierten
Fokus-Ränder, keine Titelleisten. Das gilt für gekachelte und Floating-Fenster.
Anwendungen, die ausschließlich eigene Dekorationen unterstützen, können ihre
Titelleiste weiterhin anzeigen; sie muss gegebenenfalls in der Anwendung
deaktiviert werden. Die Änderung am WM benötigt einen Sitzungsneustart.

## Getrennte Repositories

Die Quickshell-Oberfläche liegt im Nachbar-Repository `../mywm-shell`.
mywm enthält Fensterverwaltung, River-Sitzungsstart, Konfiguration und den
[Integrationsvertrag](docs/quickshell.md). Die Palette bleibt zentral unter
`[appearance]`; Shell-Integrationstests liegen im Shell-Repository.

Standardmäßig sucht mywm neben seinem Build-Quellverzeichnis nach
`mywm-shell/quickshell`. Für andere Installationsorte vor dem Sitzungsstart
`MYWM_SHELL_DIR` auf den absoluten QML-Ordner setzen. Eine ausdrücklich gesetzte
`launcher`-Befehlsliste in TOML hat weiterhin Vorrang für den Launcher.

Der ignorierte Symlink `quickshell` hält alte Pfade für bereits laufende WM-
und Quickshell-Prozesse gültig. Nach einem vollständigen Sitzungsneustart mit
dem neu gebauten WM kann er entfernt werden. Er gehört nicht zum Repository.

## Vollbild

Vollbild-Anfragen von Anwendungen (z. B. YouTube, Browser-F11 oder Spiele)
werden auf dem Monitor ihres Workspaces umgesetzt, für Wayland und Xwayland.
River übernimmt dabei die gesamte Monitorfläche ohne Gaps, Fokus-Ränder oder
Topbar. Die normale Kachelung und Floating-Geometrie bleiben gespeichert.
Beim Verlassen von Vollbild werden sie wiederhergestellt. Ein Workspace- oder
Fensterfokuswechsel setzt die Vollbilddarstellung vorübergehend aus; bei der
Rückkehr wird sie wieder aktiv, sofern die Anwendung Vollbild nicht beendet hat.
Monitorwünsche von Anwendungen überschreiben die Workspace-Zuordnung nicht.

`python3 tests/fullscreen_protocol.py` prüft die Zustandswechsel und Hotplug.
`python3 tests/fullscreen_smoke.py` prüft mit einem GTK-Testfenster Wayland und
Xwayland, die tatsächlichen Bildpunkte über der Bar-Fläche und die Rückkehr
zur normalen Größe. Dafür werden zusätzlich ein C-Compiler, pkg-config und
GTK-3-Entwicklungsdateien sowie das Nachbar-Repository mywm-shell benötigt.

## Dateiauswahl-Portale

Ordner- und Dateidialoge verwenden `xdg-desktop-portal` mit dem GTK-Backend.
`scripts/river-init` ruft nach dem WM-Start `scripts/session-environment` auf:
Es übergibt die aktuellen Display- und Desktop-Variablen an D-Bus/systemd und
startet die beiden Portal-Dienste mit dieser Umgebung neu. Ohne diese Übergabe
kann das GTK-Portal mit `cannot open display` ausfallen.

`config/river-portals.conf` kann unter
`~/.config/xdg-desktop-portal/river-portals.conf` installiert werden; es wählt GTK
für River, ohne die Hyprland-spezifische Konfiguration zu ändern. In einer
laufenden mywm-Sitzung lässt sich `scripts/session-environment` zur Reparatur
aufrufen. Dafür müssen `xdg-desktop-portal` und `xdg-desktop-portal-gtk` installiert
sein. Für Bildschirmfreigabe und Screenshots wird zusätzlich
`xdg-desktop-portal-wlr` benötigt. `config/river-portals.conf` ordnet ScreenCast
und Screenshot dem wlr-Backend zu, FileChooser bleibt bei GTK. Das Startskript
startet das installierte wlr-Backend ebenfalls mit der aktuellen Sitzungsumgebung.
Nach einer Neuinstallation gegebenenfalls `systemctl --user daemon-reload`
ausführen und anschließend `scripts/session-environment` starten.

Vesktop nach einer Portal-Umstellung neu starten und die Bildschirmfreigabe
öffnen. Die Quellenauswahl verwendet Zenity im Listenmodus. Dafür
`config/river-screencast.conf` nach `~/.config/xdg-desktop-portal-wlr/river`
installieren. Bei Anfragen nach Monitoren und Fenstern überspringt das wlr-Portal
in seiner automatischen Auswahl Slurp; ohne einen Listen-Chooser scheitert die
Freigabe dann mit `no output found`. Zenity zeigt die vom Portal angebotenen
Quellen an und gibt nur die ausdrücklich ausgewählte Quelle zurück.
