# Overlay-Rand: Befunde und verworfene Ansätze

Stand: 21. September 2026. Visuelle Tests übernimmt der Benutzer.

## Bestätigter Ausgangspunkt

Die Release-Version vor dem Schatten-Patch ist laut Benutzer transparent,
zeigt aber einen schmalen hellen Balken am oberen Rand des Overlay-Monitors.
Verwendet werden eframe/egui-winit 0.36.1, winit 0.30.13 und wgpu mit GL.
`with_monitor(...)` erzeugt einen monitorfüllenden Borderless-Fullscreen-Viewport.

## Fehlgeschlagener Versuch am 21. September

Ein lokaler egui-winit-Patch hat `has_shadow(false)` für Windows unterstützt
und die Einstellung bei Fenstererstellung und späteren Decorations-Kommandos
beibehalten. Nur das Overlay hat diese Option gesetzt. Transparenzoption,
Clear-Color und Renderer waren unverändert.

Benutzerergebnis: Das Overlay war vollständig schwarz und nicht transparent.
Der Patch ist deshalb vollständig zurückgenommen: keine lokale Bibliothekskopie,
kein Cargo-Patch und kein `has_shadow(false)` im Overlay. Die Originalabhängigkeit
wird wieder verwendet. Der zurückgebaute Stand benötigt manuelle Sichtprüfung.

## Frühere Fehlschläge aus den Projekt-Chats

- `decorations(false)` entfernen: schwarzes Overlay, zurückgenommen.
- Native WM_NCCALCSIZE-Subclass: Titelleisten-/Schwarzbild-Regressionen,
  zurückgenommen.
- DWM-Randfarbe einmalig oder wiederholt deaktivieren: keine dauerhaft vom
  Benutzer bestätigte Beseitigung des Balkens.
- Glow: diskutiert, aber kein bestätigter Fix. Vulkan und DX12 verursachten
  auf einem anderen Rechner bereits Treiber-/Surface-Probleme.

## Einordnung

Der Ein-Pixel-Offset im winit-Schattenpfad ist im Quellcode belegt. Nicht belegt
ist, dass er die einzige Ursache des sichtbaren Balkens ist oder gefahrlos
entfernt werden kann. Der fehlgeschlagene Test zeigt, dass die bisherige Annahme
unabhängiger Schatten- und Transparenzfunktionen für diese Konfiguration nicht
trägt. Die genaue Ursache des Schwarzbilds ist weiterhin offen.

Eine prüfbare Hypothese ist eine Wechselwirkung mit exakt monitorfüllenden
OpenGL-Fenstern und Fokus/Windows-Komposition. Ein ähnlicher Benutzerbericht
in Godot beschreibt Transparenzverlust bei exakt bildschirmgroßen, fokussierten
GL-Fenstern und Vermeidung durch eine kleinere Fenstergröße. Das ist ein Hinweis,
kein Nachweis für diese Anwendung:
<https://github.com/godotengine/godot/issues/107582>.

## Untersuchungsplan nach dem Rückbau

Eine separate minimale Test-EXE vergleichen, ohne die funktionierende Release-EXE
zu ersetzen: Schatten an/aus bei einem kleinen normalen Overlayfenster,
Borderless-Fullscreen und einem normalen Fenster knapp unter Monitorgröße.
In jedem Fall Fokus, Click-through und Editiermodus vergleichen. Native
Fenster-/Clientrechtecke, DPI, Renderer sowie DWM-Rückgabewerte protokollieren.

Falls der Fehler nur bei voller Monitorabdeckung auftritt, ein gewöhnliches
transparentes Overlayfenster mit expliziter Monitorgeometrie erproben. Ein
Ein-Pixel-Größenunterschied wäre zunächst Diagnose bzw. Workaround, kein
bestätigter endgültiger Fix; DPI, Ränder, Timer/GIF-Platzierung und Monitorwechsel
müssen anschließend geprüft werden.

Falls auch kleinere Fenster betroffen sind, einen separaten nativen Overlaypfad
mit per-pixel Alpha (WS_EX_LAYERED / UpdateLayeredWindow) prototypisieren.
Die Steueroberfläche kann egui behalten. Timer und GIF sollten kleine eigene
Fenster verwenden; Eingaben, Skalierung und Animationsleistung erfordern einen
größeren Umbau und manuelle Abnahme. Windows dokumentiert diesen Alphapfad:
<https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow>.

