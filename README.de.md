# Magma

[English](README.md) · **Deutsch**

**Dein zweites Gehirn – ohne Einrichtungsaufwand.** Eine Notiz-App, die zuerst
auf deiner Festplatte lebt, mit der Grundidee von Obsidian – einfache
Markdown-Dateien, `[[Wikilinks]]`, Backlinks und ein Graph – aber einfacher zu
bedienen und von Anfang an für KI gebaut.

Magma ist für zwei Autoren gemacht: **dich** und **deine KI**. Ein MCP-Server
ist eingebaut, damit Claude deinen Vault lesen *und* neue Notizen schreiben
kann, die richtig mit dem verlinkt sind, was schon da ist – keine
Waisen-Notizen.

---

## Installation

Magma läuft auf **macOS** und **Windows**. Lade das Installationsprogramm für
dein System aus dem
[neuesten Release](https://github.com/7H3-CH053N/Magma/releases/latest):

| System | Datei | Was zu tun ist |
| --- | --- | --- |
| macOS | `Magma_x.y.z_*.dmg` | DMG öffnen, Magma nach *Programme* ziehen |
| Windows 10/11 (64-Bit) | `Magma_x.y.z_x64_en-US.msi` | Installer ausführen |

> **Hinweis zu v0.1.0:** Das DMG dieses Releases (`Magma_0.1.0_aarch64.dmg`)
> läuft **nur auf Apple Silicon** — GitHubs macOS-Runner sind ARM, und der Build
> wurde nicht als Universal-Binary angefordert. Auf einem Intel-Mac startet es
> nicht. Ab dem nächsten Release ist das DMG universal; auf einem Intel-Mac
> heute: aus dem Quellcode bauen (zwei Befehle, siehe unten).

**Die Builds sind noch nicht signiert**, deshalb warnen beide Systeme beim
ersten Start. Das ist bei einer App ohne Zertifikat normal – kein Zeichen
dafür, dass etwas nicht stimmt, aber vertrauen solltest du dem nur aus einer
Quelle, der du vertraust:

- **macOS** – beim ersten Start heißt es, Magma stamme „von einem nicht
  verifizierten Entwickler". Rechtsklick auf die App in *Programme* →
  **Öffnen** → **Öffnen**. Danach startet sie für immer normal. Wenn macOS die
  App stattdessen als „beschädigt" bezeichnet, muss die Quarantäne-Markierung
  weg: `xattr -dr com.apple.quarantine /Applications/Magma.app`
- **Windows** – SmartScreen zeigt „Der Computer wurde durch Windows
  geschützt". Auf **Weitere Informationen** → **Trotzdem ausführen**.

Lieber selbst bauen? Siehe [Aus dem Quellcode bauen](#aus-dem-quellcode-bauen) –
zwei Befehle, und es kommen genau diese Installationsprogramme heraus.

### Der erste Start

1. **Ordner wählen**, in dem deine Notizen liegen. Jeder Ordner geht, auch ein
   bestehender Obsidian-Vault – Magma liest dieselben `[[Wikilinks]]`, dasselbe
   YAML-Frontmatter und dieselbe Ordnerstruktur und schreibt nichts Eigenes
   hinein. Die Wahl wird gemerkt.
2. **`Cmd/Strg+P`** öffnet alles: Notiz springen, Befehl ausführen, aus einer
   Vorlage starten.
3. Optional: **Einstellungen → Claude → Claude Desktop einrichten**, um Claude
   dazuzuholen.

Deine Notizen bleiben `.md`-Dateien auf deiner Platte. Magma legt nur seine
eigene Buchhaltung (den Versionsverlauf) in einem versteckten `.magma`-Ordner im
Vault ab, sonst nichts.

---

## Warum Magma

Menschen lieben an Obsidian, dass ihre Notizen ihnen gehören, und wie sich Ideen
verbinden. Sie scheitern an der Lernkurve, der rohen Standard-Oberfläche und
daran, dass man für Grundfunktionen erst Plugins suchen muss. Magma behält, was
geliebt wird, und repariert, was nervt:

- **Kein Lock-in** – ein Vault ist ein Ordner mit `.md`-Dateien.
  Obsidian-Vaults öffnen sich so, wie sie sind.
- **Kein Einrichtungsritual** – eine durchdachte Standard-Oberfläche.
  Live-Markdown, bei dem die Syntax beim Tippen verschwindet. Kein Umschalten
  zwischen Bearbeiten und Vorschau, keine Theme-Suche.
- **Der Graph ist ein Hauptfeature** – sieh, wie sich dein Wissen verbindet,
  inklusive dem, was deine KI beigetragen hat.
- **KI als Mitautor** – der eingebaute MCP-Server prüft jeden `[[Link]]`, bevor
  er geschrieben wird. Eine KI kann also keine stillen Sackgassen anlegen.

## Was drin ist

**Schreiben**

- Live-Markdown-Editor: Formatierung erscheint beim Tippen, die Syntax versteckt
  sich selbst.
- `[[Wikilinks]]` mit Autovervollständigung, dazu ein dezenter Stift, um einen
  Link zu bearbeiten, statt ihm zu folgen. Ein Link auf eine noch nicht
  existierende Notiz wird gestrichelt dargestellt und legt sie beim Klick an –
  wie in der Wikipedia.
- Bilder direkt einfügen; sie landen im Vault.
- Tabellen, Aufgabenlisten, Zitate, Code – ohne Plugin.

**Sich zurechtfinden**

- **Befehlspalette** (`Cmd/Strg+P`) – Notizen, Befehle und Vorlagen in einem
  Feld. Gesucht wird als Teilfolge, `grph` findet also „Graph anzeigen".
  (`Cmd/Strg+K` bleibt im Editor die Link-Taste.)
- **Suche**, die in den Notiztexten sucht und nicht nur in Titeln – und den
  Begriff dort hervorhebt, wo er wirklich steht.
- **Suchen & Ersetzen im ganzen Vault** – siehe unten.
- **Graph-Ansicht** – kräftebasiert, verschieben/zoomen/ziehen, eine Farbe pro
  Hauptordner mit Abstufungen für Unterordner, KI-Notizen mit Ring.

**Im Alltag**

- **Schnellnotiz** (`Cmd/Strg+Umschalt+N`) – hält einen Gedanken in der Notiz
  von heute fest, ohne dass du wegmusst.
- **Tagesnotizen und Kalender** – eine Notiz pro Tag, benannt `2026-07-26`. Der
  Kalender in der Seitenleiste füllt die Tage, die es gibt, und legt die an, die
  fehlen.
- **Vorlagen** – jede Notiz im Vorlagen-Ordner erscheint in der Palette als
  „Neue Notiz aus: …". `{{date}}`, `{{time}}`, `{{title}}`, `{{weekday}}`,
  `{{month}}` und `{{year}}` werden eingesetzt.
- **Ordner und Unterordner** – einen Unterordner direkt im Ordner anlegen,
  Ordner per Drag & Drop oder mit einem Knopf verschieben. Notizen lassen sich
  ebenfalls ziehen.

**Verbindungen, unter dem Editor**

- **Backlinks** – was hierher verlinkt.
- **Links raus** – worauf diese Notiz zeigt, auch auf Ziele, die es noch nicht
  gibt.
- **Erwähnungen** – Notizen, die den Namen dieser Notiz schreiben, ohne sie zu
  verlinken. Einzeln oder alle auf einmal verlinkbar. So füllt sich der Graph
  von selbst.
- **Ähnliche Notizen** – TF-IDF über den Vault, Vergleich per Kosinus. Ehrlich
  benannt: das ist *lexikalische* Ähnlichkeit, keine semantische. Es findet
  Notizen mit denselben Wörtern, nicht Notizen mit derselben Bedeutung – dafür
  ohne Modell-Download, ohne Runtime und ohne Netz.

**Sicherheitsnetz**

- **Versionsverlauf** – Magma legt vor jeder größeren Änderung eine Kopie an,
  vor einem vault-weiten Ersetzen immer. Diff-Ansicht, Wiederherstellen mit
  einem Klick, und das Wiederherstellen ist selbst rückgängig zu machen.
  Umbenennen oder Verschieben nimmt den Verlauf mit.
- **Was Claude geschrieben hat** – eine Seite mit allen Notizen, die
  `author: ai` tragen, neueste zuerst. Eine KI in den eigenen Vault zu lassen
  ist nur dann angenehm, wenn man genau sieht, was sie angefasst hat.

**Import**

- Einen ganzen WordPress-Blog als verlinkte Notizen in einen Ordner holen,
  gruppiert nach Kategorie, mit erkanntem Autor, der auf deine bestehende Notiz
  verlinkt wird.

## Suchen & Ersetzen im ganzen Vault

Eine Formulierung in 500 Notizen zu ändern ist ein Dialog. Geschrieben wird
nichts, bevor du die Vorschau gesehen hast: jede betroffene Notiz mit ihrer
Trefferzahl. Wikilinks lösen über den *Dateinamen* auf – würde nur der Text
ersetzt, zeigte jedes `[[…]]` ins Leere. Magma benennt deshalb zuerst die Notiz
um, die den Begriff im Namen trägt, biegt ihre Links mit (samt Alias und Anker)
und schreibt erst danach den Text.

## Mit Claude verbinden

Magma ist sein eigener MCP-Server – es muss nichts extra installiert werden.
**Einstellungen → Claude → Claude Desktop einrichten**. Ein Klick schreibt die
Konfiguration (mit Sicherung einer vorhandenen) und ersetzt jeden älteren
Magma-Eintrag; Claude Desktop neu starten, fertig.

Für andere MCP-Clients: das JSON unter *Manuelle Einrichtung* kopieren:

```json
{
  "mcpServers": {
    "magma": { "command": "/pfad/zu/Magma", "args": ["--mcp", "/pfad/zum/vault"] }
  }
}
```

Claude bekommt Werkzeuge zum Suchen, Lesen, Anlegen und Ändern von Notizen, zum
Durchlaufen des Ordnerbaums, zum Umbenennen, Verschieben und Löschen – und, das
ist der Punkt, um die Verbindungen im Vault zu sehen: `find_link_candidates`,
`related_notes`, `unlinked_mentions`, `link_mentions` und
`list_outgoing_links`. Jeder `[[Wikilink]]`, den Claude schreibt, wird gegen den
echten Vault geprüft; kaputte kommen mit Korrekturvorschlägen zurück, statt als
Sackgasse geschrieben zu werden. Von der KI geschriebene Notizen bekommen
`author: ai`, erscheinen im Graph violett und stehen unter *KI*. Vor jeder
KI-Änderung wird ein Snapshot angelegt. `MAGMA_MCP_ALLOW_WRITE=0` schaltet auf
Nur-Lesen.

## Anpassen

**Einstellungen → Darstellung & Sprache**: Hell/Dunkel/System, Akzent- und
KI-Notiz-Farbe, Oberflächen- und Editor-Schrift, Schriftgröße, Lesebreite.
Änderungen erscheinen sofort in der Vorschau und werden mit **Speichern**
übernommen; Schließen ohne Speichern nimmt sie zurück. Schriften nutzen nur,
was ohnehin auf deinem System liegt – nichts wird heruntergeladen.

Die native Menüleiste folgt der gewählten Sprache. Nur die Einträge, die macOS
selbst einhängt (Writing Tools, AutoFill, Diktat, Emoji & Symbole), bleiben in
der Systemsprache – die gehören zu macOS, nicht zu Magma.

## Remote-Vault (optional)

Richte Magma auf eine WebDAV-URL aus (**Einstellungen → Vault & Sync**), um
einen Vault auf einem Webserver zu halten und von jedem Rechner zu bearbeiten.
Magma synchronisiert ihn in einen lokalen Cache und schickt deine Änderungen
beim Speichern zurück. HTTPS ist Pflicht; das Passwort bleibt nur für die
Sitzung gespeichert (Ablage im Schlüsselbund ist geplant).

---

## Aus dem Quellcode bauen

Voraussetzungen: Node 20+, Rust (stable) und die
[Tauri-Systemabhängigkeiten](https://tauri.app/start/prerequisites/) für dein
Betriebssystem.

```bash
npm install
npm run tauri build      # erzeugt das DMG (macOS) / MSI (Windows)
```

Das Installationsprogramm liegt danach in `src-tauri/target/release/bundle/`.

Zum Entwickeln:

```bash
npm run tauri dev        # App mit Hot Reload starten
npm run build            # Typprüfung + Frontend bauen
cargo test -p magma-core -p magma-mcp -p magma-webdav -p magma-import
```

## Aufbau

```
src/                 React-Oberfläche (Seitenleiste, Editor, Graph, Panels)
src-tauri/           Tauri-Desktop-Shell – dünne Befehlsschicht über magma-core
crates/magma-core/   Reine Rust-Vault-Logik: Notizen, Links, Graph, Suche,
                     Verlauf, Ähnlichkeit und die Regeln für die KI als Mitautor
crates/magma-mcp/    Eingebauter MCP-Server (stdio JSON-RPC) über magma-core
crates/magma-webdav/ Optionaler Remote-Vault über WebDAV
crates/magma-import/ WordPress-Importer
docs/PLAN.md         Produktplan, Recherche und Roadmap
```

Gebaut mit **Tauri 2** (Rust-Kern, ~10 MB Binaries), **React + TypeScript +
Tailwind** und einem **TipTap/ProseMirror**-Editor. Die Vault-Logik steckt in
`magma-core` und ist auf jeder Plattform unit-getestet – Desktop-App und
MCP-Server arbeiten damit immer auf demselben Modell deiner Notizen.

## Stand

Die Meilensteine **M0–M4** und **M7** sind fertig; **M5** (Packaging) liefert
unsignierte Installationsprogramme, Signierung und Auto-Update stehen noch aus;
**M6** (Remote-Vault) hat eine funktionierende erste Version. Die Roadmap steht
in [`docs/PLAN.md`](docs/PLAN.md).

## Lizenz

Siehe [LICENSE](LICENSE).
