# mywm

Ein experimenteller Windowmanager in Rust für River 0.4 und dessen
`river-window-management-v1`-Protokoll. River übernimmt das Compositing,
mywm die Fensteranordnung und Tastenkürzel.

## Layout und Bedienung

Pro Monitor gibt es standardmäßig neun feste Workspaces mit jeweils einem horizontalen Fensterstreifen:

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
| Super + H / Pfeil links | Linkes Fenster fokussieren |
| Super + L / Pfeil rechts | Rechtes Fenster fokussieren |
| Super + Shift + H / Pfeil links | Fenster nach links verschieben |
| Super + Shift + L / Pfeil rechts | Fenster nach rechts verschieben |
| Super + V | Fokussiertes Fenster zwischen Scrolling und Floating umschalten |
| Super + linke Maustaste | Floating-Fenster unter dem Mauszeiger verschieben |
| Super + rechte Maustaste | Floating-Fenster von der nächstgelegenen Ecke skalieren |
| Super + Q | Fokussiertes Fenster zum Schließen auffordern |
| Super + 1…9 | Workspace auf dem Monitor unter dem Mauszeiger wechseln |
| Super + Shift + 1…9 | Fokussiertes Fenster auf einen Workspace desselben Monitors verschieben |
| Super + M | River-Sitzung einschließlich mywm beenden; zurück zur startenden TTY |

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
beim Wechseln und Verschieben zwischen Workspaces sowie beim erneuten Umschalten
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

Wird ein Monitor entfernt, werden seine Fenster auf den ersten verbleibenden
Monitor übernommen und behalten ihre Workspace-Nummer. Beim Wiederanschließen
werden sie nicht automatisch zurückverschoben. Wenn alle Monitore entfernt
werden, bleibt die Workspace-Zuordnung bis zum nächsten angeschlossenen Monitor
erhalten. Über einen Neustart hinweg wird der Sitzungszustand noch nicht gespeichert.

[`config/mywm.toml`](config/mywm.toml) enthält die bearbeitbaren Standardwerte.
Die Konfiguration wird beim Start geladen:

1. `MYWM_CONFIG`, falls gesetzt (ein falscher Pfad ist ein Fehler).
2. `$XDG_CONFIG_HOME/mywm/config.toml`, sonst `~/.config/mywm/config.toml`.
3. Ohne persönliche Datei verwendet das Startskript die Projektdatei;
   ein direkt gestartetes Binary verwendet eingebaute Standardwerte.

Beispiel für weniger Workspaces und zusätzliche Terminalargumente:

```toml
workspaces = 5
terminal = ["kitty", "--single-instance"]

[bindings]
focus_left = ["Super+h", "Super+Left"]
focus_right = ["Super+l", "Super+Right"]
workspace_modifiers = "Super"
move_to_workspace_modifiers = "Super+Shift"
```

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
auf den zugewiesenen Monitor. Ein Fenster auf einem inaktiven Workspace erscheint
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

Das Startskript startet Kanshi und mywm gemeinsam und beendet beim Ende eines
Prozesses auch den anderen. Rivers Beenden räumt beide Prozesse auf. Es prüft
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

## Aktuelle Grenzen

Noch keine Animationen, konfigurierbaren Spaltenbreiten, gestapelten
Spalten oder Tastenwiederholung. Es wird ein Seat verwaltet. Die Aufteilung gilt
für die gesamte Monitorfläche, reservierte Panelbereiche werden noch nicht
berücksichtigt. Größen sind Vorschläge an Anwendungen; Anwendungen dürfen davon
abweichen und werden auf ihre vorgesehene Fläche beschnitten (Protokollversion 2+).
Bei ungerader Monitorbreite bleibt bei zwei Fenstern ein Pixel frei.
