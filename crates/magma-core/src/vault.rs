//! Vault access: the file system is the source of truth. A vault is just a
//! folder of markdown files, so nothing here locks a user in — the same files
//! open in any editor, including Obsidian.

use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    /// Path relative to the vault root, using forward slashes.
    pub path: String,
    pub title: String,
    /// True when frontmatter declares `author: ai` — a note an LLM wrote.
    pub ai_authored: bool,
    /// Last modified, in milliseconds since the epoch. 0 when unknown.
    /// Lets the UI answer "what changed lately?" without reading every file.
    pub modified: u64,
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
                modified: modified_ms(&entry),
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

fn modified_ms(entry: &fs::DirEntry) -> u64 {
    entry
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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
pub(crate) fn title_of(path: &Path, content: &str) -> String {
    if let Some(t) = title_from_content(content) {
        // A template's heading is often nothing but a placeholder ("{{title}}").
        // Showing that in the sidebar names the note after a hole in itself —
        // its filename is the real name.
        let bare_placeholder = t.starts_with("{{") && t.ends_with("}}");
        if !bare_placeholder {
            return t;
        }
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
    let rel_path = Path::new(rel);
    if rel_path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) || rel.split(['/', '\\']).any(|c| c == "..")
    {
        return None;
    }
    Some(vault.join(rel_path))
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

/// Open the note at `folder/<title>.md` or create it with `content`.
///
/// Unlike `create_note_in`, this never appends " 2" — a daily note for today
/// must be *the* note for today, not a second one every time the button is
/// pressed. Returns the path and whether it had to be created.
pub fn open_or_create(
    vault: &Path,
    folder: &str,
    title: &str,
    content: &str,
) -> std::io::Result<(String, bool)> {
    let stem = slugify(title);
    let dir = sanitize_folder(folder);
    let rel = if dir.is_empty() {
        format!("{stem}.md")
    } else {
        format!("{dir}/{stem}.md")
    };
    if vault.join(&rel).exists() {
        return Ok((rel, false));
    }
    write_note(vault, &rel, content)?;
    Ok((rel, true))
}

/// Add text to the end of a note, starting a new paragraph.
///
/// This is what quick capture writes into: a thought lands at the bottom of
/// today's note without opening it, so nothing already in the note can be lost
/// by a stale editor buffer overwriting the file.
pub fn append_note(vault: &Path, rel: &str, text: &str) -> std::io::Result<()> {
    let existing = fs::read_to_string(vault.join(rel)).unwrap_or_default();
    let mut out = existing.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(text.trim());
    out.push('\n');
    write_note(vault, rel, &out)
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

/// Delete a folder and everything inside it (recursive). `folder` is a
/// vault-relative path; the vault root ("") and any `..` traversal are refused.
/// Segment names are preserved as-is (not re-slugified) so folders created by
/// the importer, whose names keep the user's original casing, match on disk.
pub fn delete_folder(vault: &Path, folder: &str) -> std::io::Result<()> {
    let cleaned = folder
        .split(['/', '\\'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "..")
        .collect::<Vec<_>>()
        .join("/");
    if cleaned.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "refusing to delete the vault root",
        ));
    }
    let target = vault.join(&cleaned);
    if !target.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "folder not found",
        ));
    }
    fs::remove_dir_all(target)
}

/// Move a whole folder (with everything in it) into `into` ("" = vault root).
///
/// Wikilinks resolve on filenames, so nothing needs rewriting — the notes keep
/// their names, only their path changes. Returns the folder's new path.
pub fn move_folder(vault: &Path, folder: &str, into: &str) -> std::io::Result<String> {
    let bad = |msg: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, msg.to_string());
    // Segments are kept as-is (not re-slugified) so folders created elsewhere,
    // which keep the user's own casing and spacing, still match on disk.
    let clean = |p: &str| {
        p.split(['/', '\\'])
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "..")
            .collect::<Vec<_>>()
            .join("/")
    };
    let from = clean(folder);
    let into = clean(into);
    if from.is_empty() {
        return Err(bad("refusing to move the vault root"));
    }
    if !vault.join(&from).is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "folder not found",
        ));
    }
    // Dropping a folder onto itself or onto one of its own children would make
    // the tree swallow itself; the filesystem would either fail cryptically or
    // strand the contents.
    if into == from || into.starts_with(&format!("{from}/")) {
        return Err(bad("a folder cannot be moved inside itself"));
    }
    let name = from.rsplit('/').next().unwrap_or(&from).to_string();
    let target = if into.is_empty() {
        name
    } else {
        format!("{into}/{name}")
    };
    if target == from {
        return Ok(from); // already there
    }
    if vault.join(&target).exists() {
        return Err(bad("a folder with that name already exists there"));
    }
    if let Some(parent) = vault.join(&target).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(vault.join(&from), vault.join(&target))?;
    Ok(target)
}

/// Move a note into `folder` (vault-relative; "" means the root), keeping its
/// filename. Wikilinks still resolve afterwards because they match on the
/// filename, not the folder. Returns the new vault-relative path.
pub fn move_note(vault: &Path, rel: &str, folder: &str) -> std::io::Result<String> {
    let filename = rel.rsplit(['/', '\\']).next().unwrap_or(rel);
    let (base, ext) = split_ext(filename);
    let dir = sanitize_folder(folder);
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut new_rel = format!("{prefix}{filename}");
    let mut n = 2;
    let old = vault.join(rel);
    while vault.join(&new_rel).exists() && vault.join(&new_rel) != old {
        new_rel = match &ext {
            Some(e) => format!("{prefix}{base} {n}.{e}"),
            None => format!("{prefix}{base} {n}"),
        };
        n += 1;
    }
    if let Some(parent) = vault.join(&new_rel).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(old, vault.join(&new_rel))?;
    Ok(new_rel)
}

