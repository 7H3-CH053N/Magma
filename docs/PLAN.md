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
| M0 | Scaffold | Tauri 2 + React + CI (macOS/Windows) | ✅ dieser PR |
| M1 | Vault + Editor | Ordner öffnen, Notizen CRUD, Live-Markdown-Editor, Autosave | 🟡 Grundgerüst steht |
| M2 | Links + Suche + Graph | `[[Wikilinks]]`, Backlinks, FTS5-Suche, Tags, Daily Notes, Graph-View | ⬜ |
| M3 | MCP-Server + KI-Mitautor | Server, semantische Suche, `find_link_candidates`, Link-Validierung, `author: ai` | ⬜ |
| M4 | Polish | Command Palette, Quick Capture, Bild-Paste, Dark Mode, Onboarding | ⬜ |
| M5 | Packaging | Installer (DMG/MSI), Auto-Update, Code Signing | ⬜ |

## Verifikation

- M0: `npm run tauri dev` startet die App lokal; CI baut Frontend + Core-Tests +
  Desktop-Bundles für macOS + Windows.
- Ab M1: Rust-Unit-Tests für Indexer/Parser (`cargo test -p magma-core`),
  Vitest für UI-Logik.
- M2: Graph gegen einen großen Test-Vault (1000+ Notizen) auf Flüssigkeit prüfen.
- M3: MCP-Server mit MCP Inspector bzw. Claude Desktop gegen einen Test-Vault
  verifizieren — inkl. End-to-End-Test: Claude legt eine Notiz an und verlinkt
  sie korrekt mit bestehenden Notizen.
