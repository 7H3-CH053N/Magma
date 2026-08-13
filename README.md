<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/magma-dark.svg" />
    <img src="assets/magma-light.svg" alt="" width="88" height="84" />
  </picture>
</p>

# Magma

**English** · [Deutsch](README.de.md)

**Your second brain, minus the setup.** A local-first note app with the core
idea of Obsidian — plain markdown files, `[[wikilinks]]`, backlinks and a graph
— but simpler to use and LLM-native from day one.

Magma is built for two authors: **you** and **your AI**. It ships with an
embedded MCP server, so Claude can read your vault *and* write new notes that
are properly linked into what you already have — not orphaned dumps.

---

## Install

Magma runs on **macOS** and **Windows**. Download the installer for your system
from the [latest release](https://github.com/7H3-CH053N/Magma/releases/latest):

| System | File | What to do |
| --- | --- | --- |
| macOS (Apple Silicon & Intel) | `Magma_x.y.z_universal.dmg` | Open the DMG, drag Magma into *Applications* |
| Windows 10/11 (64-bit) | `Magma_x.y.z_x64_en-US.msi` | Run the installer |

> The very first release (v0.1.0) shipped an Apple-Silicon-only DMG; every
> release since is a universal binary that runs on both.

**The builds are not code-signed yet**, so both systems will warn you the first
time. This is expected for an app that has no certificate — not a sign that
anything is wrong, but you should only trust that from a source you trust:

- **macOS** — the first launch says Magma "cannot be opened because it is from
  an unidentified developer". Right-click the app in *Applications* → **Open** →
  **Open**. Once done, it starts normally forever after. If macOS instead calls
  the app "damaged", the quarantine flag needs clearing:
  `xattr -dr com.apple.quarantine /Applications/Magma.app`
- **Windows** — SmartScreen shows "Windows protected your PC". Click **More
  info** → **Run anyway**.

Prefer to build it yourself? See [Build from source](#build-from-source) — it
takes two commands and produces exactly these installers.

### First run

1. **Pick a folder** for your notes. Any folder works, including an existing
   Obsidian vault — Magma reads the same `[[wikilinks]]`, YAML frontmatter and
   folder structure, and writes nothing proprietary. The choice is remembered.
2. Press **`Cmd/Ctrl+P`** for everything: jump to a note, run a command, start
   from a template.
3. Optional: **Settings → Claude → Set up Claude Desktop** to let Claude in.

Your notes stay `.md` files on your disk. Magma keeps its own bookkeeping (the
version history) in a hidden `.magma` folder inside the vault, and nothing else.

---

## Why Magma

People love Obsidian for owning their notes as local markdown and for the way
ideas connect. They bounce off its learning curve, its raw default UI, and the
plugin-hunting needed for basics. Magma keeps what's loved and fixes what's not:

- **Keeps you unlocked** — a vault is just a folder of `.md` files. Obsidian
  vaults open as-is.
- **No setup ritual** — one considered default UI. Live markdown where the
  syntax fades as you type. No edit/preview toggle, no theme hunt.
- **The graph is a headline feature** — see your knowledge connect, including
  what your AI added.
- **LLM as co-author** — a built-in MCP server that validates every `[[link]]`
  before it is written, so an AI cannot quietly create dead ends.

## What's in it

**Writing**

- Live-markdown editor: formatting renders as you type, the syntax hides itself.
- `[[Wikilinks]]` with autocomplete, plus a discreet pencil to edit a link
  without following it. A link to a note that doesn't exist yet is shown dashed
  and creates that note when clicked, Wikipedia-style.
- Paste images straight in; they are filed inside the vault.
- Tables, task lists, quotes, code — no plugin required.

**Finding your way around**

- **Command palette** (`Cmd/Ctrl+P`) — notes, commands and templates in one
  field. Matching is subsequence-based, so `grph` finds "Show graph".
  (`Cmd/Ctrl+K` stays the link key inside the editor.)
- Note names match regardless of how their umlauts are encoded. `ä` has two
  valid encodings, and macOS stored filenames in the other one for years — so a
  vault carried over from an older Mac holds names that look identical to what
  you type and compare unequal to it. Links resolve either way.
- **Search** that reads note bodies, not just titles, and highlights the term
  where it actually occurs. Write `/pattern/i` or `re:pattern` and it is treated
  as a regular expression; anything else stays a plain, case-insensitive search.
- **Find & replace across the whole vault** — see below.
- **Graph view** — force-directed, pan/zoom/drag, one colour per top-level
  folder with shades for subfolders, AI-written notes ringed. Notes are sized in
  five steps by how many others link to them, so the hub your vault revolves
  around is the one you see first.
- **Term view** — the same canvas switched to what the notes are *about*:
  terms as nodes, an edge where two keep turning up near each other, coloured
  by topic. It shows the shape of a vault whose links were never drawn, which
  is most vaults. Named as honestly as the similarity above it: this counts
  words that co-occur, it does not read meaning. German is folded first, so
  "Notiz", "Notizen" and "Notizen" are one node instead of three — and the
  label is the spelling you actually write most. Words that turn up in more
  than a third of your notes are dropped as grammar rather than subject matter:
  no stopword list is ever finished, but "appears nearly everywhere" is a
  signature grammar has and a subject does not, in any language. Click a term to
  see the notes it appears in. It runs here: no account, no upload, no model
  download.
- **Dataview queries** — a ` ```dataview ` block is answered in place: `TABLE`
  and `LIST` over `FROM` a tag or folder, with `WHERE`, `SORT` and `LIMIT`,
  reading YAML frontmatter and inline `key:: value` fields. DQL only —
  DataviewJS would run arbitrary JavaScript out of your notes.

**Every day**

- **Quick capture** (`Cmd/Ctrl+Shift+N`) — catch a thought into today's note
  without leaving what you were doing.
- **Daily notes and a calendar** — one note per day, named `2026-07-26`. The
  calendar in the sidebar fills the days that exist and creates the ones that
  don't.
- **Templates** — every note in your template folder appears in the palette as
  "New note from: …". `{{date}}`, `{{time}}`, `{{title}}`, `{{weekday}}`,
  `{{month}}` and `{{year}}` are filled in.
- **Folders and subfolders** — create a subfolder from the folder itself, move
  folders by dragging or with one button. Notes move by dragging too.

**Connections, under the editor**

- **Backlinks** — what links here.
- **Links out** — what this note points at, including targets that don't exist
  yet.
- **Unlinked mentions** — notes that write this note's name without linking it.
  Link them one at a time or all at once. This is how a graph fills itself in.
- **Similar notes** — TF-IDF over your vault, compared by cosine. Named
  honestly: this is *lexical* similarity, not semantic. It finds notes using the
  same words, not notes meaning the same thing — and it needs no model download,
  no runtime and no network.

**Safety net**

- **Version history** — Magma keeps a copy before every larger change, and
  always before a vault-wide replace. Diff view, one-click restore, and the
  restore is itself undoable. Renaming or moving a note takes its history along.
- **What Claude wrote** — a page listing every note carrying `author: ai`,
  newest first. Letting an LLM into your vault is only comfortable if you can
  see exactly what it touched.

**Import**

- Pull a whole WordPress blog into a folder as linked notes, grouped by
  category, with the author detected and linked to the note you already have.

## Find & replace across the vault

Renaming a phrase in 500 notes is one dialog. Nothing is written before you have
seen the preview: every affected note with its hit count. Wikilinks resolve on a
note's *filename*, so replacing only the text would leave every `[[…]]` pointing
at nothing — Magma renames the note that carries the term first, repointing its
links (aliases and anchors included), and only then rewrites the text.

## Connect to Claude or Codex

Magma is its own MCP server — nothing extra to install. Open
**Settings → Claude → Set up Claude Desktop**. One click writes the config
(backing up any existing one) and replaces any older Magma entry; restart Claude
Desktop and you're done.

**Codex** is one click further down the same page. It writes Codex's own
`config.toml` rather than calling the `codex` command, because an app started
from the Dock does not inherit your shell's `PATH` and the call would fail on a
perfectly good install. Your existing Codex config is parsed as TOML, so only
Magma's own entry is replaced and everything else keeps its formatting.

For other MCP clients, copy the JSON shown under *Manual setup*:

```json
{
  "mcpServers": {
    "magma": { "command": "/path/to/Magma", "args": ["--mcp", "/path/to/vault"] }
  }
}
```

Claude gets tools to search, read, create and update notes, walk the folder
tree, rename, move and delete, and — the point of it — to see the vault's
connections: `find_link_candidates`, `related_notes`, `unlinked_mentions`,
`link_mentions` and `list_outgoing_links`. Every `[[wikilink]]` it writes is
validated against the real vault; broken ones come back with suggestions instead
of being written as dead ends. AI-written notes are stamped `author: ai`, shown
in violet in the graph and listed under *AI* — with a badge naming which client
wrote them, so Claude's work and Codex's work stay apart. A version snapshot is
taken before every AI edit — and before an AI deletes a note or a folder, since
that is the one action with nothing left to compare against. The history lives
outside whatever gets removed, so a deleted note can still be read back. Set
`MAGMA_MCP_ALLOW_WRITE=0` for read-only.

## Make it yours

**Settings → Appearance & language**: light/dark/system, accent, AI-note and
text-highlight colours, interface and editor fonts, font size, reading width.
Changes preview live and are written when you press **Save**; closing without
saving takes them back. Fonts use only what is already on your system, so
nothing is downloaded.

The native menu bar follows the language you pick. Only the entries macOS
injects itself (Writing Tools, AutoFill, Dictation, Emoji & Symbols) stay in the
system language — those belong to macOS, not to Magma.

## Remote vault (optional)

Point Magma at a WebDAV URL (**Settings → Vault & sync**) to keep one vault on a
webserver and edit it from any machine. Magma syncs it into a local cache and
pushes your edits back on save. HTTPS is required; the password is kept only for
the session (OS-keychain storage is a planned follow-up).

The vault is listed one folder at a time. WebDAV can return a whole tree in a
single request, but hardly any server allows it: Apache's `mod_dav` answers 403
unless `DavDepthInfinity` is switched on, and sabre/dav — which Nextcloud and
ownCloud are built on — serves the request as if only one level had been asked
for. The second is the reason for walking rather than asking once: it looks like
a normal answer, so a vault would sync its top-level notes and silently leave
every subfolder behind.

---

## Build from source

Prerequisites: Node 20+, Rust (stable), and the
[Tauri system dependencies](https://tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri build      # produces the DMG (macOS) / MSI (Windows)
```

The installer lands in `src-tauri/target/release/bundle/`.

### Publishing updates

The installed app checks
`https://github.com/7H3-CH053N/Magma/releases/latest/download/latest.json` and
only installs what the updater key signed. That key is **not** the code-signing
certificate — Tauri's updater uses its own minisign pair, which costs nothing
and you generate yourself.

**Before the first signed release**, the repository owner has to create that
pair and replace the placeholder:

```bash
npm run tauri signer generate -- -w ~/.tauri/magma-updater.key
```

Put the public half into `pubkey` in `src-tauri/tauri.conf.json` and the private
half into the `TAURI_SIGNING_PRIVATE_KEY` secret in GitHub Actions; if the key
has no password, set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to an empty secret.
The `pubkey` in the file right now came in with the contributed patch and is a
placeholder: whoever holds its private half could sign an update that every
installed Magma would accept. Replace it before any release goes out — once
updates are public the key can no longer be rotated without cutting off apps
that are already installed.

Publishing an update is then: bump the app version and create a `vX.Y.Z` release
through the GitHub workflow.

For development:

```bash
npm run tauri dev        # run the app with hot reload
npm run build            # type-check + build the frontend
cargo test -p magma-core -p magma-mcp -p magma-webdav -p magma-import
```

## Layout

```
src/                 React UI (sidebar, editor, graph, panels, dialogs)
src-tauri/           Tauri desktop shell — thin command layer over magma-core
crates/magma-core/   Pure Rust vault logic: notes, links, graph, search,
                     history, similarity, and the AI co-authoring rules
crates/magma-mcp/    Built-in MCP server (stdio JSON-RPC) over magma-core
crates/magma-webdav/ Optional remote vault: sync a WebDAV folder to a local cache
crates/magma-import/ WordPress importer
docs/PLAN.md         Product plan, research, and roadmap
```

Built with **Tauri 2** (Rust core, ~10 MB binaries), **React + TypeScript +
Tailwind**, and a **TipTap/ProseMirror** editor. The vault logic lives in
`magma-core` and is unit-tested on any platform, so the desktop app and the MCP
server always act on the same model of your notes.

## Status

Milestones **M0–M4** and **M7** are done; **M5** (packaging) ships unsigned
installers, with code signing still open. Auto-update is built in but not live:
it starts working once the repository owner generates the updater key described
above and cuts a release with it. **M6** (remote vault) has a working first
version. The roadmap lives in [`docs/PLAN.md`](docs/PLAN.md).

## Thanks

[**Alexander Mut**](https://github.com/alexandermut) turned up shortly after the
repository went public and has contributed a good part of what is in it:

- **Dataview-style DQL queries**, rendered in place in the editor
- **Codex support**, alongside Claude, plus the `ai_client` mark that says which
  model wrote a note
- **Regular expressions in search**
- **A configurable text-highlight colour**
- **Hardened vault paths** — `safe_join` rejected `..` but let an absolute path
  through, which meant an LLM could name a file outside the vault and be given
  it. Found and fixed before anyone hit it.
- The **`desktop-tests` CI job**, which runs a crate no job had covered

## License

See [LICENSE](LICENSE).
