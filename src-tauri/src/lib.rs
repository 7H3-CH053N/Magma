//! Magma desktop shell. Exposes `magma-core` vault operations to the React UI
//! as Tauri commands. The MCP server (milestone M3) reuses the same core crate
//! so the LLM and the UI act on identical files.

use magma_core as vault;
use std::path::PathBuf;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_vault,
            list_notes,
            read_note,
            write_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running Magma");
}
