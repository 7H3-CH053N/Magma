//! Magma MCP server (stdio, newline-delimited JSON-RPC 2.0).
//!
//! Gives an LLM agent read access to a Magma vault *and* the ability to add
//! notes that are correctly linked into the existing graph. The write tools
//! deliberately funnel the model through `find_link_candidates` (surface related
//! notes) and then validate every `[[wikilink]]` it writes, reporting broken
//! links with suggestions instead of silently creating dead ends.
//!
//! Config via environment:
//!   MAGMA_VAULT            path to the vault (or pass as the first CLI arg)
//!   MAGMA_MCP_ALLOW_WRITE  "0"/"false" to run read-only (default: writable)

use magma_core as core;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

const SERVER_NAME: &str = "magma";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

struct Server {
    vault: PathBuf,
    allow_write: bool,
    client: Option<String>,
    /// The embedding model, when one is loaded. `None` keeps retrieval lexical,
    /// which is the ordinary state of a fresh install.
    model: Option<Box<dyn core::Similarity>>,
}

/// Serve the MCP protocol over stdio until stdin closes. Shared by the
/// `magma-mcp` binary and the Magma desktop app (`magma --mcp <vault>`), so a
/// user never has to install a separate server to connect Claude.
pub fn serve_stdio(vault: PathBuf, allow_write: bool) {
    serve_stdio_with(vault, allow_write, None)
}

/// As [`serve_stdio`], with an embedding model for `retrieve` to rank meaning
/// alongside words. The desktop app passes one when the user has downloaded it;
/// the standalone binary never does, so it stays free of that dependency.
pub fn serve_stdio_with(
    vault: PathBuf,
    allow_write: bool,
    model: Option<Box<dyn core::Similarity>>,
) {
    let client = std::env::var("MAGMA_MCP_CLIENT")
        .ok()
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty());
    let server = Server {
        vault,
        allow_write,
        client,
        model,
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_msg(
                    &mut out,
                    error_response(Value::Null, -32700, &format!("parse error: {e}")),
                );
                continue;
            }
        };
        if let Some(resp) = server.handle(&req) {
            write_msg(&mut out, resp);
        }
    }
}

