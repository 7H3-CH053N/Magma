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

/// How long to leave between version snapshots of the same note while it is
/// being edited. Autosave fires seconds apart; without a gap the history would
/// be a hundred near-identical copies of one afternoon and nothing older.
const SNAPSHOT_EVERY_SECS: u64 = 120;

#[tauri::command]
fn write_note(vault: String, path: String, content: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    // Keep what is about to be overwritten. Best-effort: a history that cannot
    // be written must never stop the note itself from being saved.
    let _ = vault::snapshot_if_due(&root, &path, SNAPSHOT_EVERY_SECS);
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
    // Link-safe: repoint every [[wikilink]] that named the old filename.
    let (new_path, _updated) =
        vault::rename_note_updating_links(&root, &path, &new_title).map_err(|e| e.to_string())?;
    vault::relocate_history(&root, &path, &new_path);
    Ok(new_path)
}

#[tauri::command]
fn delete_note(vault: String, path: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    vault::delete_note(&root, &path).map_err(|e| e.to_string())?;
    vault::forget_history(&root, &path);
    Ok(())
}

#[tauri::command]
fn move_note(vault: String, path: String, folder: String) -> Result<String, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    let new_path = vault::move_note(&root, &path, &folder).map_err(|e| e.to_string())?;
    vault::relocate_history(&root, &path, &new_path);
    Ok(new_path)
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

