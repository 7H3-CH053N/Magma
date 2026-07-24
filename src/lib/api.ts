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

export { hasTauri };