fn write_msg(out: &mut impl Write, msg: Value) {
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

impl Server {
    /// Handle one JSON-RPC message. Returns None for notifications (no id).
    fn handle(&self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Notifications have no id and expect no response.
        if id.is_none() {
            return None;
        }
        let id = id.unwrap();

        match method {
            "initialize" => Some(result_response(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
                }),
            )),
            "ping" => Some(result_response(id, json!({}))),
            "tools/list" => Some(result_response(id, json!({ "tools": tools_spec() }))),
            "tools/call" => Some(self.handle_call(id, &params)),
            other => Some(error_response(
                id,
                -32601,
                &format!("method not found: {other}"),
            )),
        }
    }

    fn handle_call(&self, id: Value, params: &Value) -> Value {
        let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match self.call_tool(name, &args) {
            Ok(value) => result_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": to_text(&value) }]
                }),
            ),
            Err(msg) => result_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": msg }],
                    "isError": true
                }),
            ),
        }
    }

    /// Execute a tool, returning a JSON value to hand back as text content.
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        let v = &self.vault;
        match name {
            "search_notes" => {
                let q = str_arg(args, "query")?;
                let hits = core::search(v, &q).map_err(io)?;
                Ok(json!(hits))
            }
            // Retrieval, as opposed to search: passages ranked against each
            // other across the vault, so an answer is built from the right few
            // hundred words rather than from whole notes that merely match.
            "retrieve" => {
                let q = str_arg(args, "query")?;
                let limit = args
                    .get("limit")
                    .and_then(|l| l.as_u64())
                    .unwrap_or(8)
                    .clamp(1, 50) as usize;
                // Off unless asked for. It is a diagnostic, not part of an
                // answer: it lists notes the result deliberately left out, and
                // a model reading it as findings would cite them.
                let about = args
                    .get("explain_note")
                    .and_then(|n| n.as_str())
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(str::to_string);
                // Naming a note is itself the request; asking for the detail
                // and getting no report back would read as "not found".
                let wants = about.is_some()
                    || args
                        .get("explain")
                        .and_then(|e| e.as_bool())
                        .unwrap_or(false);
                let mut report = core::Explanation {
                    about,
                    ..Default::default()
                };
                let found = core::retrieve_explained(
                    v,
                    &q,
                    limit,
                    self.model.as_deref(),
                    wants.then_some(&mut report),
                )
                .map_err(io)?;
                let mut out = json!(found);
                if wants {
                    out["explain"] = json!(report);
                }
                Ok(out)
            }
            "read_note" => {
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let note = core::read_note(v, &path).map_err(io)?;
                Ok(json!(note))
            }
            // Structure tools: the vault is a tree of folders and subfolders,
            // so an agent has to be able to see it before deciding where a note
            // belongs.
            "list_folders" => {
                let folders = core::list_folders(v).map_err(io)?;
                Ok(json!({ "folders": folders }))
            }
            "list_notes" => {
                let notes = core::list_notes(v).map_err(io)?;
                let folder = args
                    .get("folder")
                    .and_then(|f| f.as_str())
                    .map(|f| f.trim().trim_matches('/').to_string())
                    .filter(|f| !f.is_empty());
                let filtered: Vec<_> = match &folder {
                    Some(dir) => {
                        let prefix = format!("{}/", dir.to_lowercase());
                        let recursive = args
                            .get("recursive")
                            .and_then(|r| r.as_bool())
                            .unwrap_or(true);
                        notes
                            .into_iter()
                            .filter(|n| {
                                let p = n.path.to_lowercase();
                                if !p.starts_with(&prefix) {
                                    return false;
                                }
                                // Non-recursive: only notes directly in `dir`.
                                recursive || !p[prefix.len()..].contains('/')
                            })
                            .collect()
                    }
                    None => notes,
                };
                Ok(json!({ "notes": filtered }))
            }
            "create_folder" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                let created = core::create_folder(v, &path).map_err(io)?;
                Ok(json!({ "folder": created }))
            }
            "move_note" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let folder = args.get("folder").and_then(|f| f.as_str()).unwrap_or("");
                let moved = core::move_note(v, &path, folder).map_err(io)?;
                Ok(json!({ "path": moved }))
            }
            "rename_note" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let title = str_arg(args, "new_title")?;
                let (new_path, updated) =
                    core::rename_note_updating_links(v, &path, &title).map_err(io)?;
                Ok(json!({ "path": new_path, "linksUpdated": updated }))
            }
            "delete_note" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                // Report what will break, so the agent can repair or reconsider.
                let orphaned: Vec<String> = core::backlinks(v, &path)
                    .map_err(io)?
                    .into_iter()
                    .map(|n| n.path)
                    .collect();
                // The tool description asks the model to confirm first. That is a
                // request, not a guarantee — so keep the text before the file goes.
                // The history lives under `.magma/`, outside whatever is deleted,
                // so it survives and the note can be written back by hand.
                core::snapshot(v, &path).map_err(io)?;
                core::delete_note(v, &path).map_err(io)?;
                Ok(json!({ "deleted": path, "nowBrokenLinksIn": orphaned }))
            }
            "delete_folder" => {
                self.ensure_write()?;
                let folder = str_arg(args, "folder")?;
                core::safe_join(v, &folder).ok_or("invalid path")?;
                let prefix = format!("{}/", folder.trim().trim_matches('/').to_lowercase());
                let inside: Vec<String> = core::list_notes(v)
                    .map_err(io)?
                    .into_iter()
                    .filter(|n| n.path.to_lowercase().starts_with(&prefix))
                    .map(|n| n.path)
                    .collect();
                let count = inside.len();
                // Recursive and irreversible: snapshot every note first, one at a
                // time, so a folder deleted on a misunderstanding can be rebuilt.
                for note in &inside {
                    core::snapshot(v, note).map_err(io)?;
                }
                core::delete_folder(v, &folder).map_err(io)?;
                Ok(json!({ "deleted": folder, "notesDeleted": count }))
            }
            "list_backlinks" => {
                let path = str_arg(args, "path")?;
                let back = core::backlinks(v, &path).map_err(io)?;
                Ok(json!(back))
            }
            "find_link_candidates" => {
                let text = str_arg(args, "text")?;
                let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(8) as usize;
                let cands = core::find_link_candidates(v, &text, limit).map_err(io)?;
                Ok(json!(cands))
            }
            // Connections the vault already implies but nobody drew yet. This
            // is what turns an agent from a writer into an editor: it can see
            // where a note belongs and where its name is already being used.
            "list_outgoing_links" => {
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                Ok(json!(core::outgoing_links(v, &path).map_err(io)?))
            }
            "related_notes" => {
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(8) as usize;
                Ok(json!(core::related_notes(v, &path, limit).map_err(io)?))
            }
            "unlinked_mentions" => {
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                Ok(json!(core::unlinked_mentions(v, &path).map_err(io)?))
            }
            "link_mentions" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let name = str_arg(args, "name")?;
                // Keep a copy first: this rewrites a note the user owns.
                let _ = core::snapshot(v, &path);
                let linked = core::link_mentions(v, &path, &name).map_err(io)?;
                Ok(json!({ "linked": linked }))
            }
            "create_note" => {
                self.ensure_write()?;
                let title = str_arg(args, "title")?;
                let content = str_arg(args, "content")?;
                let folder = args.get("folder").and_then(|f| f.as_str());
                let res = core::ai_create_note_for_client(
                    v,
                    folder,
                    &title,
                    &content,
                    self.client.as_deref(),
                )
                .map_err(io)?;
                Ok(json!(res))
            }
            "update_note" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let content = str_arg(args, "content")?;
                let res =
                    core::ai_update_note_for_client(v, &path, &content, self.client.as_deref())
                        .map_err(io)?;
                Ok(json!(res))
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn ensure_write(&self) -> Result<(), String> {
        if self.allow_write {
            Ok(())
        } else {
            Err("this Magma vault is connected read-only (MAGMA_MCP_ALLOW_WRITE=0)".into())
        }
    }
}

