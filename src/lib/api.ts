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
  author?: string
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_wordpress", { vault, folder, siteUrl, author });
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

/** One-click: write the Magma server into Claude Desktop's config. */
export async function installMcp(vault: string): Promise<string> {
  return invoke<string>("install_mcp", { vault });
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
