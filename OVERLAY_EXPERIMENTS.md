# Inkrementelle Overlay-Versuche

## Referenz

Branch codex/overlay-one-pixel, Stand ebba020. Anwendungscode entspricht fb4ee9e.
Transparenz und fehlender Rand sind vom Benutzer bestätigt; der Timer erscheint
beim Start erst nach Positionieren. Der zurückgenommene Startfix 98f6cac machte
das Overlay schwarz und ist ausdrücklich keine Referenz.

## Versuch 1: endgültige Geometrie vor der Grafikinitialisierung

Branch codex/overlay-initial-size.
Build: target\release\overlay-timer-one-pixel-step1.exe.

Einziger geänderter Verhaltensaspekt: Erstellung mit der endgültigen physischen
Größe und Position, statt 800×450 und anschließender nativer Korrektur.
Für den bisher getesteten Monitor: Position 2561/244, Größe 1918×1078.

Der lokale egui-winit-Hook übergibt PhysicalPosition und PhysicalSize an winit
vor create_window und unterdrückt anschließend nur die erneute Anwendung von
logischer Größe/Position. Damit erhält auch die danach erzeugte wgpu-Fläche
bereits die endgültige native Fenstergröße. Es wird kein neues Fenster-
oder Renderer-Framework eingeführt.

Unverändert:
- normaler Fensterbetrieb, ein physischer Pixel Abstand an allen vier Kanten;
- Schatten aus bereits bei Erstellung;
- Klickdurchlässigkeit wie vor dem fehlgeschlagenen Startfix;
- aktive=false, zunächst verborgen und bisherige Einblendelogik;
- Timer-/GIF-Darstellung und gespeicherte Positionen;
- native Überprüfung und Korrektur bei späterem Monitorwechsel.

Die bisherige 800×450-Vorgabe bleibt nur als ungenutzter Builder-Fallback
erhalten; der physische Hook ersetzt sie vor Erstellung. Fehlt ein gültiger
Monitor, wird der Override gelöscht und die vorhandene Prüfung hält das Fenster
verborgen. Tests prüfen insbesondere, dass kein nachträgliches Zurücksetzen
auf 800×450 angefordert wird.

### Manuelle Auswertung

Ältere Version vollständig über das Tray beenden, dann nur den Step1-Build starten.
1. Ist der Timer sofort sichtbar, ohne Positionieren?
2. Bleibt der Hintergrund transparent und der obere Rand verschwunden?
3. Funktionieren danach Overlay aus/an und Positionieren an/aus?
4. Falls verwendet: Monitorwechsel und Tray-Betrieb prüfen.

Protokoll: %TEMP%\overlay-timer-diagnostics\application-one-pixel-initial-size-*.log.
Der erste Zustand soll bereits die Zielgeometrie zeigen.
Ein Eintrag geometry_correction vor dem ersten Einblenden zeigt, dass die
Erstellungsgeometrie noch korrigiert werden musste; dieser Lauf bestätigt dann
nicht die beabsichtigte Startbedingung. Solche Einträge nach Monitorwechsel sind
erwartbar. Es werden native Maße protokolliert, keine gerenderten Pixel.

Status: 28 Tests erfolgreich, Clippy ohne Warnungen und Release-Build erfolgreich.
Visueller Ausgang offen; keine GUI vom Agenten gestartet.
Erst nach diesem Ergebnis wird ein weiterer Punkt der Vergleichsliste variiert.

### Ergebnis von Versuch 1

Benutzerrückmeldung: Die Änderung hat nicht ausgereicht.
Das Protokoll application-one-pixel-initial-size-1790089418492-32100.log
zeigt beim ersten Messpunkt bereits 1918×1078 ab 2561/244 und keinen
geometry_correction-Eintrag. Das Fenster ist zunächst verborgen und nach
0,008 Sekunden sichtbar. Die beabsichtigte Startgeometrie ist damit in
diesem Lauf bestätigt; sie allein behebt das gemeldete Problem nicht.
Ein erster Größenunterschied zwischen Fenster und Renderpuffer ist dadurch
als Erklärung weniger plausibel, jedoch wurde der Renderpuffer nicht gemessen.

Nächster vorgeschlagener Einzelversuch: Auf diesem Stand aufbauen und das
Overlay bei aktivierter Overlay-Einstellung direkt sichtbar erstellen, statt
es erst nach der nativen Prüfung einzublenden. Physische Startgeometrie,
Schatten, Klickdurchlässigkeit und active=false beibehalten. Dies gleicht
den Sichtbarkeitsablauf an den Vollbild-Build an. Noch nicht umgesetzt.

## Versuch 2: direkt sichtbar erstellen

Branch codex/overlay-visible-start, aufbauend auf Versuch 1 (ed552c3).
Build: target\release\overlay-timer-one-pixel-step2.exe.

Einzige neue Verhaltensänderung: Bei aktivierter Overlay-Einstellung und gültiger
Zielgeometrie wird das zusätzliche Fenster bereits mit visible=true erstellt.
Die anfängliche Freigabe wartet nicht mehr auf eine erfolgreiche HWND-Prüfung,
die vor der Fenstererstellung grundsätzlich noch nicht stattfinden kann.
Die vorhandene Prüfung läuft weiterhin; erkannte native Fehler halten das
Fenster verborgen. Eine gespeicherte Einstellung „Overlay aus“ wird respektiert.

Beibehalten: endgültige physische Startgeometrie aus Versuch 1, Ein-Pixel-Abstand,
Schatten aus bei Erstellung, Klickdurchlässigkeit ab der bisherigen Stelle,
active=false, GL-Renderer sowie unveränderter Timer-/GIF-Code.

Protokoll: %TEMP%\overlay-timer-diagnostics\application-one-pixel-visible-start-*.log.
Bei „Overlay an“ soll bereits der erste Messpunkt visible=true und die
richtige Zielgeometrie zeigen, ohne vorausgehenden geometry_correction-Eintrag.

Manueller Test: Vorgängerversion vollständig beenden, Step2 starten und ohne
Positionieren auf den Timer achten. Transparenz und oberen Rand kontrollieren,
danach Overlay aus/an sowie Positionierung an/aus prüfen.
28 Tests erfolgreich, Clippy ohne Warnungen und Release-Build erfolgreich.
Sichtprüfung offen; keine GUI vom Agenten gestartet.
