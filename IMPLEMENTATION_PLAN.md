# Implementierungsplan – Variante B mit Hotkeys

## Zielbild

Die Anwendung besteht aus zwei nativen Fenstern in einem Prozess:

1. **Steuerfenster** – normales, interaktives Fenster auf dem Notebook-Display.
2. **Overlay** – transparentes, rahmenloses, immer im Vordergrund liegendes und klickdurchlässiges Fenster auf dem Präsentationsdisplay.

PowerPoint wird nicht erkannt oder gesteuert. Stattdessen wählt der Benutzer den Zielmonitor explizit. Dadurch bleibt die Lösung auch für andere Präsentations- und Videoprogramme nutzbar.

## Umsetzungsschritte

1. **Timer-Kern**
   - Zustände `Paused` und `Running` mit getrennten Messwerten für Rest- und Überziehungszeit.
   - Driftfreie Zeitmessung mit `Instant` und Deadline statt sekündlichem Dekrement.
   - Start/Pause, Reset und Änderung der Gesamtdauer.
   - Unit-Tests für Ablauf, Pause, Reset und Formatierung.

2. **Steuerfenster**
   - Eingabe der Dauer in Minuten und Sekunden.
   - Start/Pause- und Reset-Schaltflächen.
   - Vorschau von Restzeit und Status.
   - Auswahl von Monitor, Ecke, Randabstand, Schriftgröße und Overlay-Sichtbarkeit.
   - Expliziter Bearbeitungsmodus für freie Drag-and-drop-Positionierung und Skalierung direkt an der Timerkarte.
   - Persistenz der Anzeigeeinstellungen über `eframe`-Storage.

3. **Overlay-Fenster**
   - Zweiter nativer egui-Viewport.
   - Randlos, transparent, Always-on-top, ohne Taskbar-Eintrag, initial ohne Fokus und mit Mouse-Passthrough.
   - Monitorfüllender transparenter Viewport; die Timerkarte wird DPI-sicher in der gewählten Ecke verankert.
   - Aktualisierung alle 100 ms; Anzeige als `MM:SS`, nach Ablauf rot als `+MM:SS`.
   - Kein Textumbruch bei großen Schriftgrößen.
   - Im Bearbeitungsmodus interaktiv; beim Timerstart automatisch wieder klickdurchlässig.

4. **Globale Hotkeys**
   - `Ctrl+Alt+P`: Start/Pause.
   - `Ctrl+Alt+R`: Reset.
   - `Ctrl+Alt+O`: Overlay ein-/ausblenden.
   - Registrierungsfehler (beispielsweise durch bereits belegte Kombinationen) werden im Steuerfenster angezeigt, ohne die Anwendung zu beenden.

5. **Windows-/Mehrmonitor-Support**
   - Monitore werden über Win32 aufgelistet und mit Gerätename, Auflösung und Primärstatus dargestellt.
   - Die tatsächliche Platzierung erfolgt über den Monitorindex des eframe/winit-Viewports.

6. **Verifikation**
   - `cargo fmt --check`.
   - `cargo test` für die Timerlogik.
   - `cargo clippy --all-targets -- -D warnings`.
   - `cargo build --release` für das Windows-Binary.

7. **Toolchain- und GUI-Upgrade**
   - Reproduzierbare Projekt-Toolchain Rust 1.98.0 über `rust-toolchain.toml`.
   - Deklarierte Mindestversion Rust 1.95.
   - `eframe`/`egui` 0.36.1 einschließlich aktualisiertem WGPU-Unterbau.
   - Regressionstests für Overlay, Hotkeys, Drag-and-drop, Resize und Persistenz.

8. **System-Tray und modernes Steuerfenster**
   - Minimieren blendet das Steuerfenster aus; Overlay, Timer und Hotkeys laufen weiter.
   - Tray-Menü für Öffnen, Start/Pause, Reset, Overlay und reguläres Beenden.
   - Deferred Overlay-Viewport mit synchronisiertem Zustand für unabhängiges Rendering.
   - Material-inspiriertes Kartenlayout, Status-Chips, größere Bedienelemente und vertikaler Scrollbereich.

## Bewusste Abgrenzung des MVP

- Keine PowerPoint-Prozess- oder Fenstertitelerkennung.
- Kein Autostart.
- Die Hotkey-Kombinationen sind fest gewählt und werden im UI dokumentiert.
- Das Overlay zeigt den Countdown dem Publikum; PowerPoints Presenter View wird nicht ersetzt.
