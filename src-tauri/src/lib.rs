//! Magma desktop shell. Exposes `magma-core` vault operations to the React UI
//! as Tauri commands. The MCP server (milestone M3) reuses the same core crate
//! so the LLM and the UI act on identical files.

use magma_core as vault;
use magma_webdav as webdav;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            save_asset,
            build_graph,
            backlinks,
            search,
            remote_connect,
            remote_put,
            remote_delete
        ])
        .run(tauri::generate_context!())
        .expect("error while running Magma");
}
