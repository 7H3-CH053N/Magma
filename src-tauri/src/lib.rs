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
            save_asset
        ])
        .run(tauri::generate_context!())
        .expect("error while running Magma");
}
