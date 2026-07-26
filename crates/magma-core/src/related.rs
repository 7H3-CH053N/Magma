//! "Notes like this one" — finding the neighbours a link was never drawn to.
//!
//! Scope, stated plainly: this is *lexical* similarity (TF-IDF over the words
//! in each note, compared by cosine). It finds notes that talk about the same
//! things using the same words. It does not know that "Auto" and "Fahrzeug"
//! mean the same, the way a neural embedding would.
//!
//! It earns its place anyway: it needs no model download, no ONNX runtime, no
//! network, and it runs over a few thousand notes in milliseconds — so it works
//! offline on day one. `Similarity` is the seam a real embedding model would
//! slot into later without any caller changing.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::vault;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedNote {
    pub path: String,
    pub title: String,
    /// 0..1 — how much vocabulary this note shares with the one asked about.
    pub score: f32,
    /// True when a `[[wikilink]]` already connects the two.
    pub linked: bool,
}

/// Words too common to say anything about what a note is *about*. Kept short
/// and bilingual on purpose: TF-IDF already discounts anything that appears
/// everywhere, this just spares the index the most obvious noise.
const STOPWORDS: &[&str] = &[
    "aber", "alle", "als", "also", "am", "an", "auch", "auf", "aus", "bei", "bin", "bis", "das",
    "dass", "dem", "den", "der", "des", "die", "dies", "diese", "doch", "dort", "du", "ein",
    "eine", "einem", "einen", "einer", "eines", "er", "es", "für", "hat", "hatte", "ich", "ihr",
    "im", "in", "ist", "kann", "man", "mehr", "mit", "nach", "nicht", "noch", "nur", "oder",
    "sein", "sich", "sie", "sind", "über", "um", "und", "uns", "unter", "von", "vor", "war",
    "wenn", "werden", "wie", "wir", "wird", "zu", "zum", "zur", "about", "all", "and", "any",
    "are", "been", "but", "can", "for", "from", "has", "have", "how", "if", "into", "its", "may",
    "more", "not", "our", "out", "she", "than", "that", "the", "their", "them", "then", "there",
    "these", "they", "this", "were", "what", "when", "which", "who", "will", "with", "you",
    "your",
];

