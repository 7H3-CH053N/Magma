//! AI co-authoring: the logic that lets an LLM add notes that are correctly and
//! logically linked, instead of dropping orphans. Two ideas do the work:
//!
//!   * `find_link_candidates` — given the text the model is about to write, surface
//!     the most related existing notes so it knows what to link to.
//!   * `validate_links` — after writing, check every `[[wikilink]]` against the
//!     vault and report broken ones with close-match suggestions.
//!
//! Candidate ranking is lexical (shared significant terms) for now; the planned
//! upgrade is local embeddings (fastembed) for semantic matching. The API shape
//! stays the same, so that swap is internal.

use crate::links::extract_links;
use crate::vault::{self, NoteMeta};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCandidate {
    pub path: String,
    /// Display title (first heading of the note).
    pub title: String,
    /// The `[[wikilink]]` target to use when linking this note (filename stem).
    pub name: String,
    /// Relevance score in [0,1]; higher is more related.
    pub score: f32,
    pub snippet: String,
}

/// Rank existing notes by lexical relevance to `text`. Use this before writing a
/// note to decide which notes to link.
pub fn find_link_candidates(
    vault: &Path,
    text: &str,
    limit: usize,
) -> std::io::Result<Vec<LinkCandidate>> {
    let query_terms = significant_terms(text);
    if query_terms.is_empty() {
        return Ok(Vec::new());
    }
    let notes = vault::list_notes(vault)?;
    let mut scored: Vec<LinkCandidate> = Vec::new();
    for note in notes {
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        let mut haystack = note.title.clone();
        haystack.push(' ');
        haystack.push_str(&content);
        let note_terms = significant_terms(&haystack);
        if note_terms.is_empty() {
            continue;
        }
        let overlap = query_terms.intersection(&note_terms).count();
        if overlap == 0 {
            continue;
        }
        // Jaccard-style score, biased toward the query so short notes still rank.
        let score = overlap as f32 / query_terms.len() as f32;
        scored.push(LinkCandidate {
            snippet: first_line(&content),
            name: crate::links::note_name(&note.path).to_string(),
            path: note.path,
            title: note.title,
            score,
        });
    }
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenLink {
    pub target: String,
    /// Close existing titles the model likely meant.
    pub suggestions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCheck {
    pub resolved: Vec<String>,
    pub broken: Vec<BrokenLink>,
}

/// Check every `[[wikilink]]` in `content` against the vault. Broken links come
/// back with suggestions so the caller (or the LLM) can fix them rather than
/// leaving a dead link.
pub fn validate_links(vault: &Path, content: &str) -> std::io::Result<LinkCheck> {
    let notes = vault::list_notes(vault)?;
    // Wikilinks target the note name (filename stem), matching Obsidian.
    let names: Vec<String> = notes
        .iter()
        .map(|n| crate::links::note_name(&n.path).to_string())
        .collect();
    let lower: HashSet<String> = names.iter().map(|t| t.to_lowercase()).collect();

    let mut resolved = Vec::new();
    let mut broken = Vec::new();
    for target in extract_links(content) {
        if lower.contains(&target.to_lowercase()) {
            resolved.push(target);
        } else {
            broken.push(BrokenLink {
                suggestions: closest_titles(&target, &names, 3),
                target,
            });
        }
    }
    Ok(LinkCheck { resolved, broken })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiWriteResult {
    pub path: String,
    pub link_check: LinkCheck,
}

/// Create a note authored by the AI: stamp `author: ai` into frontmatter, write
/// it, and return the link check so the caller sees any broken links.
pub fn ai_create_note(
    vault: &Path,
    title: &str,
    content: &str,
) -> std::io::Result<AiWriteResult> {
    let stamped = stamp_ai_author(content);
    let path = vault::create_note(vault, title, &stamped)?;
    let link_check = validate_links(vault, &stamped)?;
    Ok(AiWriteResult { path, link_check })
}

/// Overwrite an existing note as AI-authored, re-stamping frontmatter.
pub fn ai_update_note(
    vault: &Path,
    rel: &str,
    content: &str,
) -> std::io::Result<AiWriteResult> {
    let stamped = stamp_ai_author(content);
    vault::write_note(vault, rel, &stamped)?;
    let link_check = validate_links(vault, &stamped)?;
    Ok(AiWriteResult {
        path: rel.to_string(),
        link_check,
    })
}

/// Ensure the note's YAML frontmatter carries `author: ai`. Adds a frontmatter
/// block if none exists; replaces an existing `author:` line otherwise.
pub fn stamp_ai_author(content: &str) -> String {
    let trimmed_start = content.trim_start_matches('\u{feff}');
    if let Some(rest) = trimmed_start.strip_prefix("---") {
        // Existing frontmatter: find its closing `---`.
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            let body = &rest[end..]; // starts at "\n---"
            let mut lines: Vec<String> = front
                .lines()
                .filter(|l| !l.trim_start().to_lowercase().starts_with("author:"))
                .map(|l| l.to_string())
                .collect();
            lines.push("author: ai".to_string());
            let new_front = lines.join("\n");
            return format!("---{new_front}{body}");
        }
    }
    format!("---\nauthor: ai\n---\n\n{content}")
}

/// Notes the model should probably link, as ready-to-use `NoteMeta`.
pub fn candidates_as_notes(candidates: &[LinkCandidate]) -> Vec<NoteMeta> {
    candidates
        .iter()
        .map(|c| NoteMeta {
            path: c.path.clone(),
            title: c.title.clone(),
            ai_authored: false,
        })
        .collect()
}

// --- helpers ---------------------------------------------------------------

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "of", "to", "in", "on", "for", "with", "is", "are",
    "was", "were", "be", "this", "that", "it", "as", "at", "by", "from", "into", "über", "und",
    "oder", "der", "die", "das", "ein", "eine", "ist", "im", "zu", "mit", "auf", "den", "dem",
];

fn significant_terms(text: &str) -> HashSet<String> {
    let stop: HashSet<&str> = STOPWORDS.iter().copied().collect();
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !stop.contains(w.as_str()))
        .collect()
}