/// The tool catalog. Descriptions steer the agent toward linking correctly:
/// look up candidates first, then write and fix any broken links reported back.
fn tools_spec() -> Value {
    json!([
        {
            "name": "search_notes",
            "description": "Full-text search across the vault. Returns matching notes with a snippet. Use to find context before answering or writing.",
            "inputSchema": {
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }
        },
        {
            "name": "retrieve",
            "description": "Retrieve the passages of the vault that best answer a question, ranked across every note. Prefer this over search_notes when you are about to answer from the vault: search_notes returns whole notes that contain a word, this returns the specific paragraphs that bear on the question, each with the note, heading and line it came from. Cite those. `offline` counts notes whose contents were not on disk and could not be read, so a low number of results can be explained rather than guessed at.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The question, in the user's own words. Full sentences work better than keywords." },
                    "limit": { "type": "integer", "description": "How many passages to return. Default 8, maximum 50." },
                    "explain_note": { "type": "string", "description": "Diagnostic. Part of a note's path, case-insensitive, e.g. 'magma 0.1.4'. Adds 'note' to 'explain': every passage of that note with the rank each half of the ranking actually gave it, out of how many, uncut — the one thing the lists above cannot say, because they stop where fusion stops and a note just outside that window looks the same as one nowhere near. Each passage also carries 'variants': the same passage measured against the same question with each candidate shape of context in front of it — nothing, the heading, the note name and heading, and the full path and heading as used now — with the rank each would give it. Compare the rows to see whether that context is helping the passage or drowning it. Implies explain." },
                    "explain": { "type": "boolean", "description": "Diagnostic. Adds 'explain' with the notes each half of the ranking found on its own — 'lexical' by word overlap, 'meaning' by embeddings — before the two were fused, best first, as far down as fusion looks, plus 'embedded' of 'passages': how many passages had a vector at all. Read 'embedded' first: well below 'passages' means the vault is only part-indexed, and the 'meaning' list says nothing until that is fixed. Otherwise it tells 'the model never found this note' apart from 'the model found it and fusion dropped it'. These are not results and must not be cited." }
                },
                "required": ["query"]
            }
        },
        {
            "name": "read_note",
            "description": "Read a note's full markdown by its vault-relative path (e.g. 'ideas/second-brain.md').",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "list_folders",
            "description": "List every folder in the vault as vault-relative paths, including nested ones (e.g. 'Blog', 'Blog/KI-Wissen'). Call this before creating a note so you can file it in the folder it belongs to instead of the vault root.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_notes",
            "description": "List notes in the vault. Pass 'folder' (a vault-relative path such as 'Blog/KI-Wissen') to list only that folder; by default subfolders are included, set 'recursive' to false for that folder alone. Use it to see how a folder is organised before adding to it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": { "type": "string" },
                    "recursive": { "type": "boolean" }
                }
            }
        },
        {
            "name": "create_folder",
            "description": "Create a folder, nested paths included (e.g. 'Projekte/2026'). Only needed for an empty folder — create_note makes any missing folders itself.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "move_note",
            "description": "Move a note into a folder, keeping its filename so every [[wikilink]] to it still resolves. Pass an empty 'folder' to move it to the vault root.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "folder": { "type": "string" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "rename_note",
            "description": "Rename a note. Every [[wikilink]] pointing at it is repointed automatically (aliases and #anchors kept), so nothing breaks. Returns the new path and how many notes were updated.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "new_title": { "type": "string" }
                },
                "required": ["path", "new_title"]
            }
        },
        {
            "name": "delete_note",
            "description": "Delete a note permanently. Irreversible — confirm with the user first. The response lists the notes whose links now point nowhere, so you can fix them.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "delete_folder",
            "description": "Delete a folder and every note inside it, subfolders included. Irreversible and usually large — confirm with the user first, and prefer delete_note when only some notes should go.",
            "inputSchema": {
                "type": "object",
                "properties": { "folder": { "type": "string" } },
                "required": ["folder"]
            }
        },
        {
            "name": "list_backlinks",
            "description": "List notes that link to the given note (by its vault-relative path).",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "find_link_candidates",
            "description": "ALWAYS call this before creating or updating a note. Given the text you intend to write, it returns the most related existing notes so you can link the new note into the graph with [[Title]] wikilinks. Link the relevant ones.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The note text (or a summary) you are about to write." },
                    "limit": { "type": "integer", "description": "Max candidates (default 8)." }
                },
                "required": ["text"]
            }
        },
        {
            "name": "list_outgoing_links",
            "description": "List the [[wikilinks]] a note points at, marking the ones whose target note does not exist yet.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "related_notes",
            "description": "Notes that share vocabulary with an existing note but may not be linked to it. Use this to find where a note belongs in the graph, or to suggest links the user has not drawn yet. Each result says whether a link already exists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Vault-relative path of the note to compare against." },
                    "limit": { "type": "integer", "description": "Max results (default 8)." }
                },
                "required": ["path"]
            }
        },
        {
            "name": "unlinked_mentions",
            "description": "Notes that write this note's name in plain text without linking it. Pair with link_mentions to close those gaps.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "link_mentions",
            "description": "Turn plain-text mentions of `name` inside one note into [[name]] links. Only whole-word matches outside existing links are touched. Returns how many were linked.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "The note to edit (from unlinked_mentions)." },
                    "name": { "type": "string", "description": "The link name to insert, i.e. the target note's filename without .md." }
                },
                "required": ["path", "name"]
            }
        },
        {
            "name": "create_note",
            "description": "Create a new note authored by the AI (frontmatter author: ai is added automatically). Reference related notes with [[Name]] wikilinks — call find_link_candidates first and link by each candidate's `name`. The `folder` may be nested (e.g. 'Projekte/2026') and is created if missing — call list_folders first and reuse an existing one where it fits. When creating several related notes in one task, pass the SAME `folder` for all of them so they are grouped together instead of cluttering the vault root. The response reports resolved and broken links; fix any broken ones with a follow-up update_note.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "content": { "type": "string", "description": "Markdown body. Use [[Name]] to link existing notes (a note's link name is its filename, returned as `name` by find_link_candidates)." },
                    "folder": { "type": "string", "description": "Optional vault-relative folder to file the note under (e.g. \"Profil Alex Januschewsky\"). Use the same folder for a related batch." }
                },
                "required": ["title", "content"]
            }
        },
        {
            "name": "update_note",
            "description": "Overwrite an existing note (by vault-relative path) with new markdown, re-stamped as author: ai. The response reports resolved and broken [[wikilinks]].",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
    ])
}

