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

Beim Start endet der Bearbeitungsmodus automatisch. Das Overlay wird wieder vollständig klickdurchlässig, damit die Präsentation normal bedient werden kann. Eine freie Position wird relativ zur Monitorgröße gespeichert; die vier Ecken bleiben weiterhin als schnell auswählbare Presets verfügbar.

Globale Hotkeys funktionieren auch dann, wenn PowerPoint den Fokus besitzt:

| Hotkey | Aktion |
|---|---|
| `Ctrl+Alt+P` | Start/Pause |
| `Ctrl+Alt+R` | Reset |
| `Ctrl+Alt+O` | Overlay ein-/ausblenden |

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
