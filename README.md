# Overlay Timer

Ein Windows-Präsentationstimer mit getrenntem Steuerfenster und klickdurchlässigem Overlay. Das Overlay kann über einer PowerPoint-Bildschirmpräsentation auf einem frei wählbaren Monitor angezeigt werden.

## Bedienung

- Dauer im Steuerfenster einstellen.
- Zielmonitor und Position wählen.
- Für eine freie Anordnung **Position und Größe bearbeiten** wählen. Die Timerkarte kann dann direkt im Overlay verschoben und am Griff unten rechts skaliert werden.
- Overlay einblenden und den Countdown starten.

Nach Ablauf wechselt der Timer auf eine rote Überziehungsanzeige (`+MM:SS`). Diese
kann wie der normale Countdown pausiert, fortgesetzt und zurückgesetzt werden.

Beim Minimieren verschwindet das Steuerfenster in den Windows-System-Tray. Timer,
Overlay und Hotkeys laufen weiter. Ein Linksklick auf das Tray-Icon öffnet die
Steuerung erneut; das Kontextmenü bietet außerdem Start/Pause, Reset, Overlay
ein/aus und Beenden. Das Schließen über `X` beendet die Anwendung weiterhin.

Beim Start endet der Bearbeitungsmodus automatisch. Er kann außerdem jederzeit über die sichtbare Schaltfläche im Overlay oder lokal mit `Esc` beendet werden; `Esc` wird nicht als globaler Hotkey registriert und beeinträchtigt PowerPoint daher nicht. Das Overlay wird anschließend wieder vollständig klickdurchlässig, damit die Präsentation normal bedient werden kann. Eine freie Position wird relativ zur Monitorgröße gespeichert; die vier Ecken bleiben weiterhin als schnell auswählbare Presets verfügbar.

Globale Hotkeys funktionieren auch dann, wenn PowerPoint den Fokus besitzt:

| Hotkey | Aktion |
|---|---|
| `Ctrl+Alt+P` | Start/Pause |
| `Ctrl+Alt+R` | Reset |
| `Ctrl+Alt+O` | Overlay ein-/ausblenden |

## Meme-Modus

Im Steuerfenster **Meme-Modus** einschalten. Sobald der Countdown abläuft,
spielt das eingebettete `hurry-up-judge-judy.gif` jeweils drei Sekunden an einer zufälligen Position auf dem
gewählten Overlay-Monitor. Es startet mit 240 px Breite und wächst alle
**4–60 Sekunden** (Standard: 5 s) um 25 % der Startgröße. Das Seitenverhältnis
bleibt erhalten; die Größe ist auf 70 % der Bildschirmbreite und -höhe begrenzt.
Auf kleinen Bildschirmen wird bereits die Startgröße entsprechend begrenzt.
Bei jedem Wachstumsschritt wechselt das GIF an eine neue zufällige Position,
auch nach Erreichen der Maximalgröße. Während der drei Sekunden bleibt es stehen,
danach verschwindet es bis zur nächsten Stufe. Bei fünf Sekunden Stufenintervall
sind das drei Sekunden Wiedergabe und zwei Sekunden Pause. Jede Stufe startet
die GIF-Animation von vorne. Alte Intervalle unter vier Sekunden werden auf vier
Sekunden angehoben, damit mindestens eine Sekunde Pause bleibt.
Die Platzierung berücksichtigt die tatsächliche Timerkarte mit 16 px Abstand,
auch bei freier Positionierung. Falls das große GIF nicht mehr daneben passt,
wird die Überlappung minimiert; die Timerkarte bleibt darüber sichtbar.
Das Overlay bleibt klickdurchlässig.

Pause hält Animation, Wachstum und die Ausblendpause an; Fortsetzen setzt den Ablauf fort. Reset oder
das Ausschalten des Meme-Modus entfernt das GIF. **Overlay ein/aus** blendet
Timer und GIF gemeinsam aus; im Hintergrund läuft die Zeit weiter. Beim erneuten
Einblenden oder Aktivieren während der Überziehung entspricht die Größe der
bereits verstrichenen Überziehungszeit. Modus und Intervall werden gespeichert;
der Meme-Modus ist anfangs ausgeschaltet. Das GIF wird in die EXE eingebettet,
sodass beim Weitergeben keine zusätzliche GIF-Datei nötig ist.

