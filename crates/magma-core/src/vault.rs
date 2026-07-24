//! Vault access: the file system is the source of truth. A vault is just a
//! folder of markdown files, so nothing here locks a user in — the same files
//! open in any editor, including Obsidian.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    /// Path relative to the vault root, using forward slashes.
    pub path: String,
    pub title: String,
    /// True when frontmatter declares `author: ai` — a note an LLM wrote.
    pub ai_authored: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub path: String,
    pub title: String,
    pub ai_authored: bool,
    pub content: String,
}

/// Walk the vault (skipping hidden and `.obsidian`-style dirs) and collect
/// every markdown file as note metadata, sorted by title.
pub fn list_notes(vault: &Path) -> std::io::Result<Vec<NoteMeta>> {
    let mut out = Vec::new();
    collect(vault, vault, &mut out)?;
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<NoteMeta>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            out.push(NoteMeta {
                path: rel_path(root, &path),
                title: title_of(&path),
                ai_authored: is_ai_authored(&content),
            });
        }
    }
    Ok(())
}

pub fn read_note(vault: &Path, rel: &str) -> std::io::Result<Note> {
    let full = vault.join(rel);
    let content = fs::read_to_string(&full)?;
    Ok(Note {
        path: rel.to_string(),
        title: title_of(&full),
        ai_authored: is_ai_authored(&content),
        content,
    })
}

pub fn write_note(vault: &Path, rel: &str, content: &str) -> std::io::Result<()> {
    let full = vault.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn title_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// Detect an `author: ai` line inside a leading YAML frontmatter block.
fn is_ai_authored(content: &str) -> bool {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return false;
    }
    // Frontmatter is everything up to the closing `---`.
    let after = &trimmed[3..];
    let end = match after.find("\n---") {
        Some(i) => i,
        None => return false,
    };
    after[..end].lines().any(|line| {
        let l = line.trim().to_lowercase();
        l == "author: ai" || l == "author: \"ai\"" || l == "author: 'ai'"
    })
}

/// Build a path relative to the vault, guarding against escaping it via `..`.
pub fn safe_join(vault: &Path, rel: &str) -> Option<PathBuf> {
    if rel.split(['/', '\\']).any(|c| c == "..") {
        return None;
    }
    Some(vault.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ai_frontmatter() {
        let md = "---\ntitle: X\nauthor: ai\n---\n\nbody";
        assert!(is_ai_authored(md));
    }

    #[test]
    fn plain_note_is_not_ai() {
        assert!(!is_ai_authored("# Just a heading\n\ntext"));
        assert!(!is_ai_authored("---\nauthor: alex\n---\ntext"));
    }

    #[test]
    fn title_comes_from_stem() {
        assert_eq!(title_of(Path::new("/v/ideas/second brain.md")), "second brain");
    }

    #[test]
    fn safe_join_blocks_traversal() {
        let v = Path::new("/vault");
        assert!(safe_join(v, "../etc/passwd").is_none());
        assert!(safe_join(v, "notes/ok.md").is_some());
    }
}
