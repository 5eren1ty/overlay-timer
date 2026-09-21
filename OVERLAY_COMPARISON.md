# Vergleich: Vollbild-Diagnose und Hauptanwendung

Stand: 21. September 2026. Quellcodevergleich mit eframe/egui-winit 0.36.1,
winit 0.30.13 und den nativen Protokollen der Diagnose Revision 2.
Die Hauptanwendung war zum Vergleichszeitpunkt nicht gestartet. Es wurde keine
GUI zur Sichtprüfung geöffnet; ein visueller Nachweis bleibt beim Benutzer.

## Gleiche technische Basis

Beide Anwendungen verwenden wgpu mit ausschließlich GL, ein transparentes,
rahmenloses und nicht skalierbares Fenster, Always-on-top und Klickdurchlässigkeit.
Die Auswahl des Monitors führt jeweils zu winit Borderless Fullscreen.
Der native Schatten ist in der verglichenen Variante aktiviert. Der globale
Transparenzschalter des wgpu-Painters ist in beiden Standardkonfigurationen aus,
die native Fenstertransparenz ist jeweils an. Der Hintergrund wird mit Alpha null
gelöscht. Die Hauptanwendung enthält keine feste innere Overlay-Startgröße, die
dem in Diagnose Revision 1 korrigierten 800×450-Fehler entspräche.

## Konkrete Unterschiede

| Punkt | Diagnose mit Vollbild und Schatten | Hauptanwendung |
|---|---|---|
| Rolle des Overlays | Root-Fenster eines eigenen Prozesses | Zusätzliches deferred Viewport-Fenster |
| Erster sichtbarer Zustand | eframe erstellt Root-Fenster verborgen und zeigt es nach dem ersten Rendern | Der zusätzliche Viewport wird mit seiner gewünschten Sichtbarkeit erstellt |
| Initialisierung | Schatten, Vollbild und Klickdurchlässigkeit werden vor dem ersten sichtbaren Frame noch einmal explizit gesetzt | Fensterattribute und spätere Viewport-Kommandos steuern den Zustand |
| DWM-Randfarbe | Die erfolgreichen zuletzt ausgewerteten Revision-2-Läufe nutzen überwiegend Rot | Wiederholtes DWMWA_COLOR_NONE |
| Neuzeichnen des nativen Rahmens | RedrawWindow nach Einrichtung und Zustandsänderung | Keine entsprechende explizite Anforderung |
| Änderungen nach Erstellung | Überwiegend konstante Konfiguration je Testprozess | Ein-/Ausblenden, Monitorwechsel, Bearbeitungsmodus, Tray und Hotkeys |
| Inhalt | Diagnosekarte und Animation | Timer, GIF und Interaktionsflächen |

Quellcode: src/diagnostics/mod.rs, src/diagnostics/native.rs, src/overlay.rs,
src/windows_overlay.rs, src/main.rs. Abhängigkeiten:
eframe/src/native/wgpu_integration.rs (Root-create_window und
Viewport::initialize_window), egui-winit/src/lib.rs (create_window,
apply_viewport_builder_to_window und process_viewport_command).

## Was sich daraus ableiten lässt

Es fehlt kein einfacher Vollbild-Schalter. Root-/Zusatzfenster und insbesondere
der Zeitpunkt des ersten Einblendens sind konkrete Unterschiede, die einen
anderen initialen Windows-Rahmenzustand erklären könnten. Auch spätere Änderungen
der Klickdurchlässigkeit veranlassen winit zu einer Rahmenaktualisierung.
Keiner dieser Unterschiede ist bisher als alleinige Ursache des hellen Streifens
in der Hauptanwendung nachgewiesen. Auch der Farbvergleich ist ohne gleiche
Randvorgabe noch kein vollständig kontrollierter Vergleich.

Die manuellen Testergebnisse und die zugehörigen Geometrieprotokolle zeigen: Volle Monitorgröße
ohne Schatten wird sowohl im normalen Fenster als auch im Vollbild schwarz.
Mit Schatten funktioniert Vollbild. Ein normales Fenster, das an jeder Kante
einen physischen Pixel eingerückt ist, bleibt ohne Schatten transparent.

## Messmöglichkeit in der tatsächlichen Anwendung

Die Vollbild-Vergleichsversion zeichnet ab jetzt Veränderungen ihres tatsächlichen
Overlay-HWNDs auf: Sichtbarkeit, Fokus, äußeres Fensterrechteck, Clientgröße,
Client-Ursprung, DWM-Rahmen, Fensterstile und das Ergebnis der bestehenden
DWM-Randunterdrückung. Diese Messung verändert keine Fensterattribute.
Protokolle liegen unter %TEMP%\overlay-timer-diagnostics/application-fullscreen-*.log.

Zum manuellen Vergleich dieselbe Monitorwahl und dieselbe Randvorgabe
(Rand unterdrücken) in der Diagnose nutzen, dann Einblenden, Bearbeitungsmodus
an/aus und Monitorwechsel in der Hauptanwendung prüfen. Der Vergleichs-Build
wird separat als target\release\overlay-timer-fullscreen.exe bereitgestellt.
Dessen ursprüngliches Overlayverhalten bleibt erhalten.

## Zusätzliche Ein-Pixel-Variante

Sie ist auf codex/overlay-one-pixel umgesetzt. Sie verwendet ein gewöhnliches
rahmenloses Fenster mit ausgeschaltetem Schatten und setzt die native äußere
Fensterfläche auf Monitorursprung +1/+1 sowie Monitorbreite/-höhe minus 2.
Die geänderte Größe wird vor dem Einblenden geprüft und nach Monitor-
oder DPI-Wechsel erneut geprüft. Die tatsächliche Hauptanwendung mit
Bearbeitungsmodus und GIF muss anschließend manuell bestätigt werden.

Die Ein-Pixel-Version liegt separat unter
target\release\overlay-timer-one-pixel.exe. Beide Versionen schreiben native
Geometrieprotokolle. Die automatischen Prüfungen der Ein-Pixel-Version sind
abgeschlossen: 26 Tests erfolgreich, Clippy ohne Warnungen und Release-Build
erfolgreich. Die GUI wurde nicht gestartet; die Sichtprüfung ist noch offen.
