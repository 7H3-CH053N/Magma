# Arbeitsregeln für dieses Repository

## Releases

**Vor jedem Release testet Alex selbst lokal. Kein Version-Bump und kein Tag,
bevor er zugestimmt hat.** Grüne CI ersetzt das nicht.

Wenn eine Änderung fertig ist, gehört ihm ein Testinstaller gebaut, nicht ein
Release geschnitten: Actions → CI → *Run workflow*, Feld `release_tag` **leer**
lassen. Das erzeugt DMG und MSI als Artefakte, ohne zu veröffentlichen. Erst
nach seiner Rückmeldung dieselbe Aktion mit `release_tag` auf `vX.Y.Z`.

Er arbeitet auf einem Mac und einem Windows-PC und testet dort, wo er gerade
sitzt. Reine Dokumentationsänderungen sind davon nicht betroffen.

### Version hochziehen

Fünf Stellen, und die Lockfiles werden regelmäßig vergessen. Bei v0.1.5 stand
`package-lock.json` noch auf 0.1.3, war also schon eine Version davor
übersehen worden, und `npm ci` installierte eine Lockfile, die
`package.json` widersprach.

- `package.json`
- `package-lock.json` — **zwei** Stellen: Wurzel-`version` und `packages[""].version`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock` — der `[[package]] name = "magma"`-Eintrag, sonst scheitert `cargo test --locked`

Der macOS-Job signiert und notarisiert. Die Notarisierung wartet auf Apple, und
wie lange ist nicht vorhersagbar: gemessen wurden 36 Minuten und acht Minuten
für denselben Build-Schritt. Nicht abbrechen, solange das Zeitlimit von 60
Minuten nicht erreicht ist.

## Was in diesem Projekt schiefgeht

Die teuren Fehler hier waren durchweg solche, die **nichts kaputt machen, was
ein gewöhnlicher Test bemerkt**: ein fehlender Snapshot ändert kein Ergebnis,
eine ungefilterte Dataview-Tabelle sieht aus wie eine richtige, ein pro Notiz
neu kompilierter Regex liefert dasselbe nur siebzigmal langsamer. „CI grün"
heißt, es kompiliert und die vorhandenen Tests laufen. Es heißt nicht, es ist
richtig.

Zwei Sonderformen, beide hier passiert:

**Eine Reparatur gilt nur auf der Seite, auf der sie gemessen wurde.** Der
Dateiname-Kontext war gegen einen echten Fehler der Wortsuche gebaut und wurde
per Analogie auf die Embedding-Seite kopiert. Dort hat er eine Passage von Rang
2 auf Rang 77 gedrückt — sieben Monate lang unbemerkt, weil kein Test die
Reihenfolge prüft.

**Wenn eine Änderung nur die Reihenfolge verschiebt und nicht die Werte, sieht
niemand etwas.** Die Kosinuswerte lagen über alle Varianten zwischen 0,814 und
0,843. Drei Hundertstel, fünfundsiebzig Ränge. Wer auf die Zahlen schaut, sieht
nichts Auffälliges.

Daraus die Regel: **Bei jedem Fix dieser Art nachweisen, dass der neue Test
ohne den Fix umfällt.** Sonst ist es ein Test, der nichts prüft. Und nach dem
Einfügen in eine Testdatei die Testzahl *und* die Namensliste prüfen, nicht nur
„grün" lesen: Ein Einschub zwischen `#[test]` und seiner Funktion hat hier
schon einmal einen Test stillgelegt, ohne dass etwas fehlschlug.

## Diagnosen

Alex testet selbst und meldet knapp („hängt und crasht", „da passiert gar
nichts"). Diese Meldungen waren bisher ausnahmslos echte Fehler, keine
Fehlbedienung. Er widerspricht, wenn eine Diagnose nicht stimmt, und lag damit
bisher immer richtig.

Deshalb: Den **Mechanismus** benennen, nicht die Ursache behaupten, und einen
Befehl zum Nachmessen mitliefern. Ein Kernel-Symbol belegt, *wie* etwas
passiert, nicht *warum*. Und bei „X ist kaputt, vorher ging es" zuerst mit
`git diff` belegen, ob die eigene Änderung überhaupt in der Nähe war. Beim
Blog-Import war sie es nicht: null geänderte Zeilen, der Fehler war alt und nur
nie sichtbar geworden.

## Reichweite

Magma bleibt lokal. Kein Cloud-Dienst, kein Tunnel, keine Endpunkte über den
eigenen Rechner hinaus. Siehe M8 in [`docs/PLAN.md`](docs/PLAN.md).

Das ist keine Vorliebe, sondern eine Randbedingung mit Vorgeschichte:
`safe_join` wies nur `..` ab, absolute Pfade gingen durch, und in v0.1.0 und
v0.1.1 war damit über MCP jede Datei der Platte les- und überschreibbar. Lokal
kostet so ein Fehler wenig, über Netz alles.