/// Move a folder (with everything in it) into another folder; "" = vault root.
#[tauri::command]
fn move_folder(vault: String, folder: String, into: String) -> Result<String, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &folder).ok_or_else(|| "invalid path".to_string())?;
    vault::safe_join(&root, &into).ok_or_else(|| "invalid path".to_string())?;
    let moved = vault::move_folder(&root, &folder, &into).map_err(|e| e.to_string())?;
    // History is filed under the note path, so it mirrors the folder tree and
    // moves as one piece.
    vault::relocate_history(&root, &folder, &moved);
    Ok(moved)
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
    author_note: Option<String>,
) -> Result<magma_import::ImportSummary, String> {
    // Network + file writes can take a while — run off the main thread.
    let author = author.unwrap_or_default();
    let author_note = author_note.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        magma_import::import_wordpress(
            &PathBuf::from(vault),
            &folder,
            &site_url,
            &author,
            &author_note,
        )
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
fn build_graph(vault: String, exclude: Option<Vec<String>>) -> Result<vault::Graph, String> {
    vault::build_graph(&PathBuf::from(vault), &exclude.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn backlinks(vault: String, path: String) -> Result<Vec<vault::NoteMeta>, String> {
    vault::backlinks(&PathBuf::from(vault), &path).map_err(|e| e.to_string())
}

/// Search, isolated from the rest of the app. A panic in here (a bad slice, a
/// pathological note) must surface as an error message, never take the window
/// down — losing the whole app because a search hiccuped is not acceptable.
#[tauri::command]
fn search(vault: String, query: String) -> Result<Vec<vault::SearchHit>, String> {
    let root = PathBuf::from(vault);
    std::panic::catch_unwind(|| vault::search(&root, &query))
        .map_err(|_| "search failed on this vault — please report the query".to_string())?
        .map_err(|e| e.to_string())
}

/// Vault-wide find & replace. `dryRun` reports what would change without
/// writing, so a bulk rewrite is never fired blind. With `renameNotes`, notes
/// whose own name carries the term are renamed too, so `[[wikilinks]]` and the
/// notes they point at stay in sync.
#[tauri::command]
fn replace_all(
    vault: String,
    find: String,
    replace: String,
    dry_run: bool,
    rename_notes: bool,
) -> Result<vault::ReplaceReport, String> {
    let root = PathBuf::from(vault);
    if !dry_run {
        // Snapshot everything this is about to touch, unconditionally — the
        // preview shows what will change, the history is how you take it back.
        if let Ok(preview) =
            vault::replace_in_vault(&root, &find, &replace, true, rename_notes)
        {
            for hit in &preview.hits {
                let _ = vault::snapshot(&root, &hit.path);
            }
            for rename in &preview.renames {
                let _ = vault::snapshot(&root, &rename.path);
            }
        }
    }
    vault::replace_in_vault(&root, &find, &replace, dry_run, rename_notes)
        .map_err(|e| e.to_string())
}

/// Open (or create) a note at an exact name — daily notes and notes made from
/// a template. Returns the path plus whether it was created just now.
#[tauri::command]
fn open_or_create(
    vault: String,
    folder: String,
    title: String,
    content: String,
) -> Result<(String, bool), String> {
    vault::open_or_create(&PathBuf::from(vault), &folder, &title, &content)
        .map_err(|e| e.to_string())
}

/// Append text to a note without opening it — what quick capture writes.
#[tauri::command]
fn append_note(vault: String, path: String, text: String) -> Result<(), String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    let _ = vault::snapshot_if_due(&root, &path, SNAPSHOT_EVERY_SECS);
    vault::append_note(&root, &path, &text).map_err(|e| e.to_string())
}

// --- Version history -------------------------------------------------------

#[tauri::command]
fn list_versions(vault: String, path: String) -> Result<Vec<vault::Version>, String> {
    vault::list_versions(&PathBuf::from(vault), &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_version(vault: String, path: String, id: String) -> Result<String, String> {
    vault::read_version(&PathBuf::from(vault), &path, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_version(vault: String, path: String, id: String) -> Result<(), String> {
    vault::restore(&PathBuf::from(vault), &path, &id).map_err(|e| e.to_string())
}

// --- Connections: outgoing links, unlinked mentions, related notes ---------

#[tauri::command]
fn outgoing_links(vault: String, path: String) -> Result<Vec<vault::OutgoingLink>, String> {
    vault::outgoing_links(&PathBuf::from(vault), &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn unlinked_mentions(vault: String, path: String) -> Result<Vec<vault::Mention>, String> {
    let root = PathBuf::from(vault);
    // Scans every note's text; same isolation as search, for the same reason.
    std::panic::catch_unwind(|| vault::unlinked_mentions(&root, &path))
        .map_err(|_| "scanning for mentions failed on this vault".to_string())?
        .map_err(|e| e.to_string())
}

/// Turn plain-text mentions of `name` inside `path` into `[[name]]` links.
#[tauri::command]
fn link_mentions(vault: String, path: String, name: String) -> Result<usize, String> {
    let root = PathBuf::from(vault);
    vault::safe_join(&root, &path).ok_or_else(|| "invalid path".to_string())?;
    let _ = vault::snapshot(&root, &path);
    vault::link_mentions(&root, &path, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn related_notes(
    vault: String,
    path: String,
    limit: Option<usize>,
) -> Result<Vec<vault::RelatedNote>, String> {
    vault::related_notes(&PathBuf::from(vault), &path, limit.unwrap_or(8))
        .map_err(|e| e.to_string())
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

// --- Remembering the vault across restarts --------------------------------
//
// Deliberately a file next to Magma's own settings rather than the WebView's
// localStorage: clearing the app's web data (or a WebView reset on update)
// would otherwise dump you back on the "open a vault" screen with no idea
// which folder it was.

/// Magma's own config file: `<config dir>/Magma/settings.json`.
fn app_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support"))?;
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("Magma/settings.json"))
}

fn read_app_settings() -> serde_json::Value {
    app_settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}))
}

/// The vault that was open last time — `None` on a first run, and also when
/// the folder has since been moved or deleted (never hand back a dead path).
#[tauri::command]
fn last_vault() -> Option<String> {
    let v = read_app_settings();
    let path = v.get("vault")?.as_str()?.to_string();
    PathBuf::from(&path).is_dir().then_some(path)
}

/// Remember the vault for the next start.
#[tauri::command]
fn set_last_vault(vault: String) -> Result<(), String> {
    let path = app_settings_path().ok_or("could not locate the settings folder")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut root = read_app_settings();
    root.as_object_mut().unwrap().insert("vault".into(), json!(vault));
    std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap_or_default())
        .map_err(|e| e.to_string())
}

/// The recommended MCP client entry: run *this* executable with `--mcp <vault>`.
fn mcp_executable() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "magma".to_string())
}

fn mcp_server_entry(vault: &str) -> serde_json::Value {
    json!({
        "command": mcp_executable(),
        "args": ["--mcp", vault],
        "env": { "MAGMA_MCP_CLIENT": "claude" }
    })
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

/// The equivalent Codex CLI MCP config block for display / manual copy.
#[tauri::command]
fn codex_mcp_config(vault: String) -> String {
    codex_mcp_config_block(&mcp_executable(), &vault)
}

/// One-click install: merge the Magma server into Claude Desktop's config,
/// backing up any existing file. Returns the config path that was written.
#[tauri::command]
fn install_mcp(vault: String) -> Result<McpInstall, String> {
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
    let servers = servers.as_object_mut().unwrap();

    // Replace *any* previous Magma entry, not just one named "magma": an older
    // install may sit under a different key or point at a stale executable or
    // vault, and leaving it behind means Claude keeps talking to the old one.
    let stale: Vec<String> = servers
        .iter()
        .filter(|(key, val)| {
            if key.as_str() == "magma" {
                return true;
            }
            let text = val.to_string().to_lowercase();
            text.contains("--mcp") && text.contains("magma")
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in stale {
        servers.remove(&key);
    }
    servers.insert("magma".to_string(), mcp_server_entry(&vault));
    let pretty = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&path, pretty).map_err(|e| e.to_string())?;

    // A binary inside cargo's target/ directory is rebuilt (and briefly absent)
    // on every `npm run tauri dev`, which is exactly what makes Claude Desktop
    // report "Server disconnected". Say so instead of leaving it a mystery.
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let dev_build = exe.contains("/target/debug/")
        || exe.contains("/target/release/")
        || exe.contains("\\target\\debug\\")
        || exe.contains("\\target\\release\\");
    Ok(McpInstall {
        config_path: path.to_string_lossy().to_string(),
        executable: exe,
        dev_build,
    })
}

/// One-click install for Codex: write Codex's config file directly. A GUI app
/// launched from Finder does not inherit the user's shell PATH, so invoking the
/// `codex` binary would fail for many valid installs.
#[tauri::command]
fn install_codex_mcp(vault: String) -> Result<McpInstall, String> {
    let exe = mcp_executable();
    let config_path = codex_config_path().ok_or("could not locate the Codex config folder")?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.exists() {
        let _ = std::fs::write(config_path.with_extension("toml.bak"), &existing);
    }
    let updated = merge_codex_mcp_config(&existing, &exe, &vault);
    std::fs::write(&config_path, updated).map_err(|e| e.to_string())?;
    let dev_build = exe.contains("/target/debug/")
        || exe.contains("/target/release/")
        || exe.contains("\\target\\debug\\")
        || exe.contains("\\target\\release\\");
    Ok(McpInstall {
        config_path: config_path.to_string_lossy().to_string(),
        executable: exe,
        dev_build,
    })
}

fn codex_config_path() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
        .map(|dir| dir.join("config.toml"))
}

fn codex_mcp_config_block(command: &str, vault: &str) -> String {
    format!(
        "[mcp_servers.magma]\nenabled = true\ncommand = {command:?}\nargs = [\"--mcp\", {vault:?}]\n\n[mcp_servers.magma.env]\nMAGMA_MCP_CLIENT = \"codex\"\n",
    )
}

fn merge_codex_mcp_config(existing: &str, command: &str, vault: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping = trimmed == "[mcp_servers.magma]" || trimmed == "[mcp_servers.magma.env]";
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    let mut out = out.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&codex_mcp_config_block(command, vault));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_merge_replaces_only_magma_server() {
        let existing = r#"
model = "gpt-5"

[mcp_servers.other]
command = "other"

[mcp_servers.magma]
enabled = true
command = "old"
args = ["--old"]

[mcp_servers.magma.env]
MAGMA_MCP_CLIENT = "old"

[profiles.default]
approval_policy = "never"
"#;
        let merged = merge_codex_mcp_config(existing, "/Applications/Magma.app/magma", "/vault");
        assert!(merged.contains("[mcp_servers.other]"));
        assert!(merged.contains("[profiles.default]"));
        assert!(!merged.contains("command = \"old\""));
        assert!(merged.contains("command = \"/Applications/Magma.app/magma\""));
        assert!(merged.contains("args = [\"--mcp\", \"/vault\"]"));
        assert!(merged.contains("MAGMA_MCP_CLIENT = \"codex\""));
    }
}

/// What the one-click setup wrote, so the UI can be specific about it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstall {
    config_path: String,
    executable: String,
    /// True when the registered binary lives in a cargo build directory.
    dev_build: bool,
}

// --- The native menu bar, in the language you chose -----------------------
//
// Tauri installs a default menu whose labels are hardcoded English. On macOS
// that menu is the one strip of Magma the user cannot restyle away, so an
// English "Edit / Undo / Paste" above a German app is the most visible thing
// in the window that ignores the language setting. We build the whole menu
// ourselves instead, and rebuild it when the language changes.
//
// The items macOS injects into the edit menu itself — Writing Tools, AutoFill,
// Start Dictation, Emoji & Symbols — belong to AppKit and follow the *system*
// language, not ours. Nothing an app can do about those.

fn menu_text(lang: &str, key: &str) -> &'static str {
    let de = lang.starts_with("de");
    match (key, de) {
        ("edit", true) => "Bearbeiten",
        ("edit", false) => "Edit",
        ("view", true) => "Ansicht",
        ("view", false) => "View",
        ("window", true) => "Fenster",
        ("window", false) => "Window",
        ("undo", true) => "Rückgängig",
        ("undo", false) => "Undo",
        ("redo", true) => "Wiederholen",
        ("redo", false) => "Redo",
        ("cut", true) => "Ausschneiden",
        ("cut", false) => "Cut",
        ("copy", true) => "Kopieren",
        ("copy", false) => "Copy",
        ("paste", true) => "Einfügen",
        ("paste", false) => "Paste",
        ("selectAll", true) => "Alles auswählen",
        ("selectAll", false) => "Select All",
        ("about", true) => "Über Magma",
        ("about", false) => "About Magma",
        ("services", true) => "Dienste",
        ("services", false) => "Services",
        ("hide", true) => "Magma ausblenden",
        ("hide", false) => "Hide Magma",
        ("hideOthers", true) => "Andere ausblenden",
        ("hideOthers", false) => "Hide Others",
        ("showAll", true) => "Alle einblenden",
        ("showAll", false) => "Show All",
        ("quit", true) => "Magma beenden",
        ("quit", false) => "Quit Magma",
        ("fullscreen", true) => "Vollbild",
        ("fullscreen", false) => "Full Screen",
        ("minimize", true) => "Minimieren",
        ("minimize", false) => "Minimize",
        ("close", true) => "Fenster schließen",
        ("close", false) => "Close Window",
        _ => "",
    }
}

fn build_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    lang: &str,
) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, PredefinedMenuItem as P, Submenu};
    let t = |key: &str| menu_text(lang, key);

    // On macOS the first submenu becomes the application menu; elsewhere it is
    // an ordinary one, which is why it carries the app name either way.
    let app_menu = Submenu::with_items(
        app,
        "Magma",
        true,
        &[
            &P::about(app, Some(t("about")), None)?,
            &P::separator(app)?,
            &P::services(app, Some(t("services")))?,
            &P::separator(app)?,
            &P::hide(app, Some(t("hide")))?,
            &P::hide_others(app, Some(t("hideOthers")))?,
            &P::show_all(app, Some(t("showAll")))?,
            &P::separator(app)?,
            &P::quit(app, Some(t("quit")))?,
        ],
    )?;

    let edit = Submenu::with_items(
        app,
        t("edit"),
        true,
        &[
            &P::undo(app, Some(t("undo")))?,
            &P::redo(app, Some(t("redo")))?,
            &P::separator(app)?,
            &P::cut(app, Some(t("cut")))?,
            &P::copy(app, Some(t("copy")))?,
            &P::paste(app, Some(t("paste")))?,
            &P::select_all(app, Some(t("selectAll")))?,
        ],
    )?;

    let view = Submenu::with_items(app, t("view"), true, &[&P::fullscreen(app, Some(t("fullscreen")))?])?;

    let window = Submenu::with_items(
        app,
        t("window"),
        true,
        &[
            &P::minimize(app, Some(t("minimize")))?,
            &P::close_window(app, Some(t("close")))?,
        ],
    )?;

    // No "File" menu on purpose: everything Magma does with notes lives in the
    // command palette, and a File menu holding only "Close window" is furniture.
    Menu::with_items(app, &[&app_menu, &edit, &view, &window])
}

