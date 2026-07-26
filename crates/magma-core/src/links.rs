//! Link graph: parse `[[wikilinks]]`, resolve them to notes, and build the
//! backlink index and graph that power Magma's headline feature — seeing how
//! ideas connect. Titles are matched case-insensitively, Obsidian-style, and a
//! `[[Note|alias]]` link resolves on the part before the pipe.

use crate::vault::{self, NoteMeta};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Extract wikilink targets from note content. `[[Target]]` and
/// `[[Target|alias]]` both yield `Target` (trimmed). A `#heading` or `^block`
/// suffix is stripped so the link resolves to the note.
pub fn extract_links(content: &str) -> Vec<String> {
    // Markdown serializers escape brackets, turning `[[Note]]` into
    // `\[\[Note\]\]` — which contains no `[[` at all. Notes already saved that
    // way must keep working, so unescape before scanning.
    let content = &unescape_brackets(content);
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(close) = content[i + 2..].find("]]") {
                let inner = &content[i + 2..i + 2 + close];
                let target = inner
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .split(['#', '^'])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !target.is_empty() {
                    out.push(target.to_string());
                }
                i = i + 2 + close + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Turn `\[` / `\]` back into `[` / `]`.
pub fn unescape_brackets(s: &str) -> String {
    if !s.contains("\\[") && !s.contains("\\]") {
        return s.to_string();
    }
    s.replace("\\[", "[").replace("\\]", "]")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub path: String,
    pub title: String,
    pub ai_authored: bool,
    /// Number of links touching this note (in + out) — drives node size.
    pub degree: usize,
    /// True for a link target that has no note yet. Shown as a ghost node so an
    /// isolated note reveals *why* it is isolated, instead of the link being
    /// dropped and the note looking unconnected for no visible reason.
    #[serde(default)]
    pub missing: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Build the whole-vault link graph. Edges use note paths as ids. A link whose
/// target has no note yet becomes a `missing:` ghost node rather than being
/// dropped — otherwise a note whose links all point nowhere looks unconnected
/// with nothing to explain why.
pub fn build_graph(vault: &Path, exclude: &[String]) -> std::io::Result<Graph> {
    // Folders whose notes are scaffolding, not knowledge — templates above all.
    // A template's links are placeholders; drawing them would put a node in the
    // graph for every "{{title}}" and connect nothing to anything.
    let hidden: Vec<String> = exclude
        .iter()
        .map(|f| f.trim().trim_matches('/').to_string())
        .filter(|f| !f.is_empty())
        .collect();
    let is_hidden = |path: &str| {
        hidden
            .iter()
            .any(|f| path == f || path.starts_with(&format!("{f}/")))
    };
    let notes: Vec<NoteMeta> = vault::list_notes(vault)?
        .into_iter()
        .filter(|n| !is_hidden(&n.path))
        .collect();
    let by_name = name_index(&notes);

    let mut degree: HashMap<String, usize> = HashMap::new();
    let mut edges = Vec::new();

    // Link targets with no note behind them, keyed by a synthetic id.
    let mut ghosts: HashMap<String, String> = HashMap::new();
    for note in &notes {
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        for target in extract_links(&content) {
            let dest = match by_name.get(&target.to_lowercase()) {
                Some(d) => {
                    if *d == note.path {
                        continue; // ignore self-links
                    }
                    d.clone()
                }
                None => {
                    // Unresolved: keep it, as a ghost the UI can show.
                    let id = format!("missing:{}", target.to_lowercase());
                    ghosts.entry(id.clone()).or_insert_with(|| target.clone());
                    id
                }
            };
            edges.push(GraphEdge {
                source: note.path.clone(),
                target: dest.clone(),
            });
            *degree.entry(note.path.clone()).or_default() += 1;
            *degree.entry(dest).or_default() += 1;
        }
    }

    let mut nodes: Vec<GraphNode> = notes
        .into_iter()
        .map(|n| GraphNode {
            degree: degree.get(&n.path).copied().unwrap_or(0),
            path: n.path,
            title: n.title,
            ai_authored: n.ai_authored,
            missing: false,
        })
        .collect();
    for (id, name) in ghosts {
        nodes.push(GraphNode {
            degree: degree.get(&id).copied().unwrap_or(0),
            path: id,
            title: name,
            ai_authored: false,
            missing: true,
        });
    }

    Ok(Graph { nodes, edges })
}

/// Rename a note and repoint every `[[wikilink]]` that named it.
///
/// A plain rename changes the filename, and links resolve on the filename — so
/// without this, renaming silently breaks every reference to the note. Returns
/// the new path and how many notes were rewritten.
pub fn rename_note_updating_links(
    vault: &Path,
    rel: &str,
    new_title: &str,
) -> std::io::Result<(String, usize)> {
    let old_name = note_name(rel).to_string();
    let new_rel = vault::rename_note(vault, rel, new_title)?;
    let new_name = note_name(&new_rel).to_string();
    if old_name.eq_ignore_ascii_case(&new_name) {
        return Ok((new_rel, 0));
    }

    let mut updated = 0;
    for note in vault::list_notes(vault)? {
        let path = vault.join(&note.path);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rewritten = replace_link_target(&content, &old_name, &new_name);
        if rewritten != content {
            std::fs::write(&path, rewritten)?;
            updated += 1;
        }
    }
    Ok((new_rel, updated))
}

/// Rewrite `[[old]]`, `[[old|alias]]` and `[[old#heading]]` to point at `new`,
/// leaving the alias and any anchor untouched. Matching is case-insensitive,
/// the same way link resolution is.
pub fn replace_link_target(content: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        let (before, tail) = rest.split_at(start);
        out.push_str(before);
        let inner_start = &tail[2..];
        let close = match inner_start.find("]]") {
            Some(i) => i,
            None => {
                out.push_str(tail);
                return out;
            }
        };
        let inner = &inner_start[..close];
        // Split off the alias and any #heading/^block suffix; only the target
        // part is compared and replaced.
        let (target, suffix) = match inner.find(['|', '#', '^']) {
            Some(i) => inner.split_at(i),
            None => (inner, ""),
        };
        if target.trim().eq_ignore_ascii_case(old) {
            out.push_str(&format!("[[{new}{suffix}]]"));
        } else {
            out.push_str(&format!("[[{inner}]]"));
        }
        rest = &inner_start[close + 2..];
    }
    out.push_str(rest);
    out
}

/// Notes that link *to* the given note (by its path).
pub fn backlinks(vault: &Path, target_path: &str) -> std::io::Result<Vec<NoteMeta>> {
    let notes = vault::list_notes(vault)?;
    let by_name = name_index(&notes);
    let mut out = Vec::new();
    for note in &notes {
        if note.path == target_path {
            continue;
        }
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        let links_here = extract_links(&content)
            .into_iter()
            .any(|t| by_name.get(&t.to_lowercase()).map(|p| p.as_str()) == Some(target_path));
        if links_here {
            out.push(NoteMeta {
                path: note.path.clone(),
                title: note.title.clone(),
                ai_authored: note.ai_authored,
                modified: note.modified,
            });
        }
    }
    Ok(out)
}

/// A link this note points *at* — the other half of the backlinks panel.
/// `path` is empty when the target has no note behind it yet.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingLink {
    pub name: String,
    pub path: String,
    pub title: String,
    pub missing: bool,
}

/// Every `[[wikilink]]` in a note, deduplicated, in the order they appear.
pub fn outgoing_links(vault: &Path, rel: &str) -> std::io::Result<Vec<OutgoingLink>> {
    let content = std::fs::read_to_string(vault.join(rel)).unwrap_or_default();
    let notes = vault::list_notes(vault)?;
    let by_name = name_index(&notes);
    let titles: HashMap<&str, &str> =
        notes.iter().map(|n| (n.path.as_str(), n.title.as_str())).collect();

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for name in extract_links(&content) {
        let key = name.to_lowercase();
        if !seen.insert(key.clone()) {
            continue;
        }
        match by_name.get(&key) {
            Some(path) => out.push(OutgoingLink {
                name,
                title: titles.get(path.as_str()).copied().unwrap_or("").to_string(),
                path: path.clone(),
                missing: false,
            }),
            None => out.push(OutgoingLink {
                title: name.clone(),
                name,
                path: String::new(),
                missing: true,
            }),
        }
    }
    Ok(out)
}

/// A note that names this one in plain text without linking to it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub count: usize,
}

