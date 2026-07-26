# Magma

[English](README.md) · **Deutsch**

**Dein zweites Gehirn – ohne Einrichtungsaufwand.** Eine schöne, lokal-zuerst
arbeitende Notiz-App mit der Grundidee von Obsidian – einfache Markdown-Dateien,
`[[Wikilinks]]`, Backlinks und ein Graph – aber einfacher zu bedienen und von
Anfang an LLM-nativ.

Magma ist für zwei Autoren gebaut: **dich** und **deine KI**. Es bringt einen
eingebauten MCP-Server mit, damit Claude (und andere Agenten) deinen Vault lesen
*und* neue Notizen schreiben können, die korrekt und logisch mit dem Bestehenden
verlinkt sind – keine verwaisten Fragmente.

> Status: früh, aber nutzbar. Meilensteine **M0–M3** sind drin: Notizen anlegen/
> bearbeiten/löschen, Live-Markdown-Editor, `[[Wikilinks]]` mit Autovervollstän-
> digung, Backlinks, Volltextsuche, Graph-Ansicht und ein eingebauter MCP-Server
> für Claude. Dazu Flammen-Icon, Startbildschirm, ein Info-/Einstellungen-Panel
> und Deutsch/Englisch. Siehe [`docs/PLAN.md`](docs/PLAN.md) für Recherche,
> Roadmap und das Remote-Vault-Design (M6).

## Warum Magma

Menschen lieben Obsidian dafür, ihre Notizen als lokale Markdown-Dateien zu
besitzen, und für die Art, wie Ideen sich verbinden (Links + Graph). Sie scheitern
an der steilen Lernkurve, der rohen Standard-UI und der Plugin-Sucherei für
Grundfunktionen. Magma behält, was geliebt wird, und behebt, was nicht:

- **Hält dich frei** – ein Vault ist nur ein Ordner mit `.md`-Dateien.
  Obsidian-Vaults öffnen sich unverändert (`[[Wikilinks]]`, YAML-Frontmatter, Tags).
- **Kein Einrichtungsritual** – eine saubere, durchdachte Standard-UI. Live-Markdown,
  bei dem sich die Syntax beim Tippen versteckt. Kein Edit/Vorschau-Umschalten,
  keine Theme-Sucherei.
- **Der Graph ist ein Hauptfeature** – sieh, wie sich dein Wissen verbindet, inklusive
  dessen, was deine KI beigetragen hat.
- **LLM als Mitautor** – eingebauter MCP-Server. Die KI schlägt Links gegen deine
  echten Notizen vor, und der Server validiert jeden `[[Link]]`, bevor er
  geschrieben wird.

## Technik

- **Tauri 2** (Rust) – klein, schnell, nativ auf macOS + Windows.
- **React + TypeScript + Tailwind** – die Oberfläche.
- **CodeMirror 6** – der Live-Markdown-Editor.
- **`crates/magma-core`** – plattformunabhängige Vault-Logik, geteilt von
  Desktop-Shell und MCP-Server. Vollständig unit-getestet.

## Entwickeln

