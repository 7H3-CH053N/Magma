// Thin typed wrapper over the Tauri command bridge to the Rust core.
// Kept isolated so the UI can be developed (and tested) against a mock
// when the desktop shell is not available.
import { invoke } from "@tauri-apps/api/core";

export interface NoteMeta {
  /** Path relative to the vault root, e.g. "ideas/second-brain.md". */
  path: string;
  title: string;
  /** true when the note was created or last edited by an LLM via MCP. */
  aiAuthored: boolean;
  /** Last modified, in milliseconds since the epoch; 0 when unknown. */
  modified: number;
}

export interface Note extends NoteMeta {
  content: string;
}

const hasTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function pickVault(): Promise<string | null> {
  return invoke<string | null>("pick_vault");
}

export async function listNotes(vault: string): Promise<NoteMeta[]> {
  if (!hasTauri) return [];
  return invoke<NoteMeta[]>("list_notes", { vault });
}

export async function readNote(vault: string, path: string): Promise<Note> {
  return invoke<Note>("read_note", { vault, path });
}

export async function writeNote(
  vault: string,
  path: string,
  content: string
): Promise<void> {
  return invoke("write_note", { vault, path, content });
}

export async function createNote(vault: string, title: string): Promise<string> {
  return invoke<string>("create_note", { vault, title });
}

export async function renameNote(
  vault: string,
  path: string,
  newTitle: string
): Promise<string> {
  return invoke<string>("rename_note", { vault, path, newTitle });
}

export async function deleteNote(vault: string, path: string): Promise<void> {
  return invoke("delete_note", { vault, path });
}

/** Move a note into a folder ("" = root); returns the new path. */
export async function moveNote(
  vault: string,
  path: string,
  folder: string
): Promise<string> {
  return invoke<string>("move_note", { vault, path, folder });
}

export async function createFolder(vault: string, name: string): Promise<string> {
  return invoke<string>("create_folder", { vault, name });
}

/** Delete a folder and every note inside it (recursive). */
export async function deleteFolder(vault: string, folder: string): Promise<void> {
  return invoke("delete_folder", { vault, folder });
}

export async function listFolders(vault: string): Promise<string[]> {
  if (!hasTauri) return [];
  return invoke<string[]>("list_folders", { vault });
}

/** Import a WordPress blog into a folder; returns the number of notes written. */
export interface ImportSummary {
  notes: number;
  posts: number;
  /** Author names found (empty when the site's REST API hides them). */
  authors: string[];
  /** Authors linked into a note you already had, as "Name → path". */
  merged: string[];
  /** Author notes the import created itself, as "Name → path". */
  created: string[];
}

export async function importWordpress(
  vault: string,
  folder: string,
  siteUrl: string,
  author?: string,
  /** Vault-relative path of the note the author should link to (optional). */
  authorNote?: string
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_wordpress", {
    vault,
    folder,
    siteUrl,
    author,
    authorNote,
  });
}

/** Save pasted image bytes into the vault; returns the vault-relative path. */
export async function saveAsset(
  vault: string,
  fileName: string,
  bytes: Uint8Array
): Promise<string> {
  return invoke<string>("save_asset", {
    vault,
    fileName,
    bytes: Array.from(bytes),
  });
}

export interface GraphNode {
  path: string;
  title: string;
  aiAuthored: boolean;
  degree: number;
  /** A link target with no note behind it yet (shown as a ghost node). */
  missing?: boolean;
}

export interface GraphEdge {
  source: string;
  target: string;
}

export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface SearchHit {
  path: string;
  title: string;
  snippet: string;
}

export async function buildGraph(vault: string): Promise<Graph> {
  if (!hasTauri) return { nodes: [], edges: [] };
  return invoke<Graph>("build_graph", { vault });
}

export async function backlinks(vault: string, path: string): Promise<NoteMeta[]> {
  if (!hasTauri) return [];
  return invoke<NoteMeta[]>("backlinks", { vault, path });
}

export async function search(vault: string, query: string): Promise<SearchHit[]> {
  if (!hasTauri) return [];
  return invoke<SearchHit[]>("search", { vault, query });
}

export interface ReplaceHit {
  path: string;
  title: string;
  count: number;
}
export interface ReplaceRename {
  path: string;
  from: string;
  to: string;
}
export interface ReplaceReport {
  hits: ReplaceHit[];
  /** Notes renamed along with the text, so wikilinks keep resolving. */
  renames: ReplaceRename[];
  total: number;
  applied: boolean;
}

/**
 * Vault-wide find & replace. With `dryRun`, nothing is written.
 * `renameNotes` also renames notes whose own name holds the term — otherwise a
 * rewritten `[[wikilink]]` would point at a note that no longer exists.
 */