/// Notes that say this note's name but never link it.
///
/// This is how a graph fills itself in: you wrote "Michael Klotz" in twelve
/// meeting notes long before you made a note for him, and none of them point
/// at it. Matching is case-insensitive but respects word boundaries, so
/// "Anna" does not match "Annahme".
pub fn unlinked_mentions(vault: &Path, rel: &str) -> std::io::Result<Vec<Mention>> {
    let name = note_name(rel);
    if name.chars().count() < 3 {
        // Too short to be anything but noise across a whole vault.
        return Ok(Vec::new());
    }
    let lower_name = name.to_lowercase();
    let needle: Vec<char> = lower_name.chars().collect();
    let notes = vault::list_notes(vault)?;
    let mut out = Vec::new();
    for note in &notes {
        if note.path == rel {
            continue;
        }
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        // Already linked? Then it is a backlink, not a missed mention.
        if extract_links(&content).iter().any(|l| l.to_lowercase() == lower_name) {
            continue;
        }
        let stripped = strip_links(&content);
        let count = count_words(&stripped, &needle);
        if count == 0 {
            continue;
        }
        out.push(Mention {
            path: note.path.clone(),
            title: note.title.clone(),
            snippet: snippet_around(&stripped, &stripped.to_lowercase(), &lower_name),
            count,
        });
    }
    Ok(out)
}

