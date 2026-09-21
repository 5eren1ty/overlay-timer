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

## Empfohlener nächster Schritt (noch nicht implementiert)

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
