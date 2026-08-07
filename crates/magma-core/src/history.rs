//! Version history: a copy of a note's previous content, kept beside the vault
//! so nothing is ever silently overwritten.
//!
//! Two things made this necessary: an LLM writes into the same files the user
//! does, and a vault-wide replace can rewrite hundreds of notes in one click.
//! A preview helps, but only a way back makes those safe.
//!
//! Snapshots live in `<vault>/.magma/history/<note path>/<millis>.md`. The
//! directory starts with a dot, so `list_notes` walks straight past it and the
//! history never shows up as notes.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many snapshots to keep per note. Old ones are dropped oldest-first.
const KEEP: usize = 50;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Opaque id (milliseconds since the epoch, as a string).
    pub id: String,
    /// When the snapshot was taken, in milliseconds since the epoch.
    pub taken_at: u64,
    pub bytes: u64,
}

fn history_dir(vault: &Path, rel: &str) -> Option<PathBuf> {
    if rel.split(['/', '\\']).any(|c| c == ".." || c.is_empty()) {
        return None;
    }
    Some(vault.join(".magma/history").join(rel))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Every snapshot of a note, newest first.
pub fn list_versions(vault: &Path, rel: &str) -> std::io::Result<Vec<Version>> {
    let dir = match history_dir(vault, rel) {
        Some(d) if d.is_dir() => d,
        _ => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let taken_at = id.parse::<u64>().unwrap_or(0);
        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(Version {
            id,
            taken_at,
            bytes,
        });
    }
    out.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    Ok(out)
}

pub fn read_version(vault: &Path, rel: &str, id: &str) -> std::io::Result<String> {
    // The id comes back from `list_versions`, but it arrives here via the UI,
    // so treat it as untrusted: digits only, no path pieces.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid version id",
        ));
    }
    let dir = history_dir(vault, rel)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?;
    fs::read_to_string(dir.join(format!("{id}.md")))
}

/// Copy the note's current content into the history.
///
/// Skipped when the newest snapshot already holds exactly this content — an
/// autosave that changed nothing should not push older versions out.
pub fn snapshot(vault: &Path, rel: &str) -> std::io::Result<Option<String>> {
    let current = match fs::read_to_string(vault.join(rel)) {
        Ok(c) => c,
        // Nothing on disk yet (a brand new note) — nothing to preserve.
        Err(_) => return Ok(None),
    };
    let dir = history_dir(vault, rel)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?;
    let versions = list_versions(vault, rel)?;
    if let Some(newest) = versions.first() {
        if read_version(vault, rel, &newest.id).ok().as_deref() == Some(current.as_str()) {
            return Ok(None);
        }
    }
    fs::create_dir_all(&dir)?;
    // Two snapshots inside the same millisecond (a restore snapshots the
    // current text and writes the old one back) would otherwise land on the
    // same filename and the earlier version would be lost.
    let mut ms = now_ms();
    while dir.join(format!("{ms}.md")).exists() {
        ms += 1;
    }
    let id = ms.to_string();
    fs::write(dir.join(format!("{id}.md")), &current)?;

    // Drop the oldest once the list grows past KEEP.
    let versions = list_versions(vault, rel)?;
    for old in versions.iter().skip(KEEP) {
        let _ = fs::remove_file(dir.join(format!("{}.md", old.id)));
    }
    Ok(Some(id))
}

/// Snapshot only if the newest one is older than `min_interval_secs`.
///
/// Autosave fires every time typing pauses; without this, a morning of writing
/// would leave a hundred near-identical snapshots and push out the version from
/// before the session, which is the one actually worth having.
pub fn snapshot_if_due(
    vault: &Path,
    rel: &str,
    min_interval_secs: u64,
) -> std::io::Result<Option<String>> {
    if let Some(newest) = list_versions(vault, rel)?.first() {
        if now_ms().saturating_sub(newest.taken_at) < min_interval_secs * 1000 {
            return Ok(None);
        }
    }
    snapshot(vault, rel)
}

/// Put an old version back, after snapshotting what is there now — so a restore
/// is itself undoable.
pub fn restore(vault: &Path, rel: &str, id: &str) -> std::io::Result<()> {
    let old = read_version(vault, rel, id)?;
    snapshot(vault, rel)?;
    crate::vault::write_note(vault, rel, &old)
}

/// Forget a note's history — used when the note itself is deleted, so the
/// vault doesn't accumulate snapshots of files that no longer exist.
pub fn forget(vault: &Path, rel: &str) {
    if let Some(dir) = history_dir(vault, rel) {
        let _ = fs::remove_dir_all(dir);
    }
}

/// Follow a note that was renamed or moved, so its history moves with it.
pub fn relocate(vault: &Path, from: &str, to: &str) {
    let (Some(a), Some(b)) = (history_dir(vault, from), history_dir(vault, to)) else {
        return;
    };
    if !a.is_dir() {
        return;
    }
    if let Some(parent) = b.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::rename(a, b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault;

    /// A vault per test: these run in parallel, and two of them sharing a
    /// directory is what a millisecond-resolution name would give us.
    fn tmp_vault(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "magma-hist-{tag}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn snapshots_then_restores() {
        let v = tmp_vault("restore");
        vault::write_note(&v, "A.md", "erste fassung").unwrap();

        snapshot(&v, "A.md").unwrap();
        vault::write_note(&v, "A.md", "zweite fassung").unwrap();

        let versions = list_versions(&v, "A.md").unwrap();
        assert_eq!(versions.len(), 1, "one snapshot of the old text");
        assert_eq!(
            read_version(&v, "A.md", &versions[0].id).unwrap(),
            "erste fassung"
        );

        restore(&v, "A.md", &versions[0].id).unwrap();
        assert_eq!(fs::read_to_string(v.join("A.md")).unwrap(), "erste fassung");
        // Restoring is itself undoable: the text it replaced was kept.
        let after = list_versions(&v, "A.md").unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(
            read_version(&v, "A.md", &after[0].id).unwrap(),
            "zweite fassung"
        );
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn identical_content_makes_no_new_snapshot() {
        let v = tmp_vault("same");
        vault::write_note(&v, "B.md", "unverändert").unwrap();
        assert!(snapshot(&v, "B.md").unwrap().is_some());
        assert!(snapshot(&v, "B.md").unwrap().is_none(), "nothing changed");
        assert_eq!(list_versions(&v, "B.md").unwrap().len(), 1);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn history_is_hidden_from_the_note_list() {
        let v = tmp_vault("hidden");
        vault::write_note(&v, "C.md", "text").unwrap();
        snapshot(&v, "C.md").unwrap();
        let notes = vault::list_notes(&v).unwrap();
        assert_eq!(
            notes.len(),
            1,
            "snapshots must not appear as notes: {notes:?}"
        );
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn version_ids_cannot_escape_the_history_folder() {
        let v = tmp_vault("escape");
        vault::write_note(&v, "D.md", "text").unwrap();
        assert!(read_version(&v, "D.md", "../../secret").is_err());
        assert!(read_version(&v, "D.md", "").is_err());
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn a_rename_takes_the_history_with_it() {
        let v = tmp_vault("rename");
        vault::write_note(&v, "Alt.md", "inhalt").unwrap();
        snapshot(&v, "Alt.md").unwrap();
        relocate(&v, "Alt.md", "Neu.md");
        assert_eq!(list_versions(&v, "Alt.md").unwrap().len(), 0);
        assert_eq!(list_versions(&v, "Neu.md").unwrap().len(), 1);
        fs::remove_dir_all(&v).ok();
    }
}