// --- JSON-RPC helpers ------------------------------------------------------

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn str_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing string argument: {key}"))
}

fn to_text(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> PathBuf {
        let p = std::env::temp_dir().join(format!("magma-mcp-{:?}", std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        core::write_note(&p, "Blog/KI-Wissen/Post A.md", "# A").unwrap();
        core::write_note(&p, "Blog/KI-Wissen/Tief/Post B.md", "# B").unwrap();
        core::write_note(&p, "Blog/Uebersicht.md", "# U").unwrap();
        core::write_note(&p, "Root.md", "# R").unwrap();
        p
    }

    fn srv(v: PathBuf) -> Server {
        Server {
            vault: v,
            allow_write: true,
            client: Some("test".to_string()),
            model: None,
        }
    }

    #[test]
    fn lists_nested_folders() {
        let v = vault();
        let out = srv(v.clone())
            .call_tool("list_folders", &json!({}))
            .unwrap();
        let folders: Vec<String> = serde_json::from_value(out["folders"].clone()).unwrap();
        assert!(folders.contains(&"Blog".to_string()));
        assert!(folders.contains(&"Blog/KI-Wissen".to_string()));
        assert!(folders.contains(&"Blog/KI-Wissen/Tief".to_string()));
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn lists_notes_of_a_folder_recursively_or_not() {
        let v = vault();
        let s = srv(v.clone());
        let all = s.call_tool("list_notes", &json!({})).unwrap();
        assert_eq!(all["notes"].as_array().unwrap().len(), 4);

        let deep = s
            .call_tool("list_notes", &json!({ "folder": "Blog/KI-Wissen" }))
            .unwrap();
        assert_eq!(
            deep["notes"].as_array().unwrap().len(),
            2,
            "includes the subfolder"
        );

        let shallow = s
            .call_tool(
                "list_notes",
                &json!({ "folder": "Blog/KI-Wissen", "recursive": false }),
            )
            .unwrap();
        assert_eq!(
            shallow["notes"].as_array().unwrap().len(),
            1,
            "that folder alone"
        );
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn moves_a_note_between_folders() {
        let v = vault();
        let out = srv(v.clone())
            .call_tool(
                "move_note",
                &json!({ "path": "Root.md", "folder": "Blog/KI-Wissen" }),
            )
            .unwrap();
        assert_eq!(out["path"], "Blog/KI-Wissen/Root.md");
        assert!(v.join("Blog/KI-Wissen/Root.md").exists());
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn rename_keeps_links_alive_and_delete_reports_breakage() {
        let v = vault();
        let s = srv(v.clone());
        core::write_note(&v, "Verweis.md", "siehe [[Post A]]").unwrap();

        let r = s
            .call_tool(
                "rename_note",
                &json!({ "path": "Blog/KI-Wissen/Post A.md", "new_title": "Post A neu" }),
            )
            .unwrap();
        assert_eq!(r["path"], "Blog/KI-Wissen/Post A neu.md");
        assert_eq!(r["linksUpdated"], 1);
        assert!(std::fs::read_to_string(v.join("Verweis.md"))
            .unwrap()
            .contains("[[Post A neu]]"));

        // Deleting it reports which note is left pointing nowhere.
        let d = s
            .call_tool(
                "delete_note",
                &json!({ "path": "Blog/KI-Wissen/Post A neu.md" }),
            )
            .unwrap();
        assert_eq!(d["nowBrokenLinksIn"][0], "Verweis.md");
        assert!(!v.join("Blog/KI-Wissen/Post A neu.md").exists());

        let f = s
            .call_tool("delete_folder", &json!({ "folder": "Blog" }))
            .unwrap();
        assert!(!v.join("Blog").exists());
        assert_eq!(f["notesDeleted"], 2);
        std::fs::remove_dir_all(&v).ok();
    }

    /// Deleting through an agent has no undo and no trash. The tool description
    /// asks the model to confirm first, which is a request rather than a check —
    /// so the text has to be kept where the history lives (`.magma/`, outside
    /// whatever gets removed).
    #[test]
    fn deleting_keeps_the_text_recoverable() {
        let v = vault();
        let s = srv(v.clone());

        let before = core::read_note(&v, "Root.md").unwrap().content;
        s.call_tool("delete_note", &json!({ "path": "Root.md" }))
            .unwrap();
        assert!(!v.join("Root.md").exists(), "die Datei ist weg");
        let versions = core::list_versions(&v, "Root.md").unwrap();
        assert_eq!(versions.len(), 1, "aber der Text nicht");
        assert_eq!(
            core::read_version(&v, "Root.md", &versions[0].id).unwrap(),
            before
        );

        // Ein ganzer Ordner: jede Notiz darin einzeln, nicht nur die oberste.
        let deep = core::read_note(&v, "Blog/KI-Wissen/Tief/Post B.md")
            .unwrap()
            .content;
        s.call_tool("delete_folder", &json!({ "folder": "Blog" }))
            .unwrap();
        assert!(!v.join("Blog").exists());
        for note in ["Blog/KI-Wissen/Post A.md", "Blog/Uebersicht.md"] {
            assert_eq!(
                core::list_versions(&v, note).unwrap().len(),
                1,
                "kein Schnappschuss fuer {note}"
            );
        }
        let vs = core::list_versions(&v, "Blog/KI-Wissen/Tief/Post B.md").unwrap();
        assert_eq!(vs.len(), 1, "auch verschachtelte Notizen");
        assert_eq!(
            core::read_version(&v, "Blog/KI-Wissen/Tief/Post B.md", &vs[0].id).unwrap(),
            deep
        );
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn read_only_refuses_destructive_tools() {
        let v = vault();
        let s = Server {
            vault: v.clone(),
            allow_write: false,
            client: None,
            model: None,
        };
        for tool in ["delete_note", "delete_folder", "rename_note"] {
            assert!(
                s.call_tool(
                    tool,
                    &json!({ "path": "Root.md", "folder": "Blog", "new_title": "X" })
                )
                .is_err(),
                "{tool} must be refused when read-only"
            );
        }
        assert!(v.join("Root.md").exists());
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn retrieve_returns_passages_with_provenance() {
        let v = vault();
        core::write_note(
            &v,
            "Signieren.md",
            "# Signieren\n\n## Zertifikat\n\nDie p12 kommt aus Meine Zertifikate.\n\n## Notarisierung\n\nApple laesst sich Zeit damit.\n",
        )
        .unwrap();
        let s = srv(v.clone());

        let out = s
            .call_tool(
                "retrieve",
                &json!({ "query": "wo kommt das zertifikat her" }),
            )
            .unwrap();
        let first = &out["passages"][0];
        assert_eq!(first["path"], "Signieren.md");
        // Provenance travels with the passage, or a citation cannot be checked.
        assert_eq!(first["heading"], "Zertifikat");
        assert!(first["line"].as_u64().unwrap() > 1);
        // A passage, not the note: the other section stays behind.
        assert!(!first["text"].as_str().unwrap().contains("Apple"));

        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn retrieve_respects_its_limit() {
        let v = vault();
        for i in 0..12 {
            core::write_note(
                &v,
                &format!("N{i}.md"),
                &format!("# N{i}\n\nEine Notiz ueber Farben.\n"),
            )
            .unwrap();
        }
        let s = srv(v.clone());
        let out = s
            .call_tool("retrieve", &json!({ "query": "farben", "limit": 3 }))
            .unwrap();
        assert_eq!(out["passages"].as_array().unwrap().len(), 3);
        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn retrieve_explains_only_when_asked() {
        let v = vault();
        core::write_note(
            &v,
            "Signieren.md",
            "# Signieren\n\nDie p12 kommt aus Meine Zertifikate.\n",
        )
        .unwrap();
        let s = srv(v.clone());

        // The default answer must stay an answer. An extra key full of notes
        // that were deliberately left out is exactly the kind of thing a model
        // starts citing.
        let plain = s
            .call_tool("retrieve", &json!({ "query": "zertifikate" }))
            .unwrap();
        assert!(plain.get("explain").is_none(), "{plain}");

        let explained = s
            .call_tool(
                "retrieve",
                &json!({ "query": "zertifikate", "explain": true }),
            )
            .unwrap();
        let lexical = explained["explain"]["lexical"].as_array().unwrap();
        assert_eq!(lexical[0], "Signieren.md", "{explained}");
        // No model in this server, so nothing measured meaning — and that has
        // to read as "not measured", not as agreement.
        assert!(explained["explain"]["meaning"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(explained["explain"]["embedded"], 0);
        assert!(explained["explain"]["passages"].as_u64().unwrap() > 0);

        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn naming_a_note_is_enough_to_ask_for_the_detail() {
        let v = vault();
        core::write_note(
            &v,
            "Projekte/Magma 0.1.4.md",
            "# Magma 0.1.4\n\n## Mitgewirkt\n\nDen Updater hat jemand beigesteuert.\n",
        )
        .unwrap();
        let s = srv(v.clone());

        // Without `explain: true` beside it. Naming the note is the request,
        // and an empty answer to it would read as "that note does not exist".
        let out = s
            .call_tool(
                "retrieve",
                &json!({ "query": "updater", "explain_note": "magma 0.1.4" }),
            )
            .unwrap();
        let note = out["explain"]["note"].as_array().unwrap();
        assert_eq!(note.len(), 1, "{out}");
        assert_eq!(note[0]["path"], "Projekte/Magma 0.1.4.md");
        assert_eq!(note[0]["lexicalRank"], 1);
        assert!(note[0]["words"].as_u64().unwrap() > 0);

        std::fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn structure_tools_are_advertised() {
        let names: Vec<String> = tools_spec()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for expected in [
            "retrieve",
            "list_folders",
            "list_notes",
            "create_folder",
            "move_note",
            "rename_note",
            "delete_note",
            "delete_folder",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "missing tool: {expected}"
            );
        }
    }
}
