//! Magma desktop shell. Exposes `magma-core` vault operations to the React UI
//! as Tauri commands. The MCP server (milestone M3) reuses the same core crate
//! so the LLM and the UI act on identical files.

use magma_core as vault;
use magma_webdav as webdav;
use serde_json::json;
use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn pick_vault(app: tauri::AppHandle) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    rx.recv()
        .ok()
        .flatten()
        .map(|p| p.to_string())
}

#[tauri::command]
fn list_notes(vault: String) -> Result<Vec<vault::NoteMeta>, String> {
    vault::list_notes(&PathBuf::from(vault)).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_note(vault: String, path: String) -> Result<vault::Note, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::read_note(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_note(vault: String, path: String, content: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::write_note(&root, &path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_note(vault: String, title: String) -> Result<String, String> {
    let root = PathBuf::from(vault);
    // Seed with an H1 of the title so the note isn't blank on open.
    let body = format!("# {}\n\n", title.trim());
    vault::create_note(&root, &title, &body).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_note(vault: String, path: String, new_title: String) -> Result<String, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::rename_note(&root, &path, &new_title).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_note(vault: String, path: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::delete_note(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_note(vault: String, path: String, folder: String) -> Result<String, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::move_note(&root, &path, &folder).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_folder(vault: String, name: String) -> Result<String, String> {
    vault::create_folder(&PathBuf::from(vault), &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_folder(vault: String, folder: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &folder).ok_or_else(|| "invalid path".to_string())?;
    vault::delete_folder(&root, &folder).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_folders(vault: String) -> Result<Vec<String>, String> {
    vault::list_folders(&PathBuf::from(vault)).map_err(|e| e.to_string())
}

/// Import a WordPress blog into a folder, returning how many notes were written.
#[tauri::command]
async fn import_wordpress(
    vault: String,
    folder: String,
    site_url: String,
    author: Option<String>,
) -> Result<magma_import::ImportSummary, String> {
    // Network + file writes can take a while — run off the main thread.
    let author = author.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        magma_import::import_wordpress(&PathBuf::from(vault), &folder, &site_url, &author)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn save_asset(vault: String, file_name: String, bytes: Vec<u8>) -> Result<String, String> {
    let root = PathBuf::from(vault);
    vault::save_asset(&root, &file_name, &bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn build_graph(vault: String) -> Result<vault::Graph, String> {
    vault::build_graph(&PathBuf::from(vault)).map_err(|e| e.to_string())
}

#[tauri::command]
fn backlinks(vault: String, path: String) -> Result<Vec<vault::NoteMeta>, String> {
    vault::backlinks(&PathBuf::from(vault), &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn search(vault: String, query: String) -> Result<Vec<vault::SearchHit>, String> {
    vault::search(&PathBuf::from(vault), &query).map_err(|e| e.to_string())
}

// --- Optional remote (WebDAV) vault ---------------------------------------
//
// A remote vault is synced into a local cache directory, which the rest of the
// app then treats as an ordinary vault. Writes go to the cache and are pushed
// back to the server (write-through) by the frontend.

fn remote_client(
    url: String,
    username: String,
    password: String,
) -> Result<webdav::WebDavClient, String> {
    webdav::WebDavClient::new(webdav::WebDavConfig {
        base_url: url,
        username,
        password,
    })
    .map_err(|e| e.to_string())
}

/// Connect to a remote WebDAV vault: download all notes into a stable local
/// cache directory and return that path for the app to open as the vault.
#[tauri::command]
fn remote_connect(
    app: tauri::AppHandle,
    url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    let client = remote_client(url.clone(), username, password)?;
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = base.join("remote-vaults").join(format!("{:x}", djb2(&url)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    client.download_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

/// Push a note to the remote vault (write-through after a local save).
#[tauri::command]
fn remote_put(
    url: String,
    username: String,
    password: String,
    path: String,
    content: String,
) -> Result<(), String> {
    remote_client(url, username, password)?
        .put_text(&path, &content)
        .map_err(|e| e.to_string())
}

/// Delete a note from the remote vault.
#[tauri::command]
fn remote_delete(
    url: String,
    username: String,
    password: String,
    path: String,
) -> Result<(), String> {
    remote_client(url, username, password)?
        .delete(&path)
        .map_err(|e| e.to_string())
}

/// Small stable hash for naming the per-vault cache folder (no extra deps).
fn djb2(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

// --- Foolproof MCP setup --------------------------------------------------
//
// Rather than shipping a separate server the user must install, Magma serves
// MCP from its own executable when launched as `magma --mcp <vault>` (see the
// early return in `run`). The one-click installer writes that command straight
// into Claude Desktop's config, so connecting Claude is a single button.

/// The recommended MCP client entry: run *this* executable with `--mcp <vault>`.
fn mcp_server_entry(vault: &str) -> serde_json::Value {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "magma".to_string());
    json!({ "command": exe, "args": ["--mcp", vault] })
}

/// Claude Desktop's config file location for this OS.
fn claude_desktop_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h).join("Library/Application Support/Claude/claude_desktop_config.json")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(|a| PathBuf::from(a).join("Claude/claude_desktop_config.json"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config/Claude/claude_desktop_config.json"))
    }
}

/// The exact MCP config JSON for the given vault (for display / manual copy).
#[tauri::command]
fn mcp_config(vault: String) -> String {
    let cfg = json!({ "mcpServers": { "magma": mcp_server_entry(&vault) } });
    serde_json::to_string_pretty(&cfg).unwrap_or_default()
}

/// One-click install: merge the Magma server into Claude Desktop's config,
/// backing up any existing file. Returns the config path that was written.
#[tauri::command]
fn install_mcp(vault: String) -> Result<String, String> {
    let path =
        claude_desktop_config_path().ok_or("could not locate the Claude Desktop config folder")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Preserve any existing config and back it up before writing.
    let mut root: serde_json::Value = if path.exists() {
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let _ = std::fs::write(path.with_extension("json.bak"), &text);
        serde_json::from_str(&text).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    if !root.is_object() {
        root = json!({});
    }
    let obj = root.as_object_mut().unwrap();
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers
        .as_object_mut()
        .unwrap()
        .insert("magma".to_string(), mcp_server_entry(&vault));
    let pretty = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&path, pretty).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Open an http(s) URL in the user's default browser. Used for real links in
/// notes (the WebView must not navigate away from the app itself).
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("only http(s) links can be opened".into());
    }
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = std::process::Command::new("xdg-open").arg(&url).spawn();
    spawned.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Serve MCP from this same executable when invoked as `magma --mcp <vault>`
    // (this is what the one-click installer wires into Claude Desktop). We do
    // this before any Tauri/GUI init so it works headless when Claude spawns us.
    let raw_args: Vec<String> = std::env::args().collect();
    if let Some(i) = raw_args.iter().position(|a| a == "--mcp") {
        let vault = raw_args.get(i + 1).cloned().unwrap_or_default();
        let allow_write = !matches!(
            std::env::var("MAGMA_MCP_ALLOW_WRITE").ok().as_deref(),
            Some("0") | Some("false") | Some("no")
        );
        magma_mcp::serve_stdio(PathBuf::from(vault), allow_write);
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_vault,
            list_notes,
            read_note,
            write_note,
            create_note,
            rename_note,
            delete_note,
            move_note,
            create_folder,
            delete_folder,
            list_folders,
            import_wordpress,
            save_asset,
            build_graph,
            backlinks,
            search,
            remote_connect,
            remote_put,
            remote_delete,
            mcp_config,
            install_mcp,
            open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running Magma");
}