Keine weiteren Schatten-/WM_NCCALCSIZE-Eingriffe in die Hauptanwendung, bevor
Transparenz und fehlende Randlinie zusammen im separaten Versuch bestätigt sind.

## Separate Diagnose und Farbvergleich implementiert

Die eigenständige overlay-diagnostics.exe ist jetzt als weiteres Binary
vorhanden. Sie vergleicht DWM-Randfarben (unterdrückt, Standard, Schwarz, Rot),
Schatten, kleine/exakte/eingerückte Geometrie, Vollbildmodus, Klickdurchlässigkeit
und Fokus. Optionale Caption- und Client-Linienfarben grenzen den Besitzer des
Pixels ein. Beschreibung: [DIAGNOSTICS.md](DIAGNOSTICS.md).

Die Farbattribute ändern weder Schatten noch Geometrie. Dass der störende Pixel
darauf reagiert, ist noch nicht visuell bestätigt. Die Diagnose protokolliert
erstmals die DWM-Rückgabewerte und zurückgelesenen Farben.

Die Diagnose verwendet einen separaten Root-Viewport statt des produktiven
Mehrfensterbetriebs und setzt Schatten zur Laufzeit vor dem ersten sichtbaren
Frame. Der Vergleich ist deshalb zunächst anhand des Ausgangszustands zu
validieren. Die Hauptanwendung und ihre bestehende Release-EXE bleiben unverändert.

## Manuelle Befunde und Diagnosekorrektur, 21. September 2026

Rückmeldung des Benutzers:

| Test | Beobachtung |
|---|---|
| Normales Fenster in exakter Monitorgröße, Schatten aus | Hintergrund wird schwarz; Diagnoseanimation bleibt sichtbar |
| Kleines Fenster, Schatten an | Rand sichtbar |
| Kleines Fenster, Schatten aus | Kein Rand |
| „Monitorfüllend wie Hauptanwendung“, Schatten an oder aus | Kein erkennbarer Rand; dieser Test war wegen eines Diagnosefehlers nicht monitorfüllend |

Die Protokolle des betroffenen Displays (1920 × 1080, Desktop-Ursprung
2560/243, Skalierung 100 %) zeigen:

- Normales exaktes Fenster: äußeres Rechteck 2560/243 bis 4480/1323;
  ohne Schatten liegt auch der Client-Ursprung bei 2560/243.
- Mit Schatten verschiebt sich der Client-Ursprung in den ausgewerteten Fällen
  um genau einen Pixel nach unten; ohne Schatten entfällt dieser Versatz.
- Der als Fullscreen bezeichnete Test hatte tatsächlich nur 800 × 450 Pixel,
  obwohl winit einen gesetzten Vollbildmodus meldete. Beispiel:
  test-1790025180564-6488.log. Daraus darf weder funktionierende randlose
  Vollbildtransparenz noch eine Abweichung zum produktiven Vollbild abgeleitet werden.
- Rot und Schwarz wurden von DwmSetWindowAttribute angenommen. Die zusätzliche
  Leseoperation scheiterte mit E_INVALIDARG; dies widerlegt den Set-Erfolg nicht.
  Eine sichtbare Wirkung der Randfarbe ist noch nicht vom Benutzer bestätigt.
- Es gab einen eingerückten Test mit Schatten an, aber noch keinen protokollierten
  eingerückten Test mit Schatten aus in dieser Auswertung.

Ursache des Diagnosefehlers: Der ViewportBuilder enthielt sowohl Monitorwahl
als auch eine feste innere Startgröße von 800 × 450. egui-winit wendet die
innere Größe nach Erstellung des Vollbildfensters erneut an. Der anschließende
Aufruf mit identischem Vollbildmodus korrigierte diese Größe nicht.

Revision 2 entfernt die feste Größe aus dem Vollbildaufbau und entfernt auch
etwaige wiederhergestellte Größen/Positionen im finalen Builder-Hook. Vor der
Diagnoseausgabe werden native Position, Größe und Vollbildstatus gegen die
gewählte Testkonfiguration geprüft. Eine Abweichung beendet den Test mit Fehler;
die geprüfte Größe erscheint im Testfenster und Protokoll. Regressionstests
verwenden die oben gemessene falsche 800×450-Konfiguration.