/// Split text into comparable terms: lowercased words of 3+ characters, with
/// markdown punctuation and stopwords dropped.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|w| w.trim().to_lowercase())
        .filter(|w| w.chars().count() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Term frequencies of one note, normalised by length so a long note does not
/// outweigh a short one just by being long.
fn term_freq(text: &str) -> HashMap<String, f32> {
    let mut counts: HashMap<String, f32> = HashMap::new();
    let mut total = 0f32;
    for term in tokenize(text) {
        *counts.entry(term).or_insert(0.0) += 1.0;
        total += 1.0;
    }
    if total > 0.0 {
        for v in counts.values_mut() {
            *v /= total;
        }
    }
    counts
}

fn cosine(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    // Walk the shorter one; terms missing from the other contribute nothing.
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let dot: f32 = short
        .iter()
        .filter_map(|(term, x)| long.get(term).map(|y| x * y))
        .sum();
    let na = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let nb = b.values().map(|v| v * v).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// The notes most similar to `rel`, best first.
///
/// Notes already linked to this one are included and flagged rather than
/// hidden: seeing that a strong match *is* linked is the reassurance that the
/// list is working, and it costs one field.
pub fn related_notes(
    vault: &Path,
    rel: &str,
    limit: usize,
) -> std::io::Result<Vec<RelatedNote>> {
    let notes = vault::list_notes(vault)?;
    let mut docs: Vec<(vault::NoteMeta, String)> = Vec::with_capacity(notes.len());
    for note in notes {
        let content = std::fs::read_to_string(vault.join(&note.path)).unwrap_or_default();
        docs.push((note, content));
    }

    let me = match docs.iter().position(|(n, _)| n.path == rel) {
        Some(i) => i,
        None => return Ok(Vec::new()),
    };

    // Inverse document frequency: a word in every note tells us nothing; a word
    // in three notes ties those three together.
    let n_docs = docs.len().max(1) as f32;
    let mut doc_count: HashMap<String, f32> = HashMap::new();
    let tfs: Vec<HashMap<String, f32>> = docs
        .iter()
        .map(|(_, text)| {
            let tf = term_freq(text);
            for term in tf.keys() {
                *doc_count.entry(term.clone()).or_insert(0.0) += 1.0;
            }
            tf
        })
        .collect();
    let idf = |term: &str| -> f32 {
        let df = doc_count.get(term).copied().unwrap_or(1.0);
        (n_docs / df).ln() + 1.0
    };
    let weighted = |tf: &HashMap<String, f32>| -> HashMap<String, f32> {
        tf.iter().map(|(t, f)| (t.clone(), f * idf(t))).collect()
    };

    let mine = weighted(&tfs[me]);
    let my_links: Vec<String> = crate::links::extract_links(&docs[me].1)
        .into_iter()
        .map(|l| l.to_lowercase())
        .collect();
    let my_name = crate::links::note_name(rel).to_lowercase();

    let mut out: Vec<RelatedNote> = Vec::new();
    for (i, (note, content)) in docs.iter().enumerate() {
        if i == me {
            continue;
        }
        let score = cosine(&mine, &weighted(&tfs[i]));
        if score <= 0.0 {
            continue;
        }
        let name = crate::links::note_name(&note.path).to_lowercase();
        let linked = my_links.contains(&name)
            || crate::links::extract_links(content)
                .iter()
                .any(|l| l.to_lowercase() == my_name);
        out.push(RelatedNote {
            path: note.path.clone(),
            title: note.title.clone(),
            score,
            linked,
        });
    }
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_vault(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "magma-rel-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ranks_a_note_about_the_same_thing_above_an_unrelated_one() {
        let v = tmp_vault("rank");
        vault::write_note(
            &v,
            "Sauerteig.md",
            "# Sauerteig\n\nBrot backen mit Sauerteig: Anstellgut, Roggenmehl, Gehzeit.",
        )
        .unwrap();
        vault::write_note(
            &v,
            "Roggenbrot.md",
            "# Roggenbrot\n\nRoggenmehl und Anstellgut, lange Gehzeit, dann backen.",
        )
        .unwrap();
        vault::write_note(
            &v,
            "Steuer.md",
            "# Steuer\n\nUmsatzsteuervoranmeldung, Belege, Fristen beim Finanzamt.",
        )
        .unwrap();

        let hits = related_notes(&v, "Sauerteig.md", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "Roggenbrot.md", "got: {:?}", hits[0].path);
        assert!(!hits[0].linked, "nothing links them yet — that's the point");
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn marks_neighbours_that_are_already_linked() {
        let v = tmp_vault("linked");
        vault::write_note(&v, "A.md", "Projekt Magma: Notizen, Graph, Vault. [[B]]").unwrap();
        vault::write_note(&v, "B.md", "Projekt Magma: Vault, Graph und Notizen.").unwrap();
        let hits = related_notes(&v, "A.md", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].linked);
        fs::remove_dir_all(&v).ok();
    }

    #[test]
    fn stopwords_alone_do_not_make_notes_related() {
        let v = tmp_vault("stop");
        vault::write_note(&v, "X.md", "und der die das mit von für").unwrap();
        vault::write_note(&v, "Y.md", "und der die das mit von für").unwrap();
        vault::write_note(&v, "Z.md", "Sauerteig Roggenmehl Anstellgut").unwrap();
        let hits = related_notes(&v, "X.md", 5).unwrap();
        assert!(hits.is_empty(), "nothing but stopwords: {:?}", hits.len());
        fs::remove_dir_all(&v).ok();
    }
}
