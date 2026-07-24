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
                title: title_of(&path, &content),
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
        title: title_of(&full, &content),
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

/// A note's display title: the first heading (or first line of text) in the
/// content — like Bear/Obsidian — falling back to the filename stem. This is
/// why typing a title inside the note names it in the sidebar.
fn title_of(path: &Path, content: &str) -> String {
    if let Some(t) = title_from_content(content) {
        return t;
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string())
}

fn title_from_content(content: &str) -> Option<String> {
    let mut lines = content.lines().peekable();
    // Skip a leading YAML frontmatter block.
    if lines.peek().map(|l| l.trim_end()) == Some("---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim_end() == "---" {
                break;
            }
        }
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Strip leading heading markers and list/quote markers.
        let title = trimmed
            .trim_start_matches('#')
            .trim_start_matches(['>', '-', '*'])
            .trim();
        if !title.is_empty() {
            return Some(title.chars().take(120).collect());
        }
    }
    None
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

/// Turn a title into a safe file stem: keep letters, digits, spaces, dashes
/// and underscores; collapse the rest. Empty titles fall back to "Untitled".
pub fn slugify(title: &str) -> String {
    let cleaned: String = title
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "Untitled".to_string()
    } else {
        cleaned
    }
}

/// Create a new note from a title, returning its vault-relative path. If a note
/// with that name exists, a numeric suffix is appended so nothing is clobbered.
pub fn create_note(vault: &Path, title: &str, content: &str) -> std::io::Result<String> {
    create_note_in(vault, "", title, content)
}

/// Like `create_note`, but places the note inside `folder` (a vault-relative
/// subdirectory, created if needed). Used to group related notes together —
/// e.g. an LLM filing a batch of linked notes under one folder.
pub fn create_note_in(
    vault: &Path,
    folder: &str,
    title: &str,
    content: &str,
) -> std::io::Result<String> {
    let stem = slugify(title);
    let dir = sanitize_folder(folder);
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut rel = format!("{prefix}{stem}.md");
    let mut n = 2;
    while vault.join(&rel).exists() {
        rel = format!("{prefix}{stem} {n}.md");
        n += 1;
    }
    write_note(vault, &rel, content)?;
    Ok(rel)
}

/// Clean a folder path: slugify each segment, drop empty/`..` segments.
fn sanitize_folder(folder: &str) -> String {
    folder
        .split(['/', '\\'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "..")
        .map(slugify)
        .collect::<Vec<_>>()
        .join("/")
}

/// Rename a note to a new title within the same folder. Returns the new
/// vault-relative path.
pub fn rename_note(vault: &Path, rel: &str, new_title: &str) -> std::io::Result<String> {
    let old = vault.join(rel);
    let parent = Path::new(rel).parent();
    let stem = slugify(new_title);
    let mut new_rel = match parent {
        Some(p) if !p.as_os_str().is_empty() => {
            format!("{}/{stem}.md", p.to_string_lossy().replace('\\', "/"))
        }
        _ => format!("{stem}.md"),
    };
    let mut n = 2;
    while vault.join(&new_rel).exists() && vault.join(&new_rel) != old {
        new_rel = match parent {
            Some(p) if !p.as_os_str().is_empty() => {
                format!("{}/{stem} {n}.md", p.to_string_lossy().replace('\\', "/"))
            }
            _ => format!("{stem} {n}.md"),
        };
        n += 1;
    }
    fs::rename(old, vault.join(&new_rel))?;
    Ok(new_rel)
}

/// Delete a note from the vault.
pub fn delete_note(vault: &Path, rel: &str) -> std::io::Result<()> {
    fs::remove_file(vault.join(rel))
}

/// Save pasted image bytes into the vault's `assets/` folder under a
/// caller-provided name, returning the vault-relative path for embedding.
pub fn save_asset(vault: &Path, file_name: &str, bytes: &[u8]) -> std::io::Result<String> {
    let dir = vault.join("assets");
    fs::create_dir_all(&dir)?;
    let mut rel = format!("assets/{file_name}");
    let (base, ext) = split_ext(file_name);
    let mut n = 2;
    while vault.join(&rel).exists() {
        rel = match &ext {
            Some(e) => format!("assets/{base} {n}.{e}"),
            None => format!("assets/{base} {n}"),
        };
        n += 1;
    }
    fs::write(vault.join(&rel), bytes)?;
    Ok(rel)
}

fn split_ext(name: &str) -> (String, Option<String>) {
    match name.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), Some(e.to_string())),
        _ => (name.to_string(), None),
    }
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
        assert!(!is_ai_authored("---\nauthor: human\n---\ntext"));
    }

    #[test]
    fn title_falls_back_to_stem() {
        assert_eq!(title_of(Path::new("/v/ideas/second brain.md"), ""), "second brain");
    }

    #[test]
    fn title_prefers_heading() {
        assert_eq!(title_of(Path::new("/v/Untitled.md"), "# My Idea\n\nbody"), "My Idea");
    }

    #[test]
    fn title_uses_first_text_line() {
        assert_eq!(title_of(Path::new("/v/Untitled.md"), "just some text\nmore"), "just some text");
    }

    #[test]
    fn title_skips_frontmatter() {
        let md = "---\nauthor: ai\n---\n\n# Real Title\n\nbody";
        assert_eq!(title_from_content(md), Some("Real Title".to_string()));
    }

    #[test]
    fn safe_join_blocks_traversal() {
        let v = Path::new("/vault");
        assert!(safe_join(v, "../etc/passwd").is_none());
        assert!(safe_join(v, "notes/ok.md").is_some());
    }

    #[test]
    fn slugify_strips_illegal_chars() {
        assert_eq!(slugify("  My/Note:2  "), "My-Note-2");
        assert_eq!(slugify(""), "Untitled");
        assert_eq!(slugify("   "), "Untitled");
    }

    fn tmp_vault() -> PathBuf {
        let mut p = std::env::temp_dir();
        // Vary by nanos to keep parallel tests isolated without Date/Random.
        let uniq = format!(
            "magma-test-{:?}-{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        p.push(uniq);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn create_avoids_clobbering() {
        let v = tmp_vault();
        let a = create_note(&v, "Idea", "one").unwrap();
        let b = create_note(&v, "Idea", "two").unwrap();
        assert_eq!(a, "Idea.md");
        assert_eq!(b, "Idea 2.md");
        assert_eq!(read_note(&v, &b).unwrap().content, "two");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn rename_moves_content() {
        let v = tmp_vault();
        let a = create_note(&v, "Old", "body").unwrap();
        let renamed = rename_note(&v, &a, "New Name").unwrap();
        assert_eq!(renamed, "New Name.md");
        assert!(!v.join(&a).exists());
        assert_eq!(read_note(&v, &renamed).unwrap().content, "body");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn delete_removes_file() {
        let v = tmp_vault();
        let a = create_note(&v, "Doomed", "x").unwrap();
        delete_note(&v, &a).unwrap();
        assert!(!v.join(&a).exists());
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn save_asset_writes_and_dedupes() {
        let v = tmp_vault();
        let p1 = save_asset(&v, "pasted.png", &[1, 2, 3]).unwrap();
        let p2 = save_asset(&v, "pasted.png", &[4, 5, 6]).unwrap();
        assert_eq!(p1, "assets/pasted.png");
        assert_eq!(p2, "assets/pasted 2.png");
        assert!(v.join(&p2).exists());
        fs::remove_dir_all(&v).ok();
    }
}