fn stored_language() -> String {
    read_app_settings()
        .get("lang")
        .and_then(|v| v.as_str())
        .unwrap_or("en")
        .to_string()
}

/// Remember the interface language and relabel the native menu bar to match.
#[tauri::command]
fn set_language(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    if let Some(path) = app_settings_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut root = read_app_settings();
        root.as_object_mut().unwrap().insert("lang".into(), json!(lang));
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&root).unwrap_or_default());
    }
    let menu = build_menu(&app, &lang).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
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
        // Build the menu from the stored language before the window shows, so
        // it never flashes English on the way to German.
        .setup(|app| {
            let handle = app.handle().clone();
            let menu = build_menu(&handle, &stored_language())?;
            handle.set_menu(menu)?;
            Ok(())
        })
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
            move_folder,
            list_folders,
            import_wordpress,
            save_asset,
            build_graph,
            backlinks,
            search,
            replace_all,
            open_or_create,
            append_note,
            list_versions,
            read_version,
            restore_version,
            outgoing_links,
            unlinked_mentions,
            link_mentions,
            related_notes,
            remote_connect,
            remote_put,
            remote_delete,
            mcp_config,
            codex_mcp_config,
            install_mcp,
            install_codex_mcp,
            last_vault,
            set_last_vault,
            set_language,
            open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running Magma");
}
