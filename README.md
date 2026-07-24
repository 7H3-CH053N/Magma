# Magma

**Your second brain, minus the setup.** A beautiful, local-first note app with
the core idea of Obsidian — plain markdown files, `[[wikilinks]]`, backlinks and
a graph — but simpler to use and LLM-native from day one.

Magma is built for two authors: **you** and **your AI**. It ships with an
embedded MCP server so Claude (and other agents) can read your vault *and* write
new notes that are correctly, logically linked into what you already have — not
orphaned dumps.

> Status: early. This repo currently contains milestone **M0** — a running
> Tauri 2 + React scaffold and CI. See [`docs/PLAN.md`](docs/PLAN.md) for the
> full product plan, the Obsidian research it's based on, and the roadmap.

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

## Layout

```
src/                 React UI (Sidebar, live-markdown Editor, app shell)
src-tauri/           Tauri desktop shell — thin command layer over magma-core
crates/magma-core/   Pure Rust vault logic (list/read/write notes, AI-authored
                     detection, path safety) — testable on any platform
docs/PLAN.md         Product plan, research, and roadmap
```

## License

See [LICENSE](LICENSE).
