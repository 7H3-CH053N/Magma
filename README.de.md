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
