# PowerPoint-Automatik

## Bestehende Architektur

- `src/timer.rs` hält den Countdown mit `Instant` und kennt Start/Pause sowie Reset.
- `src/app.rs` verarbeitet Schaltflächen, globale Hotkeys und Tray-Befehle. `logic()` läuft auch bei verborgenem Steuerfenster weiter und aktualisiert alle 100 ms Timer, Tray und Overlay.
- `src/overlay.rs` erhält nur einen Snapshot des Timerzustands. Die PowerPoint-Erkennung gehört deshalb in die Steuerlogik, nicht in das Overlay.
- `Settings` in `src/app.rs` werden mit `eframe` gespeichert; Laufzeitstatus und COM-Objekte werden nicht gespeichert.

## Erweiterung

Ein eigener Windows-STA-Thread beobachtet die bereits laufende PowerPoint-Instanz über COM. Er startet PowerPoint nicht und verändert keine Präsentation. Er fragt `Application.SlideShowWindows` ab und meldet nur Zustandswechsel an `src/app.rs`. Die Oberfläche ruft niemals COM auf, damit ein beschäftigtes PowerPoint sie nicht blockiert.

Jedes neu beobachtete Diavortragsfenster ist ein neuer Lauf, auch wenn dieselbe Datei erneut gezeigt wird. Beim Ende hält der Countdown seinen letzten Wert. Beim nächsten Beginn wird auf die konfigurierte Dauer zurückgesetzt und gestartet. Ist beim Aktivieren der Automatik bereits ein Diavortrag im Gang, beginnt die Messung erst mit dem nächsten Diavortrag; dessen unbekannte Startzeit wird nicht geschätzt.

Die Automatik ist optional und standardmäßig aus. Manuelle Timerbedienung bleibt verfügbar. Ein PowerPoint-Fehler darf weder die App beenden noch einen laufenden Timer ohne gesichertes Endsignal zurücksetzen. Für die Prüfung mit echtem PowerPoint sind nacheinander gestartete Präsentationen, Esc-Abbruch, Referentenansicht und PowerPoint-Neustart relevant.

Die Anbindung gilt für PowerPoint Desktop unter Windows. Sie setzt voraus, dass Präsentation und Overlay Timer auf demselben Rechner laufen.