Nächste gezielte Vergleiche: korrigierter Vollbildtest mit Schatten an/aus und
das normale, an jeder Kante einen physischen Pixel eingerückte Fenster mit
Schatten aus. Die beiden bereits gültigen kleinen/exakten Testfälle müssen
nicht allein wegen des Vollbildfehlers wiederholt werden. Keine Änderung an
der Hauptanwendung aus den ungültigen Vollbildergebnissen ableiten.

## Bestätigte Ergebnisse mit Revision 2

Die anschließende Benutzerrückmeldung bestätigt:

| Aufbau | Schatten | Visuelles Ergebnis |
|---|---|---|
| Normales Fenster, exakt Monitorgröße | Aus | Schwarzer Hintergrund |
| Normales Fenster, exakt Monitorgröße | An | Durchsichtiger Hintergrund, sichtbarer Schatten/Rand |
| Normales Fenster, jede Kante 1 Pixel eingerückt | Aus | Funktioniert transparent und ohne störenden Schatten |
| Monitorfüllender Vollbildmodus | An | Transparent; laut Benutzer funktioniert diese Konfiguration |
| Monitorfüllender Vollbildmodus | Aus | Schwarz und undurchsichtig; nachträglich vom Benutzer präzisiert |

Die neuen Protokolle enthalten erfolgreiche Geometrieprüfungen:

- Exakt: 1920 × 1080, äußeres Rechteck 2560/243 bis 4480/1323.
  Ohne Schatten entspricht der Client-Ursprung exakt der Monitorecke;
  mit Schatten liegt er einen Pixel darunter.
- Eingerückt ohne Schatten: 1918 × 1078, äußeres Rechteck
  2561/244 bis 4479/1322; Client-Ursprung entspricht der äußeren Ecke.
  Beispiele: test-1790025686923-29760.log und test-1790025709434-29760.log.
- Vollbild: jetzt tatsächlich 1920 × 1080. Die bei der vorherigen Auswertung
  gelesenen Vollbildprotokolle verwenden Schatten an
  (z. B. test-1790025731483-29760.log). Der Benutzer hat anschließend ausdrücklich
  präzisiert: Vollbild funktioniert nur mit Schatten; ohne Schatten wird auch
  dieses Fenster schwarz und undurchsichtig.

Alle genannten Fenster verwenden denselben GL-Renderer. Normales Fenster
und Vollbild haben zwar unterschiedliche native Fensterstile, zeigen bei voller
Monitorgröße ohne Schatten nach Benutzerbestätigung aber denselben
Transparenzverlust. Ein Wechsel zum Vollbildmodus allein löst das Problem nicht.

Ableitung: Schattenabschaltung ist für diese Grafik-/Windows-Konfiguration
bei kleinen und um einen Pixel eingerückten Fenstern möglich. Volle
Monitorgröße zusammen mit abgeschaltetem Schatten führt in beiden getesteten
Fenstermodi zum schwarzen Hintergrund. Das stützt die Geometriehypothese stärker;
die präzise interne Treiber-/Kompositionsursache ist weiterhin nicht bewiesen.

Für einen gezielten nächsten Versuch in der Hauptanwendung empfiehlt sich das
bereits visuell bestätigte normale Fenster mit einem physischen Pixel Abstand
an jeder Kante und abgeschaltetem Schatten. Auf 1920×1080 bedeutet das
1918×1078 ab Monitorursprung +1/+1. Die ausgelassenen Desktop-Pixel bleiben
unbedeckt; es wird kein schwarzer Rahmen gezeichnet. Eine neue UI-Library ist
für diesen Versuch nicht erforderlich.

Die Übertragung in den zusätzlichen Overlay-Viewport der Hauptanwendung
benötigt noch einen eigenen manuellen Test, insbesondere nach Monitorwechsel
und beim Umschalten des Bearbeitungsmodus. Bislang wurde aus dieser letzten
Rückmeldung nur die Diagnoseauswertung dokumentiert, kein neuer produktiver
Fensterpatch eingebaut.

## Ein-Pixel-Variante implementiert

Die zusätzliche Hauptanwendungsvariante liegt auf codex/overlay-one-pixel.
Sie verwendet keinen Vollbildmodus, hält Schatten und Dekoration konstant aus
und positioniert das native Fenster einen physischen Pixel innerhalb jeder
Monitorkante. Fenster- und Clientgeometrie müssen vor dem Einblenden übereinstimmen.
Die Prüfung läuft auch bei versteckter Steuerung und erneut vor dem Overlayrendern.

