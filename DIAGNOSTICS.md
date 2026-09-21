# Overlay-Diagnose · Revision 2

Eigenständiges Testprogramm: target\release\overlay-diagnostics.exe.
Die Hauptanwendung und ihre Release-EXE werden nicht verändert.


## Korrektur des ersten Vollbildtests

Die erste Fassung hatte im Modus „Monitorfüllend wie Hauptanwendung“ zusätzlich
eine feste Startgröße von 800 × 450 gesetzt. Die Benutzerprotokolle belegen,
dass diese Größe nach dem Vollbildaufbau wieder angewendet wurde. Ergebnisse
dieses Modus aus Revision 1 sind deshalb keine gültigen Vollbildvergleiche.
Die normalen kleinen, exakten und eingerückten Fenster waren davon nicht betroffen.

Revision 2 entfernt diesen Konflikt und prüft vor der Ausgabe die tatsächlich
erreichte native Fensterposition/-größe sowie den Vollbildstatus. Bei einer
Abweichung wird der Test als ungültig abgebrochen. Im Testfenster steht die
geprüfte Größe, im Protokoll stehen diagnostic_revision=2 und geometry_verified.

Auf Basis der bisherigen Rückmeldung sind jetzt zwei Vergleiche besonders wichtig:
der korrigierte Vollbildmodus mit Schatten an/aus und das um einen Pixel
eingerückte normale Fenster mit Schatten aus. Randfarbe und Fokus dabei
unverändert lassen.
## Manueller Vergleich

1. Diagnose-EXE öffnen und den betroffenen Bildschirm wählen.
2. **Ausgangszustand**, dann **Test starten**. Beobachten, ob der helle Rand und
   die Transparenz dem Verhalten der Hauptanwendung entsprechen.
3. Nach dem Test **Diagnoserot** wählen und erneut starten.
4. Anschließend **Schwarz** testen, bei denselben Fokus- und Mauseinstellungen.

Die drei Farbvorlagen verwenden dieselbe monitorfüllende Geometrie und
aktivierten Schatten. Nur die DWM-Randfarbe unterscheidet sich. Rot dient der
Zuordnung: Wenn genau der störende Streifen rot wird, ist die Farbe gezielt
steuerbar. Schwarz wäre dann ein möglicher kosmetischer Workaround.

DWMWA_BORDER_COLOR nimmt einen COLORREF ohne Alphakanal entgegen:
0x00000000 bedeutet Schwarz, 0x000000FF Rot. 0xFFFFFFFE unterdrückt den
DWM-Rand, 0xFFFFFFFF stellt den Systemstandard her. Die Farbattribute sind
ab Windows 11 Build 22000 dokumentiert. Ein erfolgreicher Aufruf beweist keine
sichtbare Änderung; Fehler und zurückgelesene Werte stehen im Protokoll.

## Transparenz und Geometrie

Anschließend bei konstanter Randfarbe nacheinander vergleichen:

- Kleines Fenster, Schatten an und aus.
- Monitorfüllend wie Hauptanwendung, Schatten an und aus.
- Normales Fenster in exakter Monitorgröße, Schatten an und aus.
- Normales Fenster mit einem physischen Pixel Abstand an jeder Kante,
  Schatten an und aus.

„Normales Fenster“ bedeutet hier weiterhin transparent und ohne Dekoration,
aber ohne winit-Vollbildmodus. Seine äußere Geometrie wird in physischen
Desktop-Pixeln gesetzt. Negative Monitorpositionen und Skalierungen ändern den
Ein-Pixel-Abstand nicht. Bei Schatten kann der Client-Ursprung um einen Pixel
von der äußeren Fensterkante abweichen; genau deshalb werden beide protokolliert.

**Mausklicks durchreichen** und **Testfenster beim Start fokussieren** jeweils
getrennt vergleichen. Bei Bedarf während eines Tests mit Alt+Tab zwischen
Anwendungen wechseln. Der tatsächlich erreichte Fokus wird protokolliert;
Windows kann insbesondere beim Vollbildwechsel selbst Fokusänderungen auslösen.

Unter **Weitere Vergleichsmöglichkeiten**:

- Titelleistenfarbe unabhängig schwarz oder grün setzen.
- Eine schwarze oder blaue Linie in der ersten Client-Pixelzeile zeichnen.
  Diese erreicht keinen Pixel außerhalb der Clientfläche.
- Den transparenten Renderer-Hauptpuffer getrennt anfordern. Standard aus
  entspricht dem globalen Renderer-Schalter unserer Hauptanwendung; das native
  Testfenster wird unabhängig davon immer transparent angefordert.

Die bewegte, halbtransparente blaue Testform hilft, laufende Bildausgabe und
Alpha-Komposition von einem eingefrorenen Fenster zu unterscheiden.

## Beenden und Protokolle

Jeder Test endet standardmäßig nach 20 Sekunden. Zusätzlich:

- **Strg+Alt+F12** beendet den Test über die unabhängige Steuerung.
- **Test beenden** beendet den eigenen Testprozess.
- **Esc** schließt ein fokussiertes Testfenster.
- Schließen der Steuerung beendet ebenfalls ihren Testprozess.
- Die Steuerung beendet einen hängen gebliebenen Test spätestens 15 Sekunden
  nach der gewählten Dauer, sofern ihr eigener Prozess noch reagiert.

Ist das globale Tastenkürzel bereits belegt, zeigt die Steuerung den Fehler an.

Pro Test entsteht eine Datei unter %TEMP%\overlay-timer-diagnostics.
Die Steuerung zeigt den Pfad an und kann eine manuelle Beobachtung ergänzen.
Erfasst werden Konfiguration, Grafikadapter und Treiber, Fokus, Skalierung,
Fenster-/Clientrechteck, Client-Ursprung im Desktop, DWM-Rahmen, Fensterstile,
DWM-Rückgabewerte und zurückgelesene Farben. Es werden keine Screenshots erstellt.

## Aussagegrenzen

Der Renderer und die Bibliotheksversionen entsprechen der Hauptanwendung:
eframe/egui-winit 0.36.1, winit 0.30.13, wgpu mit GL.
Das Testfenster läuft jedoch als eigener Root-Viewport in einem separaten
Prozess statt als zusätzlicher Viewport der Steuerung. Der globale
Renderer-Transparenzschalter wird im Standard bewusst wie im Original gesetzt.

Die Schattenoption wird über die öffentliche winit-Funktion vor dem ersten
sichtbaren Frame gesetzt, nachdem die Grafikoberfläche bereits angelegt wurde.
Es gibt keinen Bibliothekspatch und keine eigene WM_NCCALCSIZE-Subclass.
Damit eignet sich die EXE für gezielte Vergleiche, ersetzt aber keine spätere
Bestätigung im eigentlichen Mehrfensterbetrieb. Zuerst die Übereinstimmung des
Ausgangszustands prüfen; ein abweichender Ausgangszustand gehört ins Protokoll.

## Bauen

~~~powershell
cargo build --release --bin overlay-diagnostics
~~~

cargo run --release startet weiterhin die normale Hauptanwendung.
Das Diagnoseprogramm wird nicht automatisch gestartet.

## Primärquellen

- [Windows: DWM-Farbattribute](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)
- [Windows: COLORREF](https://learn.microsoft.com/en-us/windows/win32/gdi/colorref)
- [winit: Schatten und oberer Ein-Pixel-Rand](https://docs.rs/winit/0.30.13/winit/platform/windows/trait.WindowExtWindows.html#tymethod.set_undecorated_shadow)
- [Godot: Transparenzverlust bei exakter Monitorgröße](https://github.com/godotengine/godot/issues/107582)

Automatisierte Prüfungen kontrollieren Kompilierung und Geometrieberechnungen.
Die tatsächliche Randfarbe und Transparenz prüft der Benutzer visuell.

DWM-Farbwerte werden zusätzlich versuchsweise zurückgelesen. Diese Leseoperation
ist für die Farbattribute nicht dokumentiert zugesichert: Ein Lesefehler bedeutet
nicht, dass das separat protokollierte Setzen fehlgeschlagen ist.

Die Mausoption vergleicht getrennte Startkonfigurationen. Wiederholtes Umschalten
des Bearbeitungsmodus innerhalb desselben Fensters wird damit noch nicht geprüft.