fn first_line(content: &str) -> String {
    content
        .lines()
        .map(|l| l.trim_start_matches('#').trim())
        .find(|l| !l.is_empty() && *l != "---")
        .unwrap_or("")
        .chars()
        .take(80)
        .collect()
}

/// Rank titles by closeness to `target` (shared lowercased characters/prefix),
/// returning up to `n` best matches. Deliberately simple; good enough to catch
/// typos and near-misses.
fn closest_titles(target: &str, titles: &[String], n: usize) -> Vec<String> {
    let t = target.to_lowercase();
    let mut scored: Vec<(f32, &String)> = titles
        .iter()
        .map(|title| (similarity(&t, &title.to_lowercase()), title))
        .filter(|(s, _)| *s > 0.3)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|(_, t)| t.clone()).collect()
}

/// Dice coefficient over character bigrams — cheap fuzzy string similarity.
fn similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let bg = |s: &str| -> Vec<[char; 2]> {
        let ch: Vec<char> = s.chars().collect();
        ch.windows(2).map(|w| [w[0], w[1]]).collect()
    };
    let (ba, bb) = (bg(a), bg(b));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let mut bb_used = vec![false; bb.len()];
    let mut hits = 0;
    for x in &ba {
        for (j, y) in bb.iter().enumerate() {
            if !bb_used[j] && x == y {
                bb_used[j] = true;
                hits += 1;
                break;
            }
        }
    }
    (2.0 * hits as f32) / (ba.len() + bb.len()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_vault() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "magma-ai-{:?}-{}",
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
    fn stamp_adds_frontmatter_when_missing() {
        let out = stamp_ai_author("# Hello\n\nbody");
        assert!(out.starts_with("---\nauthor: ai\n---"));
        assert!(out.contains("# Hello"));
    }

    #[test]
    fn stamp_replaces_existing_author() {
        let out = stamp_ai_author("---\ntitle: X\nauthor: human\n---\n\nbody");
        assert_eq!(out.matches("author:").count(), 1);
        assert!(out.contains("author: ai"));
        assert!(out.contains("title: X"));
        assert!(out.contains("body"));
    }

    #[test]
    fn candidates_rank_by_shared_terms() {
        let v = tmp_vault();
        vault::write_note(&v, "Sourdough.md", "# Sourdough\n\nbaking bread with wild yeast starter").unwrap();
        vault::write_note(&v, "Taxes.md", "# Taxes\n\nquarterly filing deadlines").unwrap();
        let c = find_link_candidates(&v, "notes about baking bread yeast", 5).unwrap();
        assert!(!c.is_empty());
        assert_eq!(c[0].title, "Sourdough");
        assert_eq!(c[0].name, "Sourdough");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn validate_flags_broken_with_suggestion() {
        let v = tmp_vault();
        vault::write_note(&v, "Sourdough.md", "bread").unwrap();
        let check = validate_links(&v, "see [[Sourdogh]] and [[Sourdough]]").unwrap();
        assert_eq!(check.resolved, vec!["Sourdough"]);
        assert_eq!(check.broken.len(), 1);
        assert_eq!(check.broken[0].target, "Sourdogh");
        assert!(check.broken[0].suggestions.contains(&"Sourdough".to_string()));
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn ai_create_stamps_and_checks() {
        let v = tmp_vault();
        vault::write_note(&v, "Bread.md", "about bread").unwrap();
        let res = ai_create_note(&v, "Sourdough", "A [[Bread]] variant. See [[Missing]].").unwrap();
        assert_eq!(res.path, "Sourdough.md");
        let content = vault::read_note(&v, &res.path).unwrap().content;
        assert!(content.contains("author: ai"));
        assert_eq!(res.link_check.resolved, vec!["Bread"]);
        assert_eq!(res.link_check.broken.len(), 1);
        assert_eq!(res.link_check.broken[0].target, "Missing");
        // The freshly created note is detected as AI-authored by the vault.
        assert!(vault::read_note(&v, &res.path).unwrap().ai_authored);
        fs::remove_dir_all(&v).ok();
    }
}