/// Create an (empty) folder in the vault. Returns its sanitized relative path.
pub fn create_folder(vault: &Path, name: &str) -> std::io::Result<String> {
    let dir = sanitize_folder(name);
    if dir.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty folder name",
        ));
    }
    fs::create_dir_all(vault.join(&dir))?;
    Ok(dir)
}

/// List every subfolder in the vault (relative paths, forward slashes),
/// skipping hidden dirs and the `assets` image folder.
pub fn list_folders(vault: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    collect_dirs(vault, vault, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_dirs(root: &Path, dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "assets" {
            continue;
        }
        out.push(rel_path(root, &path));
        collect_dirs(root, &path, out)?;
    }
    Ok(())
}

/// Save pasted image bytes into the vault's `assets/` folder under a
/// caller-provided name, returning the vault-relative path for embedding.
pub fn save_asset(vault: &Path, file_name: &str, bytes: &[u8]) -> std::io::Result<String> {
    if !is_plain_file_name(file_name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid asset filename",
        ));
    }
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

fn is_plain_file_name(file_name: &str) -> bool {
    let trimmed = file_name.trim();
    !trimmed.is_empty()
        && trimmed == file_name
        && !file_name.chars().any(|c| c == '/' || c == '\\')
        && !Path::new(file_name).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        && file_name != ".."
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
    fn placeholder_heading_falls_back_to_the_filename() {
        let v = tmp_vault();
        write_note(&v, "Kunde.md", "# {{title}}\n\nAngelegt am {{date}}.").unwrap();
        let notes = list_notes(&v).unwrap();
        assert_eq!(notes[0].title, "Kunde");
        fs::remove_dir_all(&v).ok();
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
        assert!(safe_join(v, "/etc/passwd").is_none());
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
    fn move_note_into_folder_keeps_filename() {
        let v = tmp_vault();
        create_note(&v, "Tech-Stack", "# Tech-Stack").unwrap();
        let moved = move_note(&v, "Tech-Stack.md", "Profil Alex").unwrap();
        assert_eq!(moved, "Profil Alex/Tech-Stack.md");
        assert!(v.join("Profil Alex/Tech-Stack.md").exists());
        assert!(!v.join("Tech-Stack.md").exists());
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn create_and_list_folders() {
        let v = tmp_vault();
        create_folder(&v, "Ideas").unwrap();
        create_folder(&v, "Work/Clients").unwrap();
        let folders = list_folders(&v).unwrap();
        assert!(folders.contains(&"Ideas".to_string()));
        assert!(folders.contains(&"Work".to_string()));
        assert!(folders.contains(&"Work/Clients".to_string()));
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn move_folder_nests_and_refuses_to_swallow_itself() {
        let v = tmp_vault();
        create_folder(&v, "Kunden").unwrap();
        create_folder(&v, "Archiv").unwrap();
        write_note(&v, "Kunden/Michael Klotz.md", "# Michael Klotz").unwrap();

        let moved = move_folder(&v, "Kunden", "Archiv").unwrap();
        assert_eq!(moved, "Archiv/Kunden");
        assert!(v.join("Archiv/Kunden/Michael Klotz.md").is_file(), "notes came along");
        assert!(!v.join("Kunden").exists());

        // Into itself, into a child, and the vault root are all refused.
        assert!(move_folder(&v, "Archiv", "Archiv").is_err());
        assert!(move_folder(&v, "Archiv", "Archiv/Kunden").is_err());
        assert!(move_folder(&v, "", "Archiv").is_err());
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn move_folder_refuses_to_clobber_a_name_already_there() {
        let v = tmp_vault();
        create_folder(&v, "A/Notizen").unwrap();
        create_folder(&v, "Notizen").unwrap();
        write_note(&v, "Notizen/x.md", "x").unwrap();
        assert!(move_folder(&v, "Notizen", "A").is_err(), "would have merged silently");
        assert!(v.join("Notizen/x.md").is_file(), "nothing was moved");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn delete_folder_removes_recursively_and_guards_root() {
        let v = tmp_vault();
        write_note(&v, "Blog/post.md", "# Post").unwrap();
        write_note(&v, "Blog/nested/deep.md", "# Deep").unwrap();
        assert!(v.join("Blog").is_dir());
        delete_folder(&v, "Blog").unwrap();
        assert!(!v.join("Blog").exists());
        // The vault root and traversal are refused.
        assert!(delete_folder(&v, "").is_err());
        assert!(delete_folder(&v, "  /  ").is_err());
        assert!(delete_folder(&v, "..").is_err());
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

    #[test]
    fn save_asset_rejects_path_segments() {
        let v = tmp_vault();
        assert!(save_asset(&v, "../outside.png", b"x").is_err());
        assert!(save_asset(&v, "/tmp/outside.png", b"x").is_err());
        assert!(save_asset(&v, "nested/image.png", b"x").is_err());
        assert!(save_asset(&v, "nested\\image.png", b"x").is_err());
        fs::remove_dir_all(&v).ok();
    }
}