Der Start des zusätzlichen Fensters bleibt zunächst verborgen, damit weder die
temporäre Erstellungsgröße noch ein ungeprüfter Rahmen sichtbar werden.
Bei Monitor-/DPI-Änderungen wird eine abweichende äußere Größe korrigiert;
die Kontrolle selbst erzwingt keine erneute Rahmenberechnung, falls alles stimmt.

Die lokale Anpassung von egui-winit berücksichtigt die explizite Schattenoption
bei Erstellung. Laufende Änderungen von Dekorationen werden von dieser kleinen
Anpassung nicht allgemein unterstützt; die Anwendung hält die Dekoration konstant.
Ein Regressionstest prüft, dass Sichtbarkeits- und Bearbeitungswechsel weder
Dekorations- noch Vollbildkommandos auslösen.

Noch nicht visuell bestätigt: Die Ein-Pixel-Umsetzung im tatsächlichen
Zusatzfenster, einschließlich Monitorwechsel, Bearbeitungsmodus, Tray-Betrieb
und GIF. Keine GUI-Sichtprüfung wurde vom Agenten gestartet.

## Rückmeldung und Startkorrektur vom 22. September 2026

Der Benutzer bestätigt für die tatsächliche Anwendung:
- Vollbild-Vergleichsbuild: heller Streifen oben weiterhin vorhanden.
- Ein-Pixel-Build: kein störender Rand, aber der Timer erscheint beim ersten Start
  nicht. Aus-/Einblenden hilft nicht; Positionieren bringt die Darstellung zurück.

Die Protokolle application-one-pixel-1790027543278-24152.log und
application-one-pixel-1790027715280-5348.log zeigen bereits vor der Interaktion
korrekte 1918×1078-Client- und Fensterflächen ab 2561/244 sowie visible=true.
Beim Wechsel zur Positionierung ändern sich die erweiterten Stile von 0xC0138
zu 0x40118: winit entfernt WS_EX_LAYERED und WS_EX_TRANSPARENT für den
Bearbeitungsmodus. Die Geometrie bleibt dabei gleich. Die Protokolle enthalten
keinen Nachweis des gezeichneten Timerinhalts.

Stärkste Arbeitshypothese: Klickdurchlässigkeit wird vor Einrichtung der
GL-Zeichenfläche aktiviert. egui-winit setzt sie direkt bei create_window,
eframe initialisiert danach erst die zusätzliche wgpu-Zeichenfläche.
Ein ähnliches Symptom ist upstream beschrieben:
https://github.com/emilk/egui/issues/2537
Der ältere Bericht beweist nicht dieselbe Ursache auf diesem System.

Gezielter Fix auf codex/overlay-one-pixel:
1. Der Builder erstellt das Overlay verborgen und ohne Klickdurchlässigkeit.
2. Nach Rückkehr aus eframes Fenster- und Grafikinitialisierung prüft die
   bestehende native Synchronisierung die Ein-Pixel-Geometrie.
3. Sie aktiviert über winit die gewünschte Klickdurchlässigkeit und sendet
   erst danach den Sichtbarkeitsbefehl.
4. Sichtbarkeit und Klickdurchlässigkeit bleiben im Builder konstante
   Erstellungswerte. Das ist erforderlich, weil eframe Builder-Änderungen vor
   expliziten Viewport-Kommandos ausführt.
5. Der tatsächliche native Zustand wird auch aus der Hintergrundlogik gelesen,
   damit das Verfahren bei minimierter Steuerung und späterem Einblenden greift.

Schatten, Ein-Pixel-Abstand, Timerposition und Renderer bleiben unverändert.
29 Tests und Clippy ohne Warnungen bestätigen die automatischen Prüfungen;
die Wirksamkeit gegen das konkrete visuelle Symptom ist noch manuell zu prüfen.

Der neue Build heißt target\release\overlay-timer-one-pixel-startfix.exe
(Fenstertitel mit „Startfix“). Die vorherige Ein-Pixel-EXE wurde nicht ersetzt,
weil sie beim Bauen noch lief. Auch der Vollbild-Vergleichsbuild bleibt erhalten.
