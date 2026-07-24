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
}

fn main() {
    let vault = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("MAGMA_VAULT").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("magma-mcp: no vault given (arg 1 or MAGMA_VAULT)");
            std::process::exit(2);
        });
    let allow_write = !matches!(
        std::env::var("MAGMA_MCP_ALLOW_WRITE").ok().as_deref(),
        Some("0") | Some("false") | Some("no")
    );

    let server = Server { vault, allow_write };
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
                write_msg(&mut out, error_response(Value::Null, -32700, &format!("parse error: {e}")));
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
            other => Some(error_response(id, -32601, &format!("method not found: {other}"))),
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
            "read_note" => {
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let note = core::read_note(v, &path).map_err(io)?;
                Ok(json!(note))
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
            "create_note" => {
                self.ensure_write()?;
                let title = str_arg(args, "title")?;
                let content = str_arg(args, "content")?;
                let res = core::ai_create_note(v, &title, &content).map_err(io)?;
                Ok(json!(res))
            }
            "update_note" => {
                self.ensure_write()?;
                let path = str_arg(args, "path")?;
                core::safe_join(v, &path).ok_or("invalid path")?;
                let content = str_arg(args, "content")?;
                let res = core::ai_update_note(v, &path, &content).map_err(io)?;
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
            "name": "read_note",
            "description": "Read a note's full markdown by its vault-relative path (e.g. 'ideas/second-brain.md').",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
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
            "name": "create_note",
            "description": "Create a new note authored by the AI (frontmatter author: ai is added automatically). Reference related notes with [[Title]] wikilinks — call find_link_candidates first to know which. The response reports resolved and broken links; fix any broken ones with a follow-up update_note.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "content": { "type": "string", "description": "Markdown body. Use [[Title]] to link existing notes." }
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
