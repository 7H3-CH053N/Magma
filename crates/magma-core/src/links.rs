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

/// Build the whole-vault link graph. Edges use note paths as ids; unresolved
/// links (to notes that don't exist yet) are dropped from the graph.
pub fn build_graph(vault: &Path) -> std::io::Result<Graph> {
    let notes = vault::list_notes(vault)?;
    let by_title = title_index(&notes);

    let mut degree: HashMap<String, usize> = HashMap::new();
    let mut edges = Vec::new();

    for note in &notes {
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        for target in extract_links(&content) {
            if let Some(dest) = by_title.get(&target.to_lowercase()) {
                if *dest == note.path {
                    continue; // ignore self-links
                }
                edges.push(GraphEdge {
                    source: note.path.clone(),
                    target: dest.clone(),
                });
                *degree.entry(note.path.clone()).or_default() += 1;
                *degree.entry(dest.clone()).or_default() += 1;
            }
        }
    }

    let nodes = notes
        .into_iter()
        .map(|n| GraphNode {
            degree: degree.get(&n.path).copied().unwrap_or(0),
            path: n.path,
            title: n.title,
            ai_authored: n.ai_authored,
        })
        .collect();

    Ok(Graph { nodes, edges })
}

/// Notes that link *to* the given note (by its path).
pub fn backlinks(vault: &Path, target_path: &str) -> std::io::Result<Vec<NoteMeta>> {
    let notes = vault::list_notes(vault)?;
    let by_title = title_index(&notes);
    let mut out = Vec::new();
    for note in &notes {
        if note.path == target_path {
            continue;
        }
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        let links_here = extract_links(&content)
            .into_iter()
            .any(|t| by_title.get(&t.to_lowercase()).map(|p| p.as_str()) == Some(target_path));
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

/// Map lowercased title -> note path. Later notes win on collision, which is
/// acceptable for now (Obsidian warns on duplicate titles too).
fn title_index(notes: &[NoteMeta]) -> HashMap<String, String> {
    notes
        .iter()
        .map(|n| (n.title.to_lowercase(), n.path.clone()))
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
    fn backlinks_finds_referrers() {
        let v = tmp_vault();
        vault::write_note(&v, "Hub.md", "hub").unwrap();
        vault::write_note(&v, "A.md", "see [[Hub]]").unwrap();
        vault::write_note(&v, "B.md", "also [[hub]] here").unwrap();
        let back = backlinks(&v, "Hub.md").unwrap();
        let titles: Vec<_> = back.iter().map(|n| n.title.clone()).collect();
        assert_eq!(back.len(), 2);
        assert!(titles.contains(&"A".to_string()));
        assert!(titles.contains(&"B".to_string()));
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn search_matches_title_and_body() {
        let v = tmp_vault();
        vault::write_note(&v, "Recipes.md", "how to bake sourdough bread").unwrap();
        vault::write_note(&v, "Other.md", "unrelated").unwrap();
        let hits = search(&v, "sourdough").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Recipes");
        assert!(hits[0].snippet.to_lowercase().contains("sourdough"));
        fs::remove_dir_all(&v).ok();
    }
}