Voraussetzungen: Node 20+, Rust (stable) und die
[Tauri-Systemabhängigkeiten](https://tauri.app/start/prerequisites/) für dein
Betriebssystem.

```bash
npm install          # Frontend-Abhängigkeiten
npm run tauri dev    # Desktop-App starten
```

Weitere nützliche Befehle:

```bash
npm run build              # Frontend typprüfen + bauen
cargo test -p magma-core   # Vault-/Core-Unit-Tests
npm run tauri build        # Desktop-Bundle erzeugen (DMG / MSI)
```

## Mit Claude verbinden

Magma ist sein eigener MCP-Server – keine separate Installation. Öffne in der App
**Einstellungen → Mit Claude verbinden → Claude Desktop einrichten**. Ein Klick
schreibt die Konfiguration (mit Backup einer bestehenden); starte Claude Desktop
neu, fertig.

Für andere MCP-Clients kopiere das JSON unter *Manuelle Einrichtung* – es startet
die Magma-Anwendung als Server:

```json
{
  "mcpServers": {
    "magma": { "command": "/pfad/zu/Magma", "args": ["--mcp", "/pfad/zum/vault"] }
  }
}
```

Die Schreib-Tools leiten das Modell an, zuerst `find_link_candidates` aufzurufen
und dann jeden geschriebenen `[[Wikilink]]` zu validieren – kaputte Links kommen
mit Korrekturvorschlägen zurück statt als tote Enden. KI-Notizen werden mit
`author: ai` gekennzeichnet und im Graph violett dargestellt. Mit
`MAGMA_MCP_ALLOW_WRITE=0` läuft der Server schreibgeschützt.

## Jeden Tag, und jede Verbindung

- **Befehlspalette** — `Cmd/Strg+P` öffnet alles: Notiz springen, Befehl
  ausführen, aus einer Vorlage starten. Gesucht wird als Teilfolge, `grph`
  findet also „Graph anzeigen". (`Cmd/Strg+K` bleibt im Editor die Link-Taste.)
- **Schnellnotiz** — `Cmd/Strg+Umschalt+N` hält einen Gedanken in der Notiz von
  heute fest, ohne dass du wegmusst.
- **Tagesnotizen und Kalender** — eine Notiz pro Tag, benannt `2026-07-26`. Der
  Kalender in der Seitenleiste füllt die Tage, die es gibt, und legt die an, die
  fehlen.
- **Vorlagen** — jede Notiz im Vorlagen-Ordner erscheint in der Palette als
  „Neue Notiz aus: …". `{{date}}`, `{{time}}`, `{{title}}`, `{{weekday}}`,
  `{{month}}` und `{{year}}` werden eingesetzt.
- **Versionsverlauf** — Magma legt vor jeder größeren Änderung eine Kopie an,
  vor einem vault-weiten Ersetzen immer. Diff-Ansicht, Wiederherstellen mit
  einem Klick, und das Wiederherstellen ist selbst rückgängig zu machen.
  Umbenennen oder Verschieben nimmt den Verlauf mit.
- **Verbindungen unter dem Editor** — Backlinks, ausgehende Links (auch solche,
  deren Ziel es noch nicht gibt), *unverlinkte Erwähnungen* (Notizen, die den
  Namen schreiben, ohne zu verlinken — einzeln oder alle auf einmal verlinkbar)
  und ähnliche Notizen.
- **Ähnliche Notizen** — TF-IDF über den Vault, Vergleich per Kosinus. Ehrlich
  benannt: das ist lexikalische Ähnlichkeit, keine semantische. Es findet
  Notizen mit denselben Wörtern, nicht Notizen mit derselben Bedeutung — dafür
  ohne Modell-Download, ohne Runtime und ohne Netz.
- **Was Claude geschrieben hat** — eine Seite mit allen Notizen, die
  `author: ai` tragen, neueste zuerst. Eine KI in den eigenen Vault zu lassen
  ist nur dann angenehm, wenn man genau sieht, was sie angefasst hat.

## Suchen & ersetzen im ganzen Vault

Eine Formulierung in 500 Notizen zu ändern ist ein Dialog. Geschrieben wird
nichts, bevor du die Vorschau gesehen hast: jede betroffene Notiz mit ihrer
Trefferzahl. Wikilinks lösen über den *Dateinamen* auf — würde nur der Text
ersetzt, zeigte jedes `[[…]]` ins Leere. Magma benennt deshalb zuerst die Notiz
um, die den Begriff im Namen trägt, biegt ihre Links mit (samt Alias und Anker)
und schreibt erst danach den Text.

## Anpassen

Alles Visuelle ist unter **Einstellungen → Darstellung** anpassbar: Hell-/Dunkel-/
System-Modus, Akzent- und KI-Notiz-Farben, Oberflächen- und Editor-Schrift,
Schriftgröße und Lesebreite. Änderungen wirken sofort und bleiben erhalten.
Schriften nutzen nur, was bereits auf deinem System vorhanden ist – nichts wird
heruntergeladen.

## Remote-Vault (optional)

Richte Magma auf eine WebDAV-URL aus (Einstellungen → Remote-Vault), um einen
Vault auf einem Webserver zu halten und von jedem Rechner zu bearbeiten. Magma
synchronisiert ihn in einen lokalen Cache und überträgt deine Änderungen beim
Speichern zurück. HTTPS ist Pflicht; das Passwort wird nur für die Sitzung
behalten (Speicherung im OS-Schlüsselbund ist als Folgeschritt geplant).

## Aufbau

```
src/                 React-UI (Sidebar, Live-Markdown-Editor, Graph, Einstellungen)
src-tauri/           Tauri-Desktop-Shell – dünne Command-Schicht über magma-core
crates/magma-core/   Reine Rust-Vault-Logik: Notizen, Links, Graph, Suche und die
                     KI-Mitautor-Regeln – auf jeder Plattform testbar
crates/magma-mcp/    Eingebauter MCP-Server (stdio JSON-RPC) über magma-core
crates/magma-webdav/ Optionaler Remote-Vault: WebDAV-Ordner in lokalen Cache syncen
docs/PLAN.md         Produktplan, Recherche und Roadmap
```

## Lizenz

Siehe [LICENSE](LICENSE).
