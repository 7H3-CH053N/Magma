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
pub fn build_graph(vault: &Path) -> std::io::Result<Graph> {
    let notes = vault::list_notes(vault)?;
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
            });
        }
    }
    Ok(out)
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
    match body_lower.find(q) {
        Some(idx) => {
            let start = content[..idx].char_indices().rev().nth(30).map(|(i, _)| i).unwrap_or(0);
            let end = (idx + q.len() + 40).min(content.len());
            let end = content[..end]
                .char_indices()
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(content.len());
            let raw = content[start..end].replace('\n', " ");
            format!("…{}…", raw.trim())
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
        let g = build_graph(&v).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].source, "Alpha.md");
        assert_eq!(g.edges[0].target, "Beta.md");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn unresolved_links_appear_as_ghost_nodes() {
        let v = tmp_vault();
        // Mirrors a family note whose links name notes that don't exist.
        vault::write_note(&v, "Familie/Oma.md", "# Oma\n\n[[Opa]] und [[Mama]]").unwrap();
        let g = build_graph(&v).unwrap();

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
