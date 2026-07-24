# Magma

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

Magma ships an MCP server (`magma-mcp`) so Claude can read your vault and
co-author correctly linked notes. Add it to your MCP client config:

```json
{
  "mcpServers": {
    "magma": { "command": "magma-mcp", "args": ["/path/to/your/vault"] }
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

## Remote vault (optional)

Point Magma at a WebDAV URL (Settings → Remote vault) to keep one vault on a
webserver and edit it from any machine. Magma syncs it into a local cache and
pushes your edits back on save. HTTPS is required; the password is kept only for
the session (OS-keychain storage is a planned follow-up).

## License

See [LICENSE](LICENSE).