## Bauen und starten

Das Projekt verwendet reproduzierbar Rust 1.98.0 über `rust-toolchain.toml`. Die deklarierte Mindestversion ist Rust 1.95, da `eframe` 0.36.1 diese voraussetzt.

```powershell
cargo run --release
```

Das Release-Binary liegt anschließend unter `target\release\overlay-timer.exe`.

## Hinweise

- Das Overlay ist absichtlich nicht anklickbar; Eingaben erfolgen über das Steuerfenster oder die globalen Hotkeys.
- Nur im ausdrücklich aktivierten Bearbeitungsmodus nimmt das Overlay Mausereignisse an.
- Falls das Tray-Icon nicht erstellt werden kann, zeigt die App eine Warnung an und minimiert weiterhin normal in die Taskleiste.
- Bei einem Hotkey-Konflikt zeigt das Steuerfenster eine Warnung an. Die restliche Anwendung bleibt verwendbar.
- Das Overlay ist für normale Desktop-Vollbildfenster wie PowerPoints Präsentationsmodus gedacht, nicht für exklusives DirectX-Vollbild.

## Separate Overlay-Diagnose

Für Randfarbe, Schatten und Transparenz gibt es ein unabhängiges Testprogramm:
target\release\overlay-diagnostics.exe. Es kann schwarze und rote DWM-Ränder
sowie verschiedene Fenstergrößen vergleichen und legt pro Test ein Protokoll an.
Bedienung und Aussagegrenzen stehen in [DIAGNOSTICS.md](DIAGNOSTICS.md).

Bauen: cargo build --release --bin overlay-diagnostics.
Die Hauptanwendung bleibt das Standardziel von cargo run --release.

## Ein-Pixel-Variante

Auf dem Branch codex/overlay-one-pixel verwendet die Hauptanwendung ein normales
transparentes Overlayfenster ohne Schatten. Es bleibt an jeder Monitorkante
genau einen physischen Pixel innerhalb des Bildschirms. Native Fenster- und
Clientrechtecke werden vor dem Einblenden und während des Betriebs geprüft.
Dies gilt auch für negative Monitorpositionen und unterschiedliche Skalierungen.
Bei einem Geometriefehler wird das Overlay verborgen und die Steuerung zeigt
eine Fehlermeldung; Details stehen im Anwendungsprotokoll.

Die korrekte Position wird auch bei minimierter Steuerung im Hintergrund
synchronisiert. Timer-/GIF-Positionen beziehen sich weiterhin auf die verfügbare
Overlayfläche. Der lokale egui-winit-Patch berücksichtigt die Schattenoption bei
der Erstellung des zusätzlichen Fensters; Umfang und Grenze sind in
vendor/egui-winit/PATCH.md dokumentiert.

Der Vollbild-Vergleichsbuild liegt unter
target\release\overlay-timer-fullscreen.exe, der Ein-Pixel-Build unter
target\release\overlay-timer-one-pixel.exe. Bitte nacheinander starten, da beide
dieselben globalen Hotkeys verwenden. Die übliche overlay-timer.exe entspricht
auf diesem Branch ebenfalls der Ein-Pixel-Variante.

Protokolle: %TEMP%\overlay-timer-diagnostics\application-fullscreen-*.log
beziehungsweise application-one-pixel-*.log.
Der Quellcodevergleich steht in [OVERLAY_COMPARISON.md](OVERLAY_COMPARISON.md).

Der Startfix vom 22. September wurde nach einem schwarzen Overlay vollständig
zurückgenommen. Der wiederhergestellte Stand liegt zusätzlich unter
target\release\overlay-timer-one-pixel-restored.exe; der Anwendungscode
entspricht wieder fb4ee9e. Das Problem des beim Start unsichtbaren Timers ist
noch offen. overlay-timer-one-pixel-startfix.exe ist ein fehlgeschlagener Versuch
und soll nicht für weitere reguläre Tests verwendet werden.