/// Blank out `[[...]]` spans so text inside an unrelated link is not mistaken
/// for a plain mention.
fn strip_links(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        match tail.find("]]") {
            Some(end) => {
                // Keep the length roughly stable so snippets still read well.
                out.push_str(&" ".repeat(end + 4));
                rest = &tail[end + 2..];
            }
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Count whole-word, case-insensitive occurrences of `needle` in `haystack`.
fn count_words(haystack: &str, needle: &[char]) -> usize {
    let mut count = 0;
    let mut i = 0;
    let mut prev: Option<char> = None;
    while i < haystack.len() {
        if let Some(len) = match_ci(&haystack[i..], needle) {
            let end = i + len;
            let before_ok = prev.map(|c| !is_word_char(c)).unwrap_or(true);
            let after_ok =
                haystack[end..].chars().next().map(|c| !is_word_char(c)).unwrap_or(true);
            if before_ok && after_ok {
                count += 1;
                prev = haystack[i..end].chars().last();
                i = end;
                continue;
            }
        }
        let ch = haystack[i..].chars().next().unwrap();
        prev = Some(ch);
        i += ch.len_utf8();
    }
    count
}

/// Turn plain-text mentions of `name` into `[[name]]` inside one note.
/// Returns how many were linked. Only whole-word matches outside existing
/// links are touched.
pub fn link_mentions(vault: &Path, rel: &str, name: &str) -> std::io::Result<usize> {
    let content = std::fs::read_to_string(vault.join(rel))?;
    let (out, n) = link_mentions_in(&content, name);
    if n > 0 {
        vault::write_note(vault, rel, &out)?;
    }
    Ok(n)
}

/// The text half of `link_mentions`, kept separate so it can be tested without
/// a vault on disk.
pub fn link_mentions_in(content: &str, name: &str) -> (String, usize) {
    let needle: Vec<char> = name.chars().flat_map(|c| c.to_lowercase()).collect();
    if needle.is_empty() {
        return (content.to_string(), 0);
    }
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut count = 0;
    let mut prev: Option<char> = None;
    while i < content.len() {
        // Step over an existing [[link]] untouched.
        if content[i..].starts_with("[[") {
            match content[i + 2..].find("]]") {
                Some(end) => {
                    let stop = i + 2 + end + 2;
                    out.push_str(&content[i..stop]);
                    prev = content[i..stop].chars().last();
                    i = stop;
                    continue;
                }
                None => break,
            }
        }
        // Match case-insensitively without ever building a lowercased copy of
        // the note: its byte offsets would not line up with the original
        // (lowercasing can change a character's byte length), and slicing the
        // original with them is how the search used to panic mid-umlaut.
        if let Some(len) = match_ci(&content[i..], &needle) {
            let end = i + len;
            let before_ok = prev.map(|c| !is_word_char(c)).unwrap_or(true);
            let after_ok = content[end..].chars().next().map(|c| !is_word_char(c)).unwrap_or(true);
            if before_ok && after_ok {
                // Keep the note's own spelling inside the link.
                out.push_str("[[");
                out.push_str(&content[i..end]);
                out.push_str("]]");
                prev = content[i..end].chars().last();
                i = end;
                count += 1;
                continue;
            }
        }
        let ch = content[i..].chars().next().unwrap();
        out.push(ch);
        prev = Some(ch);
        i += ch.len_utf8();
    }
    out.push_str(&content[i.min(content.len())..]);
    (out, count)
}

/// If `hay` starts with `needle` (already lowercased, char by char), how many
/// bytes of `hay` that match takes up.
fn match_ci(hay: &str, needle: &[char]) -> Option<usize> {
    let mut chars = hay.char_indices();
    let mut consumed = 0;
    for &want in needle {
        let (idx, got) = chars.next()?;
        let mut lower = got.to_lowercase();
        if lower.next()? != want || lower.next().is_some() {
            return None;
        }
        consumed = idx + got.len_utf8();
    }
    Some(consumed)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
}

/// Case-insensitive full-text search across titles and bodies. Simple and
/// synchronous — fine at personal-vault scale; SQLite FTS5 is the planned
/// upgrade for very large vaults.
pub fn search(vault: &Path, query: &str) -> std::io::Result<Vec<SearchHit>> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let notes = vault::list_notes(vault)?;
    let mut hits = Vec::new();
    for note in notes {
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        let title_match = note.title.to_lowercase().contains(&q);
        let body_lower = content.to_lowercase();
        if title_match || body_lower.contains(&q) {
            hits.push(SearchHit {
                snippet: snippet_around(&content, &body_lower, &q),
                path: note.path,
                title: note.title,
            });
        }
    }
    Ok(hits)
}

fn snippet_around(content: &str, body_lower: &str, q: &str) -> String {
    // Everything here is char-based on purpose. The old version sliced by byte
    // offsets taken from the *lowercased* copy, and `to_lowercase` can change a
    // string's byte length (\u{130} becomes two chars, \u{1E9E} shrinks), so those
    // offsets could land inside a multi-byte character — and slicing a char in
    // half panics, which takes the whole app down mid-search.
    const BEFORE: usize = 30;
    const AFTER: usize = 40;
    match body_lower.find(q) {
        Some(idx) => {
            let hit_char = body_lower[..idx].chars().count();
            let from = hit_char.saturating_sub(BEFORE);
            let raw: String = content
                .chars()
                .skip(from)
                .take(BEFORE + q.chars().count() + AFTER)
                .collect();
            format!("\u{2026}{}\u{2026}", raw.replace('\n', " ").trim())
        }
        None => content.lines().next().unwrap_or("").to_string(),
    }
}

/// The note "name" used as a `[[wikilink]]` target: the filename stem, matching
/// Obsidian. This is independent of the display title (which comes from the
/// note's first heading), so links stay stable even if the heading changes.
pub fn note_name(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.strip_suffix(".md").unwrap_or(file)
}

/// Map lowercased note name (filename stem) -> note path.
fn name_index(notes: &[NoteMeta]) -> HashMap<String, String> {
    notes
        .iter()
        .map(|n| (note_name(&n.path).to_lowercase(), n.path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_vault() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "magma-links-{:?}-{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn extracts_plain_and_aliased_links() {
        let md = "See [[Alpha]] and [[Beta|the beta]] plus [[Gamma#section]].";
        assert_eq!(extract_links(md), vec!["Alpha", "Beta", "Gamma"]);
    }

    #[test]
    fn ignores_unclosed_brackets() {
        assert!(extract_links("a [[ b").is_empty());
    }

    #[test]
    fn graph_resolves_edges_case_insensitively() {
        let v = tmp_vault();
        vault::write_note(&v, "Alpha.md", "links to [[beta]]").unwrap();
        vault::write_note(&v, "Beta.md", "no links").unwrap();
        let g = build_graph(&v, &[]).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].source, "Alpha.md");
        assert_eq!(g.edges[0].target, "Beta.md");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn extracts_links_even_when_the_brackets_were_escaped() {
        // Exactly what the editor used to save, which made links vanish.
        let md = r"Sohn von \[\[Profil Alex Januschewsky|Alex\]\] und \[\[Birgit Januschewsky\]\].";
        assert_eq!(
            extract_links(md),
            vec!["Profil Alex Januschewsky", "Birgit Januschewsky"]
        );
    }

    #[test]
    fn unresolved_links_appear_as_ghost_nodes() {
        let v = tmp_vault();
        // Mirrors a family note whose links name notes that don't exist.
        vault::write_note(&v, "Familie/Oma.md", "# Oma\n\n[[Opa]] und [[Mama]]").unwrap();
        let g = build_graph(&v, &[]).unwrap();

        let oma = g.nodes.iter().find(|n| n.path == "Familie/Oma.md").unwrap();
        assert_eq!(oma.degree, 2, "the note is no longer isolated");

        let ghosts: Vec<_> = g.nodes.iter().filter(|n| n.missing).collect();
        assert_eq!(ghosts.len(), 2);
        let titles: Vec<_> = ghosts.iter().map(|n| n.title.as_str()).collect();
        assert!(titles.contains(&"Opa"));
        assert!(titles.contains(&"Mama"));
        assert_eq!(g.edges.len(), 2);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn backlinks_finds_referrers() {
        let v = tmp_vault();
        vault::write_note(&v, "Hub.md", "# Hub\n\nhub").unwrap();
        vault::write_note(&v, "A.md", "# A\n\nsee [[Hub]]").unwrap();
        vault::write_note(&v, "B.md", "# B\n\nalso [[hub]] here").unwrap();
        let back = backlinks(&v, "Hub.md").unwrap();
        let titles: Vec<_> = back.iter().map(|n| n.title.clone()).collect();
        assert_eq!(back.len(), 2);
        assert!(titles.contains(&"A".to_string()));
        assert!(titles.contains(&"B".to_string()));
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn rename_repoints_links_and_keeps_aliases() {
        let v = tmp_vault();
        vault::write_note(&v, "Michael Klotz.md", "# Michael Klotz").unwrap();
        vault::write_note(
            &v,
            "Notiz.md",
            "Siehe [[Michael Klotz]], [[michael klotz|den Kunden]] und [[Michael Klotz#Termine]].\nAber [[Andere]] bleibt.",
        )
        .unwrap();

        let (new_rel, updated) =
            rename_note_updating_links(&v, "Michael Klotz.md", "Michael Klotz GmbH").unwrap();
        assert_eq!(new_rel, "Michael Klotz GmbH.md");
        assert_eq!(updated, 1);

        let body = std::fs::read_to_string(v.join("Notiz.md")).unwrap();
        assert!(body.contains("[[Michael Klotz GmbH]]"));
        assert!(body.contains("[[Michael Klotz GmbH|den Kunden]]"), "alias kept: {body}");
        assert!(body.contains("[[Michael Klotz GmbH#Termine]]"), "anchor kept: {body}");
        assert!(body.contains("[[Andere]]"), "unrelated links untouched");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn snippets_never_split_a_character() {
        // Text whose lowercase differs in byte length from the original, so
        // byte offsets taken from it do not line up with the source.
        let samples = [
            "İstanbul: Größe und Qualität für Bäcker, süße Öfen überall",
            "STRAẞE und Qualität — Maße, Öfen, Bäcker",
            "Ää Öö Üü ß İ Qualität ẞ Maße Größe",
        ];
        for content in samples {
            let lower = content.to_lowercase();
            for q in ["qualität", "größe", "maße", "öfen"] {
                // Must not panic, whatever the offsets do.
                let s = snippet_around(content, &lower, q);
                assert!(s.is_char_boundary(0));
            }
        }
    }

    #[test]
    fn excluded_folders_stay_out_of_the_graph() {
        let v = tmp_vault();
        vault::write_note(&v, "Echt.md", "# Echt\n\nVerlinkt [[Auch echt]].").unwrap();
        vault::write_note(&v, "Auch echt.md", "# Auch echt").unwrap();
        vault::write_note(&v, "Templates/Kunde.md", "# {{title}}\n\n[[Platzhalter]]").unwrap();

        let g = build_graph(&v, &["Templates".to_string()]).unwrap();
        let paths: Vec<&str> = g.nodes.iter().map(|n| n.path.as_str()).collect();
        assert!(!paths.iter().any(|p| p.starts_with("Templates")), "got: {paths:?}");
        // ...and the placeholder link inside the template makes no ghost node.
        assert!(!paths.iter().any(|p| p.contains("platzhalter")), "got: {paths:?}");
        assert_eq!(g.nodes.len(), 2);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn finds_mentions_nobody_linked() {
        let v = tmp_vault();
        vault::write_note(&v, "Michael Klotz.md", "# Michael Klotz").unwrap();
        vault::write_note(&v, "Meeting.md", "Termin mit Michael Klotz am Montag.").unwrap();
        vault::write_note(&v, "Verlinkt.md", "Siehe [[Michael Klotz]].").unwrap();
        vault::write_note(&v, "Fremd.md", "Michael Klotzmann ist jemand anderes.").unwrap();

        let hits = unlinked_mentions(&v, "Michael Klotz.md").unwrap();
        let paths: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths, vec!["Meeting.md"], "got: {paths:?}");
        assert_eq!(hits[0].count, 1);
        assert!(hits[0].snippet.contains("Montag"), "snippet: {}", hits[0].snippet);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn linking_a_mention_leaves_existing_links_and_partial_words_alone() {
        let (out, n) = link_mentions_in(
            "Michael Klotz kam. [[Michael Klotz]] steht schon. Michael Klotzmann nicht.",
            "Michael Klotz",
        );
        assert_eq!(n, 1, "only the bare mention: {out}");
        assert_eq!(
            out,
            "[[Michael Klotz]] kam. [[Michael Klotz]] steht schon. Michael Klotzmann nicht."
        );
    }

    #[test]
    fn mentions_survive_umlauts_at_any_offset() {
        // The lowercase form of this text has different byte offsets than the
        // original — the shape that used to panic the search.
        let text = "Größe und Qualität: MÜLLER GMBH liefert. Straße 5.";
        let (out, n) = link_mentions_in(text, "Müller GmbH");
        assert_eq!(n, 1, "got: {out}");
        assert!(out.contains("[[MÜLLER GMBH]]"), "keeps the note's spelling: {out}");
    }

    #[test]
    fn outgoing_links_separate_real_targets_from_missing_ones() {
        let v = tmp_vault();
        vault::write_note(&v, "Ziel.md", "# Das Ziel").unwrap();
        vault::write_note(&v, "Quelle.md", "[[Ziel]], nochmal [[Ziel]] und [[Nirgendwo]].")
            .unwrap();
        let out = outgoing_links(&v, "Quelle.md").unwrap();
        assert_eq!(out.len(), 2, "duplicates collapse");
        assert_eq!(out[0].path, "Ziel.md");
        assert_eq!(out[0].title, "Das Ziel");
        assert!(!out[0].missing);
        assert!(out[1].missing && out[1].path.is_empty());
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn replace_previews_before_it_writes() {
        let v = tmp_vault();
        vault::write_note(&v, "A.md", "Von [[Profil Alex Januschewsky]] und Profil Alex Januschewsky.").unwrap();
        vault::write_note(&v, "B.md", "Nur Profil Alex Januschewsky hier.").unwrap();
        vault::write_note(&v, "C.md", "Nichts davon.").unwrap();

        let preview =
            replace_in_vault(&v, "Profil Alex Januschewsky", "Alex Januschewsky", true, false)
                .unwrap();
        assert_eq!(preview.total, 3);
        assert_eq!(preview.hits.len(), 2, "C.md is untouched");
        assert!(!preview.applied);
        assert!(
            std::fs::read_to_string(v.join("A.md")).unwrap().contains("Profil Alex"),
            "a preview must not write"
        );

        let done =
            replace_in_vault(&v, "Profil Alex Januschewsky", "Alex Januschewsky", false, false)
                .unwrap();
        assert_eq!(done.total, 3);
        assert!(done.applied);
        let a = std::fs::read_to_string(v.join("A.md")).unwrap();
        assert!(!a.contains("Profil"), "got: {a}");
        assert!(a.contains("[[Alex Januschewsky]]"), "links rewritten too: {a}");
        assert_eq!(std::fs::read_to_string(v.join("C.md")).unwrap(), "Nichts davon.");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn replace_moves_wikilink_targets_with_the_text() {
        let v = tmp_vault();
        // The note the links point at carries the term in its *filename* —
        // rewriting only the text would aim every link at nothing.
        vault::write_note(
            &v,
            "Profil Alex Januschewsky.md",
            "# Profil Alex Januschewsky\n\nÜber mich.",
        )
        .unwrap();
        vault::write_note(
            &v,
            "Blog.md",
            "Von [[Profil Alex Januschewsky]], siehe auch [[Profil Alex Januschewsky|den Autor]].",
        )
        .unwrap();

        let preview =
            replace_in_vault(&v, "Profil Alex Januschewsky", "Alex Januschewsky", true, true)
                .unwrap();
        assert_eq!(preview.renames.len(), 1);
        assert_eq!(preview.renames[0].to, "Alex Januschewsky");
        assert!(v.join("Profil Alex Januschewsky.md").exists(), "preview writes nothing");

        replace_in_vault(&v, "Profil Alex Januschewsky", "Alex Januschewsky", false, true).unwrap();

        assert!(!v.join("Profil Alex Januschewsky.md").exists(), "note renamed");
        let target = std::fs::read_to_string(v.join("Alex Januschewsky.md")).unwrap();
        assert_eq!(target, "# Alex Januschewsky\n\nÜber mich.", "heading rewritten too");

        let blog = std::fs::read_to_string(v.join("Blog.md")).unwrap();
        assert!(blog.contains("[[Alex Januschewsky]]"), "got: {blog}");
        assert!(blog.contains("[[Alex Januschewsky|den Autor]]"), "alias kept: {blog}");
        // The whole point: every link still resolves to a note that exists.
        for link in extract_links(&blog) {
            assert!(
                v.join(format!("{link}.md")).exists(),
                "link [[{link}]] points at nothing"
            );
        }
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn search_matches_title_and_body() {
        let v = tmp_vault();
        vault::write_note(&v, "Recipes.md", "# Recipes\n\nhow to bake sourdough bread").unwrap();
        vault::write_note(&v, "Other.md", "# Other\n\nunrelated").unwrap();
        let hits = search(&v, "sourdough").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Recipes");
        assert!(hits[0].snippet.to_lowercase().contains("sourdough"));
        fs::remove_dir_all(&v).ok();
    }
}

/// One note affected by a replace, and how many occurrences it holds.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceHit {
    pub path: String,
    pub title: String,
    pub count: usize,
}

/// A note whose own name carries the search term, so the note itself is
/// renamed instead of being left behind as the dead target of rewritten links.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceRename {
    pub path: String,
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceReport {
    pub hits: Vec<ReplaceHit>,
    /// Notes that get renamed along with the text (empty when off).
    pub renames: Vec<ReplaceRename>,
    pub total: usize,
    /// False when this was only a preview and nothing was written.
    pub applied: bool,
}

/// Replace `find` with `replace` across every note in the vault.
///
/// `dry_run` reports what *would* change without touching anything — a bulk
/// rewrite over hundreds of notes is not something to fire blind. Matching is
/// exact and case-sensitive, so replacing "Profil Alex" cannot also hit
/// "profil alex" in a URL by accident.
///
/// Wikilinks resolve on the *filename*, so a plain text pass would rewrite
/// `[[Profil Alex]]` to `[[Alex]]` and leave every one of them pointing at a
/// note that does not exist. With `rename_notes`, a note whose own name holds
/// the term is renamed first (repointing all links to it), and only then does
/// the text pass run — links and their targets move together.
pub fn replace_in_vault(
    vault: &Path,
    find: &str,
    replace: &str,
    dry_run: bool,
    rename_notes: bool,
) -> std::io::Result<ReplaceReport> {
    if find.is_empty() {
        return Ok(ReplaceReport {
            hits: Vec::new(),
            renames: Vec::new(),
            total: 0,
            applied: false,
        });
    }

    // Scan first, so the preview and the applied run report the same thing:
    // renaming rewrites links, which would otherwise change the text counts
    // underneath us.
    let mut hits = Vec::new();
    let mut renames = Vec::new();
    let mut total = 0;
    for note in vault::list_notes(vault)? {
        if rename_notes {
            let name = note_name(&note.path);
            if name.contains(find) {
                let renamed = name.replace(find, replace);
                if !renamed.trim().is_empty() {
                    renames.push(ReplaceRename {
                        path: note.path.clone(),
                        from: name.to_string(),
                        to: renamed,
                    });
                }
            }
        }
        let content = match std::fs::read_to_string(vault.join(&note.path)) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let count = content.matches(find).count();
        if count == 0 {
            continue;
        }
        total += count;
        hits.push(ReplaceHit { path: note.path.clone(), title: note.title.clone(), count });
    }

    if dry_run {
        return Ok(ReplaceReport { hits, renames, total, applied: false });
    }

    for rename in &renames {
        rename_note_updating_links(vault, &rename.path, &rename.to)?;
    }
    // Re-list: the renames moved files, so the paths from the scan are stale.
    for note in vault::list_notes(vault)? {
        let path = vault.join(&note.path);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.contains(find) {
            std::fs::write(&path, content.replace(find, replace))?;
        }
    }
    Ok(ReplaceReport { hits, renames, total, applied: true })
}
