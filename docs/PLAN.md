# Magma — Plan für eine Obsidian-Alternative

## Kontext

Ziel: eine Desktop-App (macOS + Windows) mit der Grundidee von Obsidian
(lokale Markdown-Notizen, Verlinkung, "Second Brain"), aber deutlich einfacher
und schöner zu bedienen, und von Haus aus als Second Brain für LLMs anbindbar
(MCP). Auslöser: Frust über Obsidians UI, die umständliche Notizeingabe und die
generelle Bedienung.

## Web-Recherche: Was User an Obsidian mögen / hassen

**Geliebt (unbedingt behalten):**
- **Lokale Markdown-Dateien, volle Datenhoheit** — kein Lock-in, keine
  Cloud-Pflicht, Notizen gehören dem User
  ([Lindy Review](https://www.lindy.ai/blog/obsidian-review),
  [thebusinessdive](https://thebusinessdive.com/obsidian-review))
- **Bidirektionale Links + Backlinks + Graph** — Ideen vernetzen ist der Kern
  des Second-Brain-Konzepts
- **Schnell & kostenlos**, Plain-Text = zukunftssicher
- **Erweiterbarkeit** (Plugins) — geschätzt, aber zugleich Quelle der Komplexität

**Gehasst (unsere Chance):**
- **Steile Lernkurve**: "convoluted", Monate an Experimenten nötig,
  Markdown-Syntax als Einstiegshürde
  ([aitooldiscovery Reddit-Auswertung](https://www.aitooldiscovery.com/guides/obsidian-reddit),
  ["obsidian is too complicated"](https://productivematters.substack.com/p/obsidian-is-too-complicated),
  [DEV.to](https://dev.to/charudatta10/why-obsidian-falls-short-as-a-note-taking-tool-3ef2))
- **UI wirkt roh/karg**, bis man sie stundenlang mit Themes/Plugins
  konfiguriert; Plugin-Overload → Decision Fatigue
- **Basics brauchen Plugins** (anständige Tabellen, Kalender, WYSIWYG-Gefühl)
- **Sync kostet extra** und verwirrt Neueinsteiger
- **Mobile-Apps unpoliert** (v. a. Bilder-Handling)

**Was die "schönen" Konkurrenten richtig machen**
([Bear](https://www.xda-developers.com/bear-is-the-best-note-taking-app-and-its-not-even-close/),
Craft, [Reflect/Capacities](https://noteapps.info/best_note_taking_apps_2026)):
sofort loslegen ohne Konfiguration, Markdown-Syntax die sich beim Schreiben
**automatisch versteckt** (Live-Preview als Default), starke Typografie, wenige
aber gute Defaults, Apple-Design-Award-Niveau an Polish. Schwäche der
Konkurrenz: meist Cloud-only, Mac-only oder proprietär → genau da positioniert
sich Magma.

**LLM-Integration heute**: Obsidian braucht Dritt-MCP-Server
([obsidian-mcp](https://mcpservers.org/servers/lwaetzig/obsidian-mcp),
[Vault as MCP Plugin](https://community.obsidian.md/plugins/vault-as-mcp)).
Magma baut den MCP-Server **direkt ein** — ein Schalter in den Settings statt
Bastelei.

## Produktprinzipien

1. **Behalten, was geliebt wird**: Vault = normaler Ordner mit `.md`-Dateien,
   Obsidian-kompatibel (`[[Wikilinks]]`, YAML-Frontmatter, Tags) → bestehende
   Obsidian-Vaults öffnen ohne Import.
2. **Fixen, was gehasst wird**: Null-Konfiguration, eine schöne Default-UI
   (Bear-Niveau), Live-Markdown das Syntax versteckt, alles Wichtige eingebaut
   statt Plugin-Basar.
3. **LLM-nativ, lesend UND schreibend**: eingebauter MCP-Server mit semantischer
   Suche. Claude & Co. sollen nicht nur Infos abrufen, sondern aktiv Notizen
   anlegen und diese **richtig und logisch mit bestehenden Notizen verlinken** —
   der Server liefert die Kandidaten und prüft die Links.
4. **Der Graph ist ein Hauptfeature**: Die Vernetzung sichtbar zu machen ist
   genau das, was User lieben — der Graph kommt früh, wird schön und performant,
   und zeigt live, wie das Second Brain wächst (inkl. dessen, was LLMs beitragen).

## Tech-Stack

- **Tauri 2** (Rust-Core) statt Electron: kleine Binaries, schnell, nativ auf
  macOS + Windows, geringer RAM-Verbrauch.
- **Frontend**: React + TypeScript + Tailwind CSS; Editor auf **CodeMirror 6**
  (Live-Markdown als einziger, polierter Modus).
- **Rust-Core** (`crates/magma-core`, plattformunabhängig, überall testbar):
  Vault-Logik, später File-Watcher (`notify`), Index in **SQLite + FTS5**,
  Embeddings lokal via `fastembed-rs`. Geteilt von Desktop-Shell und MCP-Server.
- **Graph**: Canvas/WebGL-Rendering (`d3-force` + Canvas oder `sigma.js`), damit
  große Vaults flüssig bleiben.

## Architektur

```
Vault (Ordner mit .md)  ←→  magma-core (Watcher + SQLite-Index)  ←→  React-UI (Tauri WebView)
                                      ↓
                          MCP-Server (stdio/HTTP) ← Claude Desktop, Claude Code, andere Agents
```

Dateien bleiben die Source of Truth; der Index ist ein Cache, jederzeit neu
aufbaubar.

### MCP-Server: LLM als vollwertiger Mitautor

Lesende Tools: `search_notes` (FTS + semantisch), `read_note`,
`list_backlinks`, `list_tags`, `get_daily_note`, `get_graph_neighborhood`.

Schreibende Tools — so gebaut, dass das LLM **korrekt und logisch verlinkt**
statt Waisen-Notizen abzuladen:
- `find_link_candidates(text)`: liefert vor dem Schreiben die ähnlichsten
  bestehenden Notizen mit Kurz-Kontext — das LLM sieht, wohin die neue Notiz
  gehört.
- `create_note` / `update_note` / `append_to_note`: der Server **validiert alle
  `[[Wikilinks]]`** gegen den Index; Links auf nicht existierende Notizen werden
  mit Fast-Treffern als Korrekturvorschlag zurückgemeldet statt stumm kaputte
  Links zu erzeugen.
- Von LLMs erstellte/geänderte Notizen bekommen ein Frontmatter-Flag
  (`author: ai`) → in UI und Graph erkennbar, Review-Ansicht "Was hat die KI
  zuletzt geschrieben?".
- Schreibzugriff per Setting abschaltbar bzw. auf Ordner begrenzbar.

## UI-Konzept (das Anti-Obsidian)

- **Zwei Panes statt Fensterchaos**: Sidebar (Notizenliste + Tags + Suche) und
  Editor.
- **Ein Editor-Modus**: Live-Markdown, Syntax versteckt sich beim Schreiben;
  `/`-Menü für Blöcke; Bilder per Paste.
- **Command Palette** (`Cmd/Ctrl+K`) für alles; **Quick Capture** per globalem
  Hotkey.
- **Schöne Defaults**: kuratierte Typo-/Farbpalette, Light + Dark.
- Backlinks-Panel dezent am Notiz-Ende; **lokaler Mini-Graph** pro Notiz.
- **Graph-View als erstklassige Ansicht**: flüssig, hübsch animiert, Filter nach
  Tags/Ordnern/Zeit, KI-erstellte Notizen farblich markiert.

## Meilensteine

| # | Meilenstein | Inhalt | Status |
|---|---|---|---|
| M0 | Scaffold | Tauri 2 + React + CI (macOS/Windows) | ✅ |
| M1 | Vault + Editor | Ordner öffnen, Notizen anlegen/umbenennen/löschen, Live-Markdown mit versteckter Syntax, Bild-Paste, Autosave | ✅ |
| M2 | Links + Suche + Graph | `[[Wikilinks]]` mit Autocomplete + Cmd/Ctrl-Klick, Backlinks-Panel, Volltextsuche, Graph-View (Canvas-Force-Layout, KI-Notizen markiert) | ✅ |
| M3 | MCP-Server + KI-Mitautor | Eingebauter stdio-MCP-Server (`crates/magma-mcp`), `find_link_candidates`, Link-Validierung mit Vorschlägen, `author: ai`-Stempel, „Mit Claude verbinden"-Config in Settings | ✅ |
| M4 | Polish | Anpassbare Themes ✅, System-Dark-Mode ✅, i18n (DE/EN) ✅, Ein-Klick-MCP-Setup ✅, Bild-Paste ✅, Command Palette (Cmd/Ctrl+P) ✅, Quick Capture (Cmd/Ctrl+Shift+N) ✅, Onboarding ✅, KI-Review-Ansicht ✅ | ✅ |
| M7 | Zweites Gehirn im Alltag | Tagesnotizen + Kalender ✅, Vorlagen mit Platzhaltern ✅, Versionsverlauf mit Diff und Wiederherstellen ✅, ausgehende Links + unverlinkte Erwähnungen ✅, ähnliche Notizen (TF-IDF) ✅ | ✅ |
| M5 | Packaging | Installer (DMG/MSI), CI-Download-Artefakte ✅, Auto-Update, Code Signing | 🟡 |
| M6 | Online-/Remote-Vault | Vault auf Webserver (WebDAV): Sync in lokalen Cache, Write-Through beim Speichern, Settings-UI, https-Pflicht | 🟡 erste Version |
| M8 | Lokales RAG | Index + Watcher, Passagen statt ganzer Notizen, hybride Rangliste, lokale Embeddings, `retrieve` über MCP. Bleibt vollständig lokal. | 🟡 Phase 2 fertig |

## M8 — Lokales RAG

Ziel: Der Vault wird zur Wissensquelle, aus der ein Modell antworten kann, ohne
dass ein einziges Byte den Rechner verlässt.

**Die Grundentscheidung steht vorab und ist nicht verhandelbar: Magma bleibt
lokal.** Kein Cloud-Dienst, kein Tunnel, keine Einbettungs-API, kein Endpunkt,
der über den eigenen Rechner hinaus erreichbar ist. Jede Anforderung unten ist
daran gemessen.

### Was schon da ist, und was daran fehlt

`links::search` sucht Volltext (wörtlich oder Regex), `related::related_notes`
macht TF-IDF mit Kosinus, der MCP-Server reicht beides durch. Ein Modell kann
den Vault also heute schon durchsuchen. Vier Dinge trennen das von RAG:

1. **Die Einheit ist die Notiz, nicht die Passage.** `search_notes` liefert
   einen Treffer je Datei mit Ausschnitt. Eine lange Notiz kommt ganz oder gar
   nicht. Ein Modell braucht Abschnitte mit Herkunftsangabe.
2. **Es findet nur, was wörtlich dasteht.** `related.rs` sagt es selbst: es
   weiß nicht, dass „Auto" und „Fahrzeug" dasselbe meinen. Bei einem deutschen
   Vault ist das die größte Einzellücke, Komposita verschärfen sie.
3. **Es gibt keinen Index.** Jede Suche liest den ganzen Vault von der Platte
   (Issue #18). Heute unangenehm, für RAG ein Ausschlusskriterium: Embeddings
   bei jeder Anfrage neu zu rechnen ist nicht machbar.
4. **Es gibt kein gemeinsames Ranking.** Volltext- und Ähnlichkeitstreffer
   stehen nebeneinander, nie in einer Rangliste.

### Die eine echte Entscheidung: womit gerechnet wird

Alles andere ist Handwerk. Zwei ernsthafte Wege für die Embeddings:

- **ONNX Runtime (`ort`).** Schnell und ausgereift, bindet aber eine
  C++-Bibliothek ein. Für den Universal-Build muss sie für beide Architekturen
  vorliegen, und sie braucht beim Signieren ihre eigene Signatur. Das macht die
  Kette aus M5 sofort komplizierter.
- **`candle`, reines Rust.** Kein C++-Runtime, keine zweite Binärdatei, der
  Build bleibt wie er ist. Langsamer, und die Modellauswahl ist enger.

**Gewählt: `candle`**, und zwar nicht aus Geschwindigkeitsgründen, sondern weil
Geschwindigkeit hier kein Problem ist. Bei einem Vault dieser Größe dauert der
einmalige Durchlauf Minuten, danach werden nur geänderte Notizen neu gerechnet.
Dafür eine C++-Abhängigkeit und eine zweite zu signierende Datei einzuhandeln,
wäre schlecht getauscht.

Das Modell muss mehrsprachig sein, sonst nützt es einem deutschen Vault nichts.
Es wird **nicht mitgeliefert**, sondern beim ersten Einschalten geladen und im
App-Datenordner abgelegt: Der Installer bleibt bei rund 12 MB, und wer den
Download ablehnt, behält TF-IDF, das ja funktioniert. Konkrete Modellgröße und
Auswahl gehören in Phase 3 gemessen statt hier geraten.

### Ablage

Der Index liegt im **App-Datenordner**, nach Vault-Pfad getrennt, nicht im
Vault. Der Vault bleibt reines Markdown, das ist das Versprechen des Projekts,
und ein Indexordner darin würde bei WebDAV (M6) und bei fremden Editoren sofort
Ärger machen. Ein Neuaufbau muss jederzeit möglich sein: Der Index ist
abgeleitete Information, nie die Quelle.

### Phasen

Jede Phase ist für sich nützlich. Wer nach Phase 2 aufhört, hat trotzdem etwas
Besseres als heute.

**Phase 1 — Index, Watcher, Messlatte.** Die Prüfsammlung steht (siehe unten)
und läuft als Test in der CI mit; Index und Watcher warten auf die Messung des
echten Vaults, weil davon abhängt, ob sie überhaupt dringend sind. Das ist
Issue #18 und die
Voraussetzung für alles Weitere: In-Memory-Index plus `notify`-Watcher auf
Dateiänderungen, Notizen werden über einen Hash nur bei echter Änderung neu
verarbeitet. Schon ohne Embeddings werden Suche und Ähnlichkeit dadurch
spürbar schneller.

In dieselbe Phase gehört die **Prüfsammlung** (siehe unten), bevor irgendetwas
am Ranking gedreht wird.

Erste Aufgabe, weil davon abhängt, ob der Index überhaupt dringend ist:

```bash
find <vault> -name '*.md' | wc -l
find <vault> -name '*.md' -print0 | xargs -0 cat | wc -c
```

**Phase 2 — Passagen und hybride Rangliste, noch ohne Embeddings.** ✅ Notizen
werden an Überschriften und Absätzen entlang zerlegt, nicht stur nach
Zeichenzahl, mit Überlappung an den Schnittstellen. BM25 statt reinem
Substring-Match. Das MCP-Werkzeug `retrieve` gibt eine Rangliste von Passagen
mit Notiz, Überschrift und Zeile zurück statt einer Liste von Dateien.
Implementiert in `crates/magma-core/src/retrieval.rs`.

Beim Bauen kam etwas dazu, das in diesem Plan fehlte und wichtiger ist als das
Synonym-Problem: **deutsche Beugung**. Der Prüfsatz fiel sofort um, weil „wie
exportiere ich das Zertifikat" eine Notiz mit „die p12 muss aus Meine
Zertifikate exportiert werden" nicht erreichte. Kein einziges Wort der Frage
passte. Handgeschriebene Endungsregeln wären dieselbe Falle gewesen wie die
Stoppwortliste im Begriffsgraphen, also stemmt die Zerlegung jetzt mit Snowball
(`rust-stemmers`, die vierte Abhängigkeit von `magma-core`).

Was das nachweislich behebt und was nicht, gemessen statt angenommen:
Substantivformen fallen zusammen (`Zertifikat`/`Zertifikate`/`Zertifikats`,
`Farbe`/`Farben`), Partizipien auf `-iert` nicht (`exportiere` und
`exportieren` werden beide zu `exporti`, `exportiert` bleibt stehen), und ein
Substantiv trifft sein Verb nicht (`Import` gegen `importieren`). Beide Lücken
sind als Tests festgehalten, damit sie niemand zufällig wiederfindet.

**Phase 3 — Embeddings.** Jetzt erst, und hinter einer Schnittstelle, die es
noch gar nicht gibt: Der Kommentar in `related.rs` nennt sie `Similarity`, M7
oben nennt sie `RelatedNote`, tatsächlich existiert keine von beiden. Diese
Phase legt sie an. Semantische Treffer **ergänzen** die lexikalischen, sie
ersetzen sie nicht: Reine Vektorsuche ist bei Eigennamen, Codebezeichnern und
exakten Begriffen schlechter als Volltext, und beides steht in echten Notizen.

Ob die Embeddings einem echten deutschen Vault etwas bringen, ist damit noch
nicht beantwortet. Zwei Synonymfragen auf dem echten Vault gingen daneben, und
eine Rangliste sagt nicht, warum: Ein Treffer kann fehlen, weil das Modell ihn
nirgends in der Nähe der Frage sah, oder weil es ihn gut platzierte und die
Fusion oder die Deckelung pro Notiz ihn wieder herauswarf — das verlangt
entgegengesetzte Reparaturen. Deshalb hat `retrieve` ein `explain`: die beiden
Ranglisten getrennt, so weit die Fusion überhaupt hinsieht, und davor
`embedded` von `passages`. Die Zahl steht dort, weil eine Anfrage den Vault
nicht kodiert, sondern nachschlägt; eine Passage, die der Indexlauf nie
erreicht hat, kommt als Nullvektor zurück und bekommt gegen alles die Null.
In der Rangliste sieht das genauso aus wie ein Modell, das nichts gefunden hat.
Erst wenn `embedded` nahe bei `passages` liegt, ist die Frage nach der Qualität
der Embeddings überhaupt gestellt.

Gemessen auf dem echten Vault, 852 Notizen, 7082 Passagen:

- `embedded` war 7058. Der Index hat keine Löcher; die Vermutung, an der ich
  zuerst hing, ist damit widerlegt.
- Die `meaning`-Liste ist **thematisch richtig** und die `lexical`-Liste ist es
  nicht. Auf „Wer hat an der automatischen Aktualisierung mitgeholfen?" fand
  die Wortsuche 45 Blogartikel, die Wortstämme teilen und sonst nichts; das
  Modell fand n8n Sync-Automation, „Claude, mein neuer Systemadministrator",
  „dhw Radio Betrieb" — Notizen, in denen etwas automatisch aktualisiert wird,
  ohne dass das Wort dort steht. Genau die Lücke, für die Phase 3 gebaut wurde.
- Die eine Notiz, die die Frage beantwortet, stand trotzdem in keiner der
  beiden Listen. Mit ihren *eigenen* Wörtern gefragt („Updater beigesteuert,
  Mitgewirkt") steht sie auf Rang 1. Ihr Vektor ist also in Ordnung, und die
  Kürzung auf 256 Token hat sie nicht verstümmelt.

Gemessen: semantischer Rang **77 von 7082**. Ein größeres Fusionsfenster hätte
nicht geholfen — bei Tiefe 100 bekäme die Passage 1/138 und läge fusioniert um
Platz 154, weil RRF einen Treffer, den nur eine Hälfte kennt, zu Recht nicht
über die Erstplatzierten der anderen hebt.

Der Verdächtige war stattdessen `embedding_text`, also ich selbst: Es stellt
jeder Passage ihren vollen Notizpfad voran. Bei einer Passage von dreizehn
Wörtern ist das die Hälfte des kodierten Textes, und die Kosinuswerte liegen so
eng beieinander, dass 0,0165 in derselben Notiz 644 Ränge ausmachten. Gemessen
auf dem echten Vault, dieselbe Passage ohne Pfad: **Rang 77 → 6**, und sechs von
sieben Passagen der Notiz steigen.

Was ein Indexlauf kostet, ist damit auch gemessen und nicht mehr geschätzt:
**rund 15 Minuten** für 7082 Passagen auf dem Windows-PC, nach den Korrekturen
an Speicherbudget und Parallelität. Vorher waren es 592 Passagen in 15 Minuten,
hochgerechnet drei Stunden. Das ist die Zahl, gegen die jede Änderung an
`embedding_text` oder an der Modell-ID abzuwägen ist: teuer genug, um sie nicht
nebenbei zu machen, billig genug, um sie zu machen, wenn eine Messung sie
rechtfertigt.

Der Pfad kam trotzdem nicht ersatzlos weg — er wurde selbst gegen einen
gemessenen Fehler eingebaut (`Alexander Mut.md`, deren Thema nur im Dateinamen
steht). Weil eine Umstellung jede Passage im Vault neu kodiert und eine falsche
Wahl das zweimal kostet, hat `explain_note` alle Kandidaten auf einmal gemessen.
Zwei Notizen, zwei Fragen, Rang von 7082:

| vor der Passage | „Mitgewirkt", 13 Wörter | `Alexander Mut`, 13 Wörter |
|---|---|---|
| nur die Überschrift | **2** | **1** |
| nichts | 6 | 35 |
| Notizname + Überschrift | 31 | 1 |
| voller Pfad + Überschrift (bis dahin) | 77 | 1 |

**Die Leiter ist nicht monoton, und das ist der Befund.** Ein Wort Überschrift
schlägt gar keinen Kontext; acht Wörter Pfad machen es viel schlechter. Es geht
nicht um die Menge, sondern darum, was das Label beschreibt: „Mitgewirkt"
beschreibt die Passage, `Projekte / Magma / Projekt / Magma 0.1.4` beschreibt
die Datei — und sagt „Magma" zweimal, bevor die Passage anfängt. Auch der Fall,
der den Kontext überhaupt erst nötig machte, setzt die Überschrift nach vorn.

Der Kosinus lag dabei über alle vier Varianten zwischen 0,814 und 0,843. Drei
Hundertstel, fünfundsiebzig Ränge. Deshalb fiel es nie auf: An den Werten sieht
man es nicht, nur an der Reihenfolge.

`embedding_text` stellt einer Passage daher **ein** Label voran — ihre
Überschrift, ersatzweise den Notiznamen, wenn sie keine hat. Nie zwei. Der
Rückfall auf den Namen ist begründet und nicht gemessen; die Begründung ist
bewusst eng gehalten: Sonst stünde vor der Passage gar nichts, und das ist die
Zeile „nichts" oben.

Die lexikalische Hälfte behält Name **und** Ordner (`CONTEXT_REPEATS`,
unverändert). Dass beide Hälften denselben Kontext tragen müssen, folgt aus
nichts — im Gegenteil: Wörter finden Namen, Bedeutung findet Bedeutung. Dass
`embedding_text` den Dateinamen trug, war eine per Analogie von der einen auf
die andere Seite kopierte Reparatur, nie eine gemessene.

### Ergebnis nach der Umstellung

Neu indiziert, 7082 von 7082 Passagen, dieselbe Frage wie vorher:

- „Mitgewirkt" auf **semantischem Rang 1** statt 77. Die Schätzung erster
  Ordnung hatte 2 gesagt, lag also leicht zu vorsichtig.
- **Lexikalischer Rang 4935.** Die Wortsuche findet diese Passage praktisch
  nicht; alles, was sie nach vorn bringt, kommt aus der Bedeutungshälfte. Das
  ist die Lücke aus Punkt 2 oben, zum ersten Mal messbar geschlossen.
- Im Ergebnis selbst steht sie auf **Rang 4 von acht**, vorher gar nicht.
  Darüber stehen drei Passagen, die *beide* Hälften finden — RRF hebt
  Übereinstimmung über einen Fund aus nur einer Liste, und das ist so gewollt.
- Gegentest `Alexander Mut.md`: semantisch **und** lexikalisch weiter Rang 1.
  Der Umbau hat nichts eingetauscht.

Möglich wurde das durch `explain_note`: Es nimmt eine Notiz entgegen und meldet
für jede ihrer Passagen Rang und Wert in beiden Hälften, aus wie vielen, **ohne
Abschnitt**. Ohne das sah „knapp hinter dem Fenster" genauso aus wie „nirgends
in der Nähe", und die beiden verlangen entgegengesetzte Reparaturen.

### Was Phase 3 nicht kann, und was daraus folgt

Drei Gegentests auf dem echten Vault nach der Umstellung: Die Namensfrage
(`llmfit`) liefert Rang 1 und 2 — die Wortsuche ist unbeschädigt. Zwei
Detailfragen fallen durch, und zwar identisch: Ziel jeweils auf
`meaningRank 17` von 7082, in keinem Top-8.

Das ist **nicht** die Fusion. Eine Passage auf semantischem Rang 17 bekommt
`1/77`; jede Passage auf den semantischen Rängen 1–16 bekommt mindestens
`1/76`. Sechzehn stehen also allein dadurch davor, egal was die Wortsuche sagt.
In acht Plätze passt das nie — auch reine Vektorsuche mit `limit 8` verfehlte
sie.

Der Grund steht in der Liste selbst: Auf die Frage nach dem Bildprompt mit der
Italienerin mit lockigen schwarzen Haaren waren die ersten sechzehn Treffer
**sechzehn Bildprompt-Notizen**. Kein Ausreißer. Das Modell trifft das *Thema*
perfekt und die *Merkmale* gar nicht — die bekannte Eigenschaft kleiner
Bi-Encoder, die eine Passage einmal vorab in einen Vektor pressen müssen, ohne
die Frage zu kennen. Und die Wortsuche, die „curly black long hair" sofort
fände, scheitert an der Sprachgrenze: deutscher Vault, englische Prompts.

**Beide Hälften versagen hier aus jeweils dem Grund, aus dem die andere gebaut
wurde.** Deshalb eine dritte Stufe: ein Cross-Encoder, der Frage und Passage
*gemeinsam* liest und die besten fünfzig neu sortiert. Er kann nicht
vorberechnet werden — das ist der Preis, ein bis drei Sekunden pro Abfrage —
und er kann nur umsortieren, was die beiden Hälften gefunden haben. Trefferquote
bleibt ihre Aufgabe, Genauigkeit wird seine.

Eigener Download, unabhängig vom Encoder. Und er zählt erst als bereit, wenn er
seinen Selbsttest besteht: Ein Cross-Encoder mit vertauschten Labels setzt den
schlechtesten Kandidaten nach vorn und sieht dabei aus wie ein funktionierendes
Feature.

Gemessen auf dem echten Vault:

| Frage | vorher | mit Nachsortierung |
|---|---|---|
| Bildprompt „Italienerin, lockige schwarze Haare" | nicht unter acht Treffern | **Rang 1** |
| „Wer hat an der automatischen Aktualisierung mitgeholfen?" | Rang 4 | **Rang 1** |
| „was ist llmfit" (Wächter, Namensfrage) | Rang 1 und 2 | **unverändert** |

Der Wächter ist der wichtigste der drei. Ein Cross-Encoder fasst *jede* Abfrage
an, und einer, der Namensfragen verschlechtert, kostet mehr als er bringt.

Der `meaningRank` der Bildprompt-Notiz blieb dabei 17 — der Re-Ranker ändert
keine Vektoren. Sie stand nicht in den Top 8 und steht jetzt an erster Stelle,
allein weil ein zweites Modell die fünfzig Kandidaten gelesen hat.

Ein vierter Test war **ungültig, nicht durchgefallen**, und das gehört
festgehalten, weil es ein wiederkehrender Fehler ist: Die Frage wurde aus einem
Notiz*titel* abgeleitet, ohne den Inhalt zu kennen. „Ich hab einen API-Key
gefunden. Was jetzt?" liest sich wie ein Sicherheitsvorfall und ist eine
Bau-Geschichte. Der Re-Ranker hat die Notiz zurückgewiesen und Notizen über
Prompt Injection geliefert — für die gestellte Frage die bessere Antwort.

Nebenbei, und schärfer als zuerst notiert: Nicht das **Vorzeichen** des Logits
trägt die Information, sondern der **Abstand** zum Rest.

| Frage | Spitze | dahinter | traf zu |
|---|---|---|---|
| „Was sagte Musk in Davos 2026?" | +7,12 | +1,20 | ja |
| „Wer hat mitgeholfen?" | −0,58 | −1,7 | ja |
| die ungültige Frage | −3,5 | −4,1 | nichts passte |

Die mittlere Zeile ist der Grund, hier keinen Schwellwert einzubauen: Eine Regel
„negativ heißt nichts gefunden" hätte bei null geschnitten und die *richtige*
Antwort verworfen. Ein Ergebnis ohne Abstand nach oben heißt „im Vault steht die
Antwort nicht" — und das steht als Zahl im Ergebnis, wo es jeder liest, statt
als erfundene Grenze im Code.

**Phase 4 — Zugang für beliebige Modelle, lokal.** Über stdio funktioniert das
heute schon mit allem, was MCP spricht und auf demselben Rechner läuft, also
auch mit einem lokalen Modell in LM Studio oder Ollama. Ergänzend ein
HTTP-Transport, **gebunden an `127.0.0.1`**, mit einem Token, das Magma erzeugt
und anzeigt, für lokale Programme ohne stdio-Unterstützung. Standard bleibt
**nur lesend**; der Schalter dafür existiert bereits als
`MAGMA_MCP_ALLOW_WRITE=0`.

Ausdrücklich nicht Teil davon: Erreichbarkeit über den eigenen Rechner hinaus.
Es gab in diesem Projekt bereits einen Fehler dieser Klasse (`safe_join` prüfte
nur auf `..`, absolute Pfade gingen durch, in v0.1.0 und v0.1.1 war damit über
MCP jede Datei der Platte les- und überschreibbar). Lokal kostet so etwas
wenig, über Netz alles.

### Was bewusst nicht gebaut wird

Keine Vektordatenbank als Abhängigkeit und kein ANN-Index wie HNSW: Bei dieser
Größenordnung passt der ganze Vektorsatz in den Arbeitsspeicher, und ein
linearer Durchlauf dauert Millisekunden. Kein zweites Modell zum Nachsortieren,
bevor gemessen ist, dass es fehlt. Keine Zusammenfassung von Passagen durch ein
LLM beim Indizieren, das erfindet Fehler, die später niemand mehr findet.

### Die Prüfsammlung, und warum sie nicht ans Ende gehört

RAG ist die perfekte Umgebung für genau die Fehlerklasse, die dieses Projekt am
häufigsten getroffen hat: **Es macht nie etwas kaputt, was auffällt.** Eine
Suche liefert immer Treffer, ein Modell antwortet immer, und ob die richtigen
Passagen dabei waren, sieht man der Antwort nicht an. Ein Test, der prüft, dass
`retrieve` fünf Ergebnisse zurückgibt, ist ein Freispruch, den man sich selbst
ausstellt, genau wie der Test, der nur die ersten vier Schattierungsschritte
ansah.

Deshalb in Phase 1, nicht am Schluss: zwanzig bis dreißig Fragen aus dem echten
Vault, zu denen bekannt ist, welche Notiz die Antwort enthält. Daraus eine Zahl,
wie oft die richtige Notiz unter den ersten fünf Treffern steht. Dann ist jede
Änderung an Zerlegung, Ranking oder Modell eine Zahl, die steigt oder fällt,
statt eines Gefühls. Ohne das lassen sich Wochen in Feintuning stecken, ohne je
zu erfahren, ob es besser geworden ist.

## M7 — Notizen, die sich selbst vernetzen

Fünf Bausteine, jeweils an dem Obsidian-Plugin orientiert, das die Lücke dort
füllt — aber eingebaut statt installiert:

- **Tagesnotizen + Kalender** (*Calendar / Periodic Notes*): eine Notiz pro Tag,
  benannt `2026-07-26`. Der Kalender in der Seitenleiste füllt Tage, die schon
  existieren, und legt fehlende beim Klick an. Ordner und Vorlage frei wählbar.
- **Vorlagen** (*Templater*): jede Notiz im Vorlagen-Ordner erscheint in der
  Befehlspalette als „Neue Notiz aus: …". Platzhalter `{{date}}`, `{{time}}`,
  `{{title}}`, `{{weekday}}`, `{{month}}`, `{{year}}`.
- **Versionsverlauf** (*File Recovery*): Snapshots unter `.magma/history`,
  höchstens einer alle zwei Minuten beim Tippen, aber garantiert vor jedem
  vault-weiten Ersetzen. Diff-Ansicht, Wiederherstellen ist selbst rückgängig
  zu machen. Umbenennen und Verschieben nehmen den Verlauf mit.
- **Ausgehende Links + unverlinkte Erwähnungen** (*Backlinks, zweite Hälfte*):
  „Links raus" zeigt auch Ziele, die es noch nicht gibt. „Erwähnungen" findet
  Notizen, die den Namen im Text nennen, ohne zu verlinken — einzeln oder alle
  auf einmal verlinkbar. So wächst der Graph von selbst.
- **Ähnliche Notizen** (*Smart Connections*): TF-IDF über den Vault, Vergleich
  per Kosinus. Ehrlich benannt: das ist lexikalische Ähnlichkeit, keine
  semantische — es findet Notizen mit denselben Wörtern, nicht Notizen mit
  derselben Bedeutung. Dafür ohne Modell-Download, ohne ONNX-Runtime und ohne
  Netz. `RelatedNote` ist die Nahtstelle, an der später echte Embeddings
  einsteigen können, ohne dass ein Aufrufer sich ändert.

Der MCP-Server bekommt dieselben vier Werkzeuge (`related_notes`,
`unlinked_mentions`, `link_mentions`, `list_outgoing_links`) — damit Claude
nicht nur schreibt, sondern auch aufräumt. Ein Snapshot wird vor jeder
KI-Änderung angelegt.

## M6 — Online-/Remote-Vault (Design)

Ziel: Der Vault kann optional auf einem Webserver liegen; der User trägt den Ort
in den Settings ein, und jeder Rechner mit Magma greift darauf zu — Daten
überall verfügbar, ohne dass Magma selbst eine Cloud betreibt.

**Empfohlener Ansatz: WebDAV.** Ein Vault ist ein Ordner mit `.md`-Dateien —
WebDAV bildet genau das über HTTP ab und wird von gängigem Webspace, Nextcloud,
Synology u. v. m. unterstützt. Vorteile: offener Standard, kein eigener
Server-Code nötig, funktioniert mit dem bestehenden „Ordner mit Dateien"-Modell.

- **Storage-Abstraktion**: `magma-core` bekommt ein `VaultBackend`-Trait
  (`list_notes`, `read_note`, `write_note`, …). Heute: `LocalBackend` (Dateisystem).
  Neu: `WebDavBackend` (HTTP via `reqwest`, Basic/Bearer-Auth). Die ganze bestehende
  Logik (Links, Graph, Suche, KI-Mitautor) läuft unverändert über das Trait.
- **Settings**: Vault-Quelle wählbar — „Lokaler Ordner" oder „Remote (WebDAV)"
  mit URL + Zugangsdaten. Credentials verschlüsselt im OS-Keychain (Tauri
  `keyring`), nicht im Klartext.
- **Lokaler Cache + Offline**: Remote-Dateien werden lokal gecacht; Schreibvorgänge
  gehen an den Server und aktualisieren den Cache. Einfache
  Last-Write-Wins-Auflösung mit `ETag`/`Last-Modified`-Prüfung; bei Konflikt eine
  `.conflict`-Kopie statt Datenverlust.
- **MCP**: Der MCP-Server nutzt dieselbe Storage-Abstraktion, kann also auch gegen
  einen Remote-Vault arbeiten.

**Status (erste Version gebaut):** `crates/magma-webdav` implementiert den
Sync-Ansatz (kein Trait-Refactor nötig): PROPFIND-Listing, Download in einen
lokalen Cache, `PUT`/`DELETE` fürs Zurückschreiben; HTTPS wird erzwungen,
Basic-Auth. Die Tauri-Commands `remote_connect`/`remote_put`/`remote_delete`
(in `src-tauri/src/lib.rs`) synchronisieren beim Verbinden in
`app_data_dir()/remote-vaults/<hash>` und schreiben Änderungen beim Speichern
zurück (Write-Through, best-effort). Settings-UI mit URL/Benutzer/Passwort.

**Noch offen (Follow-ups):** Passwort im OS-Keychain statt nur Session-Speicher;
echtes Konflikt-Handling via `ETag`/`Last-Modified` (aktuell Last-Write-Wins);
periodisches Re-Sync/Pull; Löschen entfernter Dateien beim Pull.

## Verifikation

- M0: `npm run tauri dev` startet die App lokal; CI baut Frontend + Core-Tests +
  Desktop-Bundles für macOS + Windows.
- Ab M1: Rust-Unit-Tests für Indexer/Parser (`cargo test -p magma-core`),
  Vitest für UI-Logik.
- M2: Graph gegen einen großen Test-Vault (1000+ Notizen) auf Flüssigkeit prüfen.
- M3: MCP-Server mit MCP Inspector bzw. Claude Desktop gegen einen Test-Vault
  verifizieren — inkl. End-to-End-Test: Claude legt eine Notiz an und verlinkt
  sie korrekt mit bestehenden Notizen.