export async function replaceAll(
  vault: string,
  find: string,
  replace: string,
  dryRun: boolean,
  renameNotes: boolean
): Promise<ReplaceReport> {
  if (!hasTauri) return { hits: [], renames: [], total: 0, applied: false };
  return invoke<ReplaceReport>("replace_all", {
    vault,
    find,
    replace,
    dryRun,
    renameNotes,
  });
}

// --- Connections: outgoing links, unlinked mentions, related notes ---------

export interface OutgoingLink {
  name: string;
  /** Empty when no note of that name exists yet. */
  path: string;
  title: string;
  missing: boolean;
}

export interface Mention {
  path: string;
  title: string;
  snippet: string;
  count: number;
}

export interface RelatedNote {
  path: string;
  title: string;
  /** 0..1 share of vocabulary with the note asked about. */
  score: number;
  linked: boolean;
}

export async function outgoingLinks(
  vault: string,
  path: string
): Promise<OutgoingLink[]> {
  if (!hasTauri) return [];
  return invoke<OutgoingLink[]>("outgoing_links", { vault, path });
}

/** Notes naming this one in plain text without linking it. */
export async function unlinkedMentions(
  vault: string,
  path: string
): Promise<Mention[]> {
  if (!hasTauri) return [];
  return invoke<Mention[]>("unlinked_mentions", { vault, path });
}

/** Turn plain mentions of `name` inside `path` into wikilinks; returns how many. */
export async function linkMentions(
  vault: string,
  path: string,
  name: string
): Promise<number> {
  return invoke<number>("link_mentions", { vault, path, name });
}

export async function relatedNotes(
  vault: string,
  path: string,
  limit = 8
): Promise<RelatedNote[]> {
  if (!hasTauri) return [];
  return invoke<RelatedNote[]>("related_notes", { vault, path, limit });
}

// --- Version history -------------------------------------------------------

export interface Version {
  id: string;
  /** Milliseconds since the epoch. */
  takenAt: number;
  bytes: number;
}

export async function listVersions(vault: string, path: string): Promise<Version[]> {
  if (!hasTauri) return [];
  return invoke<Version[]>("list_versions", { vault, path });
}

export async function readVersion(
  vault: string,
  path: string,
  id: string
): Promise<string> {
  return invoke<string>("read_version", { vault, path, id });
}

export async function restoreVersion(
  vault: string,
  path: string,
  id: string
): Promise<void> {
  return invoke("restore_version", { vault, path, id });
}

// --- Daily notes, templates, quick capture ---------------------------------

/** Open the note named `title` in `folder`, creating it from `content` if new. */
export async function openOrCreate(
  vault: string,
  folder: string,
  title: string,
  content: string
): Promise<{ path: string; created: boolean }> {
  const [path, created] = await invoke<[string, boolean]>("open_or_create", {
    vault,
    folder,
    title,
    content,
  });
  return { path, created };
}

/** Append text to the end of a note without opening it. */
export async function appendNote(
  vault: string,
  path: string,
  text: string
): Promise<void> {
  return invoke("append_note", { vault, path, text });
}

export interface RemoteConfig {
  url: string;
  username: string;
  password: string;
}

/** Connect a remote WebDAV vault; returns the local cache dir to open. */
export async function remoteConnect(cfg: RemoteConfig): Promise<string> {
  return invoke<string>("remote_connect", { ...cfg });
}

/** Push a note to the remote vault (write-through after a local save). */
export async function remotePut(
  cfg: RemoteConfig,
  path: string,
  content: string
): Promise<void> {
  return invoke("remote_put", { ...cfg, path, content });
}

export async function remoteDelete(cfg: RemoteConfig, path: string): Promise<void> {
  return invoke("remote_delete", { ...cfg, path });
}

/** The exact MCP config JSON for this machine (real executable path + vault). */
export async function mcpConfig(vault: string): Promise<string> {
  if (!hasTauri) return "";
  return invoke<string>("mcp_config", { vault });
}

export interface McpInstall {
  configPath: string;
  executable: string;
  /** True when the registered binary sits in a cargo build directory. */
  devBuild: boolean;
}

/** One-click: write the Magma server into Claude Desktop's config. */
export async function installMcp(vault: string): Promise<McpInstall> {
  return invoke<McpInstall>("install_mcp", { vault });
}

/** The vault that was open last time, if that folder still exists. */
export async function lastVault(): Promise<string | null> {
  if (!hasTauri) return null;
  return invoke<string | null>("last_vault");
}

/** Remember the vault so the next start opens it straight away. */
export async function setLastVault(vault: string): Promise<void> {
  if (!hasTauri) return;
  return invoke("set_last_vault", { vault });
}

/** Open an http(s) link in the system browser (never inside the app WebView). */
export async function openExternal(url: string): Promise<void> {
  if (!hasTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  return invoke("open_external", { url });
}

export { hasTauri };
