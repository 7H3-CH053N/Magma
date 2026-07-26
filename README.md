# Magma

**English** · [Deutsch](README.de.md)

**Your second brain, minus the setup.** A beautiful, local-first note app with
the core idea of Obsidian — plain markdown files, `[[wikilinks]]`, backlinks and
a graph — but simpler to use and LLM-native from day one.

Magma is built for two authors: **you** and **your AI**. It ships with an
embedded MCP server so Claude (and other agents) can read your vault *and* write
new notes that are correctly, logically linked into what you already have — not
orphaned dumps.

> Status: early but usable. Milestones **M0–M3** are in: vault CRUD, a
> live-markdown editor, `[[wikilinks]]` with autocomplete, backlinks, full-text
> search, a graph view, and a built-in MCP server for Claude. Plus a flame icon,
> startup splash, an About/Settings panel, and German/English localization.
> See [`docs/PLAN.md`](docs/PLAN.md) for the research, roadmap, and the remote
> vault design (M6).

## Why Magma

People love Obsidian for owning their notes as local markdown and for the way
ideas connect (links + graph). They bounce off its steep learning curve, raw
default UI, and the plugin-hunting needed for basics. Magma keeps what's loved
and fixes what's not:

- **Keeps you unlocked** — a vault is just a folder of `.md` files. Obsidian
  vaults open as-is (`[[wikilinks]]`, YAML frontmatter, tags).
- **No setup ritual** — one clean, considered default UI. Live markdown where the
  syntax fades as you type. No edit/preview toggle, no theme hunt.
- **The graph is a headline feature** — see your knowledge connect, including
  what your AI added.
- **LLM as co-author** — built-in MCP server. The AI proposes links against your
  real notes and the server validates every `[[link]]` before it's written.

## Tech

- **Tauri 2** (Rust) — small, fast, native macOS + Windows.
- **React + TypeScript + Tailwind** — UI.
- **CodeMirror 6** — the live-markdown editor.
- **`crates/magma-core`** — platform-independent vault logic, shared by the
  desktop shell and (soon) the MCP server. Fully unit-tested.

## Develop

Prerequisites: Node 20+, Rust (stable), and the
[Tauri system dependencies](https://tauri.app/start/prerequisites/) for your OS.

```bash
npm install          # frontend deps
npm run tauri dev    # run the desktop app
```

Other useful commands:

```bash
npm run build              # type-check + build the frontend
cargo test -p magma-core   # run the vault/core unit tests
npm run tauri build        # produce a desktop bundle (DMG / MSI)
```

## Connect to Claude

Magma is its own MCP server — no separate install. In the app, open
**Settings → Connect to Claude → Set up Claude Desktop**. One click writes the
config (backing up any existing one); restart Claude Desktop and you're done.

For other MCP clients, copy the JSON shown under *Manual setup* — it runs the
Magma executable as the server:

```json
{
  "mcpServers": {
    "magma": { "command": "/path/to/Magma", "args": ["--mcp", "/path/to/vault"] }
  }
}
```

The write tools guide the model to call `find_link_candidates` first, then
validate every `[[wikilink]]` it writes — broken links come back with
suggestions instead of dead ends. AI-written notes are stamped `author: ai` and
shown in violet in the graph. Set `MAGMA_MCP_ALLOW_WRITE=0` for read-only.

## Layout

```
src/                 React UI (Sidebar, live-markdown Editor, Graph, Settings)
src-tauri/           Tauri desktop shell — thin command layer over magma-core
crates/magma-core/   Pure Rust vault logic: notes, links, graph, search, and the
                     AI co-authoring rules — testable on any platform
crates/magma-mcp/    Built-in MCP server (stdio JSON-RPC) over magma-core
crates/magma-webdav/ Optional remote vault: sync a WebDAV folder to a local cache
docs/PLAN.md         Product plan, research, and roadmap
```

## Every day, and every connection

- **Command palette** — `Cmd/Ctrl+P` opens everything: jump to a note, run a
  command, start from a template. Matching is subsequence-based, so `grph` finds
  "Show graph". (`Cmd/Ctrl+K` stays the link key inside the editor.)
- **Quick capture** — `Cmd/Ctrl+Shift+N` catches a thought into today's note
  without leaving what you were doing.
- **Daily notes and a calendar** — one note per day, named `2026-07-26`. The
  calendar in the sidebar fills the days that exist and creates the ones that
  don't.
- **Templates** — every note in your template folder shows up in the palette as
  "New note from: …". `{{date}}`, `{{time}}`, `{{title}}`, `{{weekday}}`,
  `{{month}}` and `{{year}}` are filled in.
- **Version history** — Magma keeps a copy before every larger change, and
  always before a vault-wide replace. Diff view, one-click restore, and the
  restore is itself undoable. Renaming or moving a note takes its history along.
- **Connections under the editor** — backlinks, outgoing links (including
  targets that don't exist yet), *unlinked mentions* (notes that write this
  note's name without linking it — link them one by one or all at once), and
  similar notes.
- **Similar notes** — TF-IDF over your vault, compared by cosine. Named
  honestly: this is lexical similarity, not semantic. It finds notes using the
  same words, not notes meaning the same thing — and it needs no model
  download, no runtime and no network.
- **What Claude wrote** — a page listing every note carrying `author: ai`,
  newest first. Letting an LLM into your vault is only comfortable if you can
  see exactly what it touched.

## Find & replace across the vault

Renaming a phrase in 500 notes is one dialog. Nothing is written before you have
seen the preview: every affected note with its hit count. Wikilinks resolve on a
note's *filename*, so replacing only the text would leave every `[[…]]` pointing
at nothing — Magma renames the note that carries the term first, repointing its
links (aliases and anchors included), and only then rewrites the text.

## Make it yours

Everything visual is adjustable in **Settings → Appearance**: light/dark/system
mode, accent and AI-note colors, interface and editor fonts, font size, and
reading width. Changes apply live and are written when you save. The native
menu bar follows the language you pick, too — only the entries macOS injects
itself (Writing Tools, AutoFill, Dictation, Emoji & Symbols) stay in the system
language, because they belong to macOS rather than to Magma. Changes persist. Fonts use only what's already on
your system, so nothing is downloaded.

## Remote vault (optional)

Point Magma at a WebDAV URL (Settings → Remote vault) to keep one vault on a
webserver and edit it from any machine. Magma syncs it into a local cache and
pushes your edits back on save. HTTPS is required; the password is kept only for
the session (OS-keychain storage is a planned follow-up).

## License

See [LICENSE](LICENSE).
