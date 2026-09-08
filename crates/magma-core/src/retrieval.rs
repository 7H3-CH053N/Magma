//! Retrieval: passages, not files.
//!
//! Search answers "which notes mention this". Retrieval answers "which few
//! hundred words should a model read before it says anything". The difference
//! is the unit. [`crate::search`] returns one hit per note with a snippet cut
//! around the match, so a five-thousand-word note arrives whole or not at all;
//! a model handed that spends its context on the wrong paragraphs.
//!
//! So a note is cut into passages along its own structure, and passages are
//! ranked against each other across the whole vault by BM25. Every passage
//! carries where it came from — note, heading, line — because a retrieved
//! passage without provenance is an assertion nobody can check.
//!
//! Scope, stated plainly: this is *lexical*. It ranks by word overlap, so it
//! still does not know that "Auto" and "Fahrzeug" mean the same. Embeddings
//! are M8 phase 3 and slot in beside this, not instead of it: on names, code
//! identifiers and exact terms, word overlap beats a vector, and real notes are
//! full of all three.

use rust_stemmers::{Algorithm, Stemmer};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

use crate::vault;

/// Words per passage, aimed at rather than enforced: a passage ends at the
/// paragraph that crosses this, so sentences are never cut mid-thought.
const TARGET_WORDS: usize = 220;

/// A passage carries the tail of the one before it, so a thought that runs
/// across the boundary is retrievable from either side. Paragraph-aligned, so
/// this is a floor rather than an exact count.
const OVERLAP_WORDS: usize = 40;

/// BM25 term-frequency saturation. 1.2 is the long-standing default: a term
/// appearing ten times makes a passage more relevant than one occurrence, but
/// nowhere near ten times more.
const K1: f32 = 1.2;

/// BM25 length normalisation. At 0.75 a long passage is discounted for its
/// length, but not as harshly as full normalisation would.
const B: f32 = 0.75;

/// How often a passage's context — the note's name and folders, and the heading
/// it sits under — counts alongside its own words.
///
/// Not cosmetic. A note called `Alexander Mut.md` describing a person may never
/// repeat the name in its body, and a note's title falls back to its first line
/// of text, so the name can live *only* in the file name. Scoring the passage
/// text alone made that note unreachable by the person's name, while ordinary
/// notes using "Mut" in its everyday sense filled the results. Headings had the
/// same hole: the chunker lifts a heading out of the text to hand back as
/// provenance, so a passage under `## Notarisierung` could not be reached by
/// that word unless the body happened to repeat it.
///
/// Once, not more. Context should break a tie and rescue a note whose subject
/// is only in its name; it should not outrank a passage that actually discusses
/// the thing.
const CONTEXT_REPEATS: usize = 1;

/// How far down each ranking the fusion looks. Beyond this a hit is noise in
/// one list and absent from the other, and letting it in only dilutes.
const FUSION_DEPTH: usize = 50;

/// Reciprocal-rank-fusion constant. 60 is the value the method was published
/// with and the one search systems use: large enough that the top few ranks are
/// not wildly more valuable than the next few, small enough that rank still
/// decides.
const RRF_K: f32 = 60.0;

/// The seam an embedding model plugs into.
///
/// Deliberately narrow: text in, vectors out. Everything else — which model,
/// where its weights live, whether a vector is cached or computed — belongs to
/// the implementation, so `magma-core` keeps its three dependencies and stays
/// testable without a model on disk.
///
/// `id` names the model. A vector produced by one model means nothing to
/// another, so anything caching vectors keys them by this.
pub trait Similarity: Send + Sync {
    fn id(&self) -> &str;

    /// Embed the question.
    ///
    /// Separate from [`Self::embed_passages`] because the leading models for
    /// this job are asymmetric: E5 and its relatives are trained with "query:"
    /// and "passage:" prefixes and lose accuracy when both sides are embedded
    /// the same way. That loss is invisible — results still come back, just
    /// worse — so the distinction belongs in the interface where an
    /// implementation cannot forget it, rather than in a comment.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;

    /// Embed passages, as one batch: at these sizes a forward pass costs mostly
    /// setup, so one call for many beats many calls for one.
    fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// One retrievable piece of a note.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    /// The nearest heading above this passage, empty when there is none.
    pub heading: String,
    /// 1-based line in the note where the passage starts.
    pub line: usize,
    pub text: String,
}

/// A passage that answered a query, with everything needed to check it.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Passage {
    /// Vault-relative path of the note this came from.
    pub path: String,
    /// The note's display title.
    pub title: String,
    /// The nearest heading above the passage, empty when there is none.
    pub heading: String,
    /// 1-based line in the note where the passage starts.
    pub line: usize,
    pub text: String,
    /// BM25 score. Comparable within one result set, meaningless across two.
    pub score: f32,
}

/// The result of a retrieval, including what it could not see.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Retrieval {
    pub passages: Vec<Passage>,
    /// Notes that were read and cut into passages.
    pub notes_scanned: usize,
    /// Notes skipped because their contents are not on disk (cloud
    /// placeholders). Reported rather than swallowed: a result built from half
    /// a vault must not pass for one built from all of it.
    pub offline: usize,
    /// True when meaning was ranked alongside words. False means the answer is
    /// word overlap only — either no model was given, or it failed and the
    /// search carried on without it. Worth saying out loud: the same query
    /// answers differently, and a caller should not have to guess which it got.
    pub semantic: bool,
}

/// Split text into comparable terms for retrieval.
///
/// Deliberately *not* [`crate::related::tokenize`], and the difference matters
/// on a German vault: that one drops words under three characters, which
/// silently removes "KI" — the subject of a good share of these notes. It also
/// drops stopwords, which BM25 does not need help with, since a term in every
/// passage earns an IDF near zero on its own.
///
/// NFC-normalised because macOS has stored decomposed filenames and text for
/// years, and "über" typed in the query box must match "über" on disk even when
/// the two are different byte sequences.
///
/// Then stemmed, which the evaluation set forced rather than taste: without it
/// "wie exportiere ich das Zertifikat" did not reach a note reading "die p12
/// muss aus Meine Zertifikate exportiert werden". Not one word of the query
/// matched the note. Hand-written suffix rules were not an option — the concept
/// graph already learned that a hand-maintained German word list is never
/// finished — so this is Snowball, the algorithm search engines use for the job.
///
/// What it fixes, measured rather than assumed: German noun forms collapse
/// (`Zertifikat`/`Zertifikate`/`Zertifikats`, `Farbe`/`Farben`,
/// `Schattierung`/`Schattierungen`). What it does not: the `-iert` participle
/// stays apart from its own infinitive (`exportiere` and `exportieren` both
/// reduce to `exporti`, `exportiert` does not), and a noun does not meet its
/// verb (`Import` against `importieren`). So the case above passes on the noun
/// alone. Those gaps are real and are the other half of what embeddings are
/// for; they are pinned in the tests so nobody rediscovers them by accident.
pub fn tokenize(text: &str) -> Vec<String> {
    let stemmer = stemmer();
    text.nfc()
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.chars().count() >= 2)
        .map(|w| stemmer.stem(&w.to_lowercase()).into_owned())
        .collect()
}

/// The stemmer, built once. Constructing one parses a rule table, and this runs
/// per word over the whole vault.
///
/// German for both languages, and that is a choice worth stating. Query and
/// document must be reduced by the *same* function or nothing matches, so the
/// question is not which language a note is in but which single reduction costs
/// least. German it is: this vault is German, and Snowball's German rules strip
/// German endings, which leaves most English words nearly untouched. An English
/// vault loses some English stemming; a German one would lose almost everything
/// the other way round.
fn stemmer() -> &'static Stemmer {
    static STEMMER: OnceLock<Stemmer> = OnceLock::new();
    STEMMER.get_or_init(|| Stemmer::create(Algorithm::German))
}

/// Cut a note into passages along its own structure.
///
/// Headings start a new passage, because a heading is the author's own
/// statement about where one subject ends. Inside a section, paragraphs are
/// accumulated until the passage is long enough, then it closes and the next
/// one opens carrying the tail of this one.
///
/// Frontmatter is skipped: it is metadata about the note, and retrieving
/// "tags: ki, rust" as an answer to a question helps nobody.
pub fn chunk_note(content: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    let mut chunker = Chunker::default();

    for (i, raw) in lines.iter().enumerate().skip(frontmatter_end(&lines)) {
        let trimmed = raw.trim();

        if let Some(text) = heading_text(trimmed) {
            chunker.end_paragraph();
            // A heading ends the previous subject, so the next passage starts
            // clean instead of dragging the old one along.
            chunker.close(false);
            chunker.heading = text;
            continue;
        }

        if trimmed.is_empty() {
            chunker.end_paragraph();
            continue;
        }

        chunker.push_line(raw, i + 1);
    }

    chunker.finish()
}

/// Accumulates lines into paragraphs and paragraphs into passages.
#[derive(Default)]
struct Chunker {
    chunks: Vec<Chunk>,
    heading: String,
    /// Paragraphs of the passage being built, each with its starting line.
    pending: Vec<(usize, String)>,
    pending_words: usize,
    para: Vec<String>,
    para_line: usize,
}

impl Chunker {
    fn push_line(&mut self, raw: &str, line_no: usize) {
        if self.para.is_empty() {
            self.para_line = line_no;
        }
        self.para.push(raw.to_string());
    }

    /// A blank line or a heading ends a paragraph. Closes the passage too once
    /// it is long enough, so passages end at a paragraph rather than mid-thought.
    fn end_paragraph(&mut self) {
        if self.para.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.para).join("\n");
        self.pending_words += word_count(&text);
        self.pending.push((self.para_line, text));
        if self.pending_words >= TARGET_WORDS {
            self.close(true);
        }
    }

    /// Emit the passage under construction. With `carry`, the next passage
    /// opens holding this one's tail, so a thought spanning the boundary stays
    /// findable from either side.
    fn close(&mut self, carry: bool) {
        if self.pending.is_empty() {
            return;
        }
        let line = self.pending[0].0;
        let text = self
            .pending
            .iter()
            .map(|(_, p)| p.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        self.chunks.push(Chunk {
            heading: self.heading.clone(),
            line,
            text,
        });
        self.pending = if carry {
            tail(&self.pending, OVERLAP_WORDS)
        } else {
            Vec::new()
        };
        self.pending_words = self.pending.iter().map(|(_, p)| word_count(p)).sum();
    }

    fn finish(mut self) -> Vec<Chunk> {
        self.end_paragraph();
        self.close(false);
        self.chunks
    }
}

/// Rank passages across the whole vault against a query.
///
/// Reads through [`vault::read_for_scan`], like every other vault-wide sweep,
/// so a cloud placeholder is counted and reported rather than downloaded and
/// waited on.
pub fn retrieve(vault: &Path, query: &str, limit: usize) -> std::io::Result<Retrieval> {
    retrieve_with(vault, query, limit, None)
}

/// Retrieve, optionally ranking meaning alongside words.
///
/// With a model, two rankings are built over the same passages — BM25 over
/// words, cosine over embeddings — and fused by reciprocal rank. Fusion by rank
/// rather than by score is the point: a BM25 score has no fixed range and means
/// nothing across two queries, a cosine sits in [-1, 1], and any formula
/// weighing one against the other would be a guess dressed up as arithmetic.
/// Ranks are comparable by construction.
///
/// The two halves cover different failures. Words find names, identifiers and
/// exact terms, where a vector is weak. Meaning finds the note that says the
/// same thing in other words, which word overlap cannot reach at all. Neither
/// replaces the other, which is why this fuses instead of choosing.
///
/// A model that fails does not fail the search: the result comes back lexical
/// with `semantic: false`. A broken model should cost quality, not answers.
pub fn retrieve_with(
    vault: &Path,
    query: &str,
    limit: usize,
    model: Option<&dyn Similarity>,
) -> std::io::Result<Retrieval> {
    let terms = tokenize(query);
    let notes = vault::list_notes(vault)?;

    // Everything the ranking needs, gathered in one pass over the vault.
    struct Candidate {
        path: String,
        title: String,
        chunk: Chunk,
        len: usize,
        freq: HashMap<String, usize>,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut notes_scanned = 0usize;
    let mut offline = 0usize;

    for note in &notes {
        let full = vault.join(&note.path);
        let content = vault::read_for_scan(&full);
        if content.is_empty() {
            // Tell "not on disk" apart from "genuinely empty": only the first
            // means the vault has more to say than this result shows.
            if vault::is_offline(&full) {
                offline += 1;
            }
            continue;
        }
        notes_scanned += 1;
        // The note's own name and its folders, which carry subject matter a
        // body often leaves implicit ("Blog/KI-Wissen/…", "Alexander Mut").
        let name_tokens = tokenize(note.path.trim_end_matches(".md"));
        for chunk in chunk_note(&content) {
            let mut tokens = tokenize(&chunk.text);
            for _ in 0..CONTEXT_REPEATS {
                tokens.extend(name_tokens.iter().cloned());
                tokens.extend(tokenize(&chunk.heading));
            }
            let mut freq: HashMap<String, usize> = HashMap::new();
            for t in &tokens {
                *freq.entry(t.clone()).or_insert(0) += 1;
            }
            candidates.push(Candidate {
                path: note.path.clone(),
                title: note.title.clone(),
                len: tokens.len(),
                freq,
                chunk,
            });
        }
    }

    let mut out = Retrieval {
        passages: Vec::new(),
        notes_scanned,
        offline,
        semantic: false,
    };
    if terms.is_empty() || candidates.is_empty() {
        return Ok(out);
    }

    let n = candidates.len() as f32;
    let avgdl = candidates.iter().map(|c| c.len).sum::<usize>() as f32 / n;

    // Document frequency per query term, counted once rather than per passage.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for term in &terms {
        if df.contains_key(term.as_str()) {
            continue;
        }
        let count = candidates
            .iter()
            .filter(|c| c.freq.contains_key(term))
            .count();
        df.insert(term.as_str(), count);
    }

    let mut scored: Vec<(f32, usize)> = Vec::new();
    for (i, cand) in candidates.iter().enumerate() {
        let mut score = 0f32;
        for term in &terms {
            let f = match cand.freq.get(term) {
                Some(f) => *f as f32,
                None => continue,
            };
            let n_q = df.get(term.as_str()).copied().unwrap_or(0) as f32;
            // Lucene's IDF: always positive, so a term in every passage adds
            // almost nothing instead of subtracting.
            let idf = (1.0 + (n - n_q + 0.5) / (n_q + 0.5)).ln();
            let norm = 1.0 - B + B * (cand.len as f32 / avgdl);
            score += idf * (f * (K1 + 1.0)) / (f + K1 * norm);
        }
        if score > 0.0 {
            scored.push((score, i));
        }
    }

    // Ties by path and line, so the same query gives the same order twice.
    // Vault order comes from the file system otherwise.
    let tie = |a: &(f32, usize), b: &(f32, usize)| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| candidates[a.1].path.cmp(&candidates[b.1].path))
            .then_with(|| candidates[a.1].chunk.line.cmp(&candidates[b.1].chunk.line))
    };
    scored.sort_by(tie);

    let semantic = match model {
        Some(m) => {
            let texts: Vec<String> = candidates
                .iter()
                .map(|c| embedding_text(&c.path, &c.chunk))
                .collect();
            semantic_ranking(m, query, &texts).map(|mut sem| {
                sem.sort_by(tie);
                sem
            })
        }
        None => None,
    };

    let ranked: Vec<(f32, usize)> = match &semantic {
        Some(sem) => {
            out.semantic = true;
            fuse(&scored, sem)
        }
        None => scored,
    };

    out.passages = ranked
        .into_iter()
        .take(limit)
        .map(|(score, i)| {
            let c = &candidates[i];
            Passage {
                path: c.path.clone(),
                title: c.title.clone(),
                heading: c.chunk.heading.clone(),
                line: c.chunk.line,
                text: c.chunk.text.clone(),
                score,
            }
        })
        .collect();
    Ok(out)
}

/// What actually goes to the model for a passage.
///
/// Not the passage text alone. The note's name and the heading above it carry
/// subject matter a body often leaves implicit, and leaving them out here would
/// reopen on the semantic side the hole that scoring text alone opened on the
/// lexical one.
pub fn embedding_text(path: &str, chunk: &Chunk) -> String {
    let name = path.trim_end_matches(".md").replace('/', " / ");
    if chunk.heading.is_empty() {
        format!("{name}\n{}", chunk.text)
    } else {
        format!("{name} / {}\n{}", chunk.heading, chunk.text)
    }
}

/// Cosine of every passage against the query, as a ranking. `None` when the
/// model could not answer, which leaves the search lexical rather than empty.
fn semantic_ranking(
    model: &dyn Similarity,
    query: &str,
    texts: &[String],
) -> Option<Vec<(f32, usize)>> {
    let q = model.embed_query(query).ok()?;
    let vectors = model.embed_passages(texts).ok()?;
    // A model that returns the wrong number of vectors is broken in a way that
    // would silently misalign every passage with someone else's meaning.
    if vectors.len() != texts.len() {
        return None;
    }
    Some(
        vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (cosine(&q, v), i))
            .collect(),
    )
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Reciprocal rank fusion of two rankings over the same passages.
///
/// A passage scores `1 / (k + rank)` in each list it appears in, summed. A hit
/// both halves agree on rises above one that only either found, which is the
/// whole reason to run two.
fn fuse(lexical: &[(f32, usize)], semantic: &[(f32, usize)]) -> Vec<(f32, usize)> {
    let mut fused: HashMap<usize, f32> = HashMap::new();
    for list in [lexical, semantic] {
        for (rank, (_, idx)) in list.iter().take(FUSION_DEPTH).enumerate() {
            *fused.entry(*idx).or_insert(0.0) += 1.0 / (RRF_K + rank as f32 + 1.0);
        }
    }
    let mut out: Vec<(f32, usize)> = fused.into_iter().map(|(i, s)| (s, i)).collect();
    // Ties broken by index so the order is the same twice, as in the lexical
    // path. A HashMap hands them back in whatever order it likes.
    out.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    out
}

/// The last paragraphs of a passage, totalling at least `want` words, to open
/// the next passage with.
///
/// Never the whole passage: the next one would then be a superset of this one,
/// and a single paragraph longer than [`TARGET_WORDS`] would carry itself into
/// every passage after it, repeating verbatim down the note. Capping the carry
/// at one paragraph short of the passage makes that case an empty carry, which
/// is right — a paragraph that long has no tail to share.
fn tail(pending: &[(usize, String)], want: usize) -> Vec<(usize, String)> {
    let max = pending.len().saturating_sub(1);
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut words = 0usize;
    for item in pending.iter().rev().take(max) {
        words += word_count(&item.1);
        out.push(item.clone());
        if words >= want {
            break;
        }
    }
    out.reverse();
    out
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// The heading's own text, for a line that is an ATX heading (`## Title`).
fn heading_text(trimmed: &str) -> Option<String> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    // `#Tag` is not a heading; markdown wants the space.
    let rest = trimmed[hashes..].strip_prefix(' ')?;
    Some(rest.trim().to_string())
}

/// Index of the first line after YAML frontmatter, or 0 when there is none.
fn frontmatter_end(lines: &[&str]) -> usize {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return 0;
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim_end() == "---" {
            return i + 1;
        }
    }
    // An unterminated block is not frontmatter, it is a horizontal rule and
    // then the note. Reading the whole note as metadata would lose everything.
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_starts_a_new_passage_and_names_it() {
        let note = "Vorspann ohne Ueberschrift.\n\n## Farben\n\nRot bleibt rot.\n\n## Import\n\nDer Feed wird uebersprungen.\n";
        let chunks = chunk_note(note);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading, "");
        assert_eq!(chunks[1].heading, "Farben");
        assert_eq!(chunks[2].heading, "Import");
        assert!(chunks[1].text.contains("Rot bleibt rot"));
        // A passage must not drag the previous subject along.
        assert!(!chunks[2].text.contains("Rot bleibt rot"));
    }

    #[test]
    fn a_passage_says_which_line_it_starts_on() {
        // Provenance is the whole point: a passage nobody can locate in the
        // note is an assertion nobody can check.
        let note = "eins\n\n## Zwei\n\ndrei\n";
        let chunks = chunk_note(note);
        assert_eq!(chunks[0].line, 1);
        assert_eq!(chunks[1].line, 5);
    }

    #[test]
    fn frontmatter_is_not_retrievable_text() {
        let note = "---\ntags: ki, rust\nauthor: ai\n---\n\nDer eigentliche Text.\n";
        let chunks = chunk_note(note);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Der eigentliche Text.");
        assert_eq!(chunks[0].line, 6);
    }

    #[test]
    fn an_unterminated_rule_is_not_frontmatter() {
        // `---` with no closing fence is a horizontal rule. Treating the rest
        // of the note as metadata would drop the entire note from retrieval.
        let note = "---\n\nAlles was hier steht ist Text.\n";
        let chunks = chunk_note(note);
        assert!(
            chunks
                .iter()
                .any(|c| c.text.contains("Alles was hier steht")),
            "note vanished: {chunks:#?}"
        );
    }

    #[test]
    fn a_hashtag_is_not_a_heading() {
        // Markdown wants a space. Without this check every #tag would cut the
        // note in two and take the heading slot.
        let note = "Notiz mit #tag mittendrin.\n\n#nichtueberschrift\n\nWeiter.\n";
        let chunks = chunk_note(note);
        assert_eq!(chunks.len(), 1, "{chunks:#?}");
        assert_eq!(chunks[0].heading, "");
    }

    #[test]
    fn a_long_section_is_split_with_overlap() {
        let para = |n: usize| format!("absatz{n} {}", "wort ".repeat(80));
        let note = format!("{}\n\n{}\n\n{}\n", para(1), para(2), para(3));
        let chunks = chunk_note(&note);
        assert_eq!(chunks.len(), 2, "{chunks:#?}");
        // The tail of the first passage opens the second, so a thought running
        // across the cut is retrievable from either side. The tail is the last
        // paragraph, so it is absatz3 that appears twice, not absatz2.
        assert!(chunks[0].text.contains("absatz3"));
        assert!(chunks[1].text.contains("absatz3"));
        // And only the tail carries over: an overlap that took the whole
        // passage would make the second a copy of the first.
        assert!(!chunks[1].text.contains("absatz1"));
        assert!(!chunks[1].text.contains("absatz2"));
    }

    // The failure this replaced: the carry took every pending paragraph, so a
    // single paragraph past the target copied itself into the next passage and
    // from there into the one after it.
    #[test]
    fn a_single_long_paragraph_is_not_repeated() {
        let para = "wort ".repeat(300);
        let chunks = chunk_note(&format!("{para}\n\nzweiter absatz hier\n"));
        assert_eq!(chunks.len(), 2, "{chunks:#?}");
        assert!(chunks[1].text.contains("zweiter absatz"));
        assert!(!chunks[1].text.starts_with("wort"));
    }

    #[test]
    fn two_letter_words_survive_tokenising() {
        // The reason this does not reuse `related::tokenize`: that drops words
        // under three characters, and "KI" is the subject of a good share of
        // these notes. Retrieving nothing for "KI" would look like an empty
        // vault rather than a dropped term.
        assert!(tokenize("Notizen über KI").contains(&"ki".to_string()));
        assert!(!crate::related::tokenize("Notizen über KI").contains(&"ki".to_string()));
    }

    #[test]
    fn german_noun_forms_reduce_to_the_same_term() {
        // What the evaluation set caught: "Zertifikat" in a query has to reach
        // "Zertifikate" in a note. One word to a reader, two to the index.
        assert_eq!(tokenize("Zertifikat"), tokenize("Zertifikate"));
        assert_eq!(tokenize("Zertifikat"), tokenize("Zertifikats"));
        assert_eq!(tokenize("Farbe"), tokenize("Farben"));
        assert_eq!(tokenize("Schattierung"), tokenize("Schattierungen"));
        // Not everything collapses, or the index would stop discriminating.
        assert_ne!(tokenize("Signieren"), tokenize("Signal"));
    }

    // Measured, not assumed, and left failing-by-assertion so it is visible:
    // Snowball German is conservative about verbs. These pairs read as the same
    // word and stay two terms, which is a live limitation of lexical retrieval
    // and part of what M8 phase 3 is meant to cover. If a later change makes
    // either pair match, this test is the one to update.
    #[test]
    fn verb_forms_still_do_not_meet_and_that_is_a_known_gap() {
        assert_ne!(tokenize("exportiere"), tokenize("exportiert"));
        assert_ne!(tokenize("Import"), tokenize("importieren"));
    }

    #[test]
    fn stopwords_are_kept_because_bm25_discounts_them_itself() {
        assert!(tokenize("das und der").contains(&"und".to_string()));
    }

    // ---- Ranking, against a vault on disk ----

    fn tmp_vault(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("magma-retrieval-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) {
        let full = dir.join(name);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, body).unwrap();
    }

    /// A small vault standing in for a real one: German, headed sections,
    /// several notes that share vocabulary so ranking has something to do.
    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir = tmp_vault(tag);
        write(
            &dir,
            "Signieren.md",
            "# Signieren\n\n## Zertifikat\n\nDie p12 muss aus Meine Zertifikate exportiert werden, nur dort haengt der private Schluessel dran.\n\n## Notarisierung\n\nDie Notarisierung wartet auf Apple. Der erste Lauf brauchte sechsunddreissig Minuten in der Warteschlange.\n",
        );
        write(
            &dir,
            "Graph/Farben.md",
            "# Farben im Graph\n\n## Picker\n\nDie gewaehlte Farbe wird exakt uebernommen. Helles Rot bleibt hell statt volles Rot zu werden.\n\n## Schattierung\n\nUnterordner laufen per goldenem Schnitt durch das Band, damit zwoelf Unterordner zwoelf Schattierungen ergeben.\n",
        );
        write(
            &dir,
            "Import.md",
            "# Blog Import\n\nDer Autoren Feed wird uebersprungen, wenn ein Autor eingetragen ist. Der Fortschritt meldet jetzt auch die stillen Phasen.\n",
        );
        write(
            &dir,
            "Vault/Platzhalter.md",
            "# Platzhalter\n\nDateien ohne Inhalt auf der Platte werden beim Durchsuchen uebersprungen statt heruntergeladen.\n",
        );
        write(
            &dir,
            "Sammelsurium.md",
            &format!(
                "# Sammelsurium\n\nEine lange Notiz die alles streift. Farbe kommt hier genau einmal vor.\n\n{}\n",
                "fuellwort ".repeat(400)
            ),
        );
        dir
    }

    /// The evaluation set the plan puts in phase one: a question, and the note
    /// that answers it. Add real cases here as they come up; the measurement is
    /// what turns a change to chunking or ranking into a number instead of a
    /// feeling.
    const CASES: &[(&str, &str)] = &[
        ("wie exportiere ich das zertifikat", "Signieren.md"),
        ("wie lange dauert die notarisierung", "Signieren.md"),
        ("warum wird helles rot zu vollem rot", "Graph/Farben.md"),
        ("schattierung der unterordner", "Graph/Farben.md"),
        ("autoren feed beim import", "Import.md"),
        ("platzhalter beim durchsuchen", "Vault/Platzhalter.md"),
    ];

    #[test]
    fn every_evaluation_case_lands_in_the_top_five() {
        let dir = fixture("eval");
        let mut missed = Vec::new();
        for (question, expected) in CASES {
            let got = retrieve(&dir, question, 5).unwrap();
            let paths: Vec<&str> = got.passages.iter().map(|p| p.path.as_str()).collect();
            if !paths.contains(expected) {
                missed.push(format!("{question:?} -> {paths:?}, wanted {expected:?}"));
            }
        }
        assert!(
            missed.is_empty(),
            "recall@5 {}/{}:\n{}",
            CASES.len() - missed.len(),
            CASES.len(),
            missed.join("\n")
        );
    }

    // Straight from a real vault, and a case the fixture above could never
    // catch. "Alexander Mut.md" describes a person and never repeats the name
    // in its body, so the name exists only as the file's name. Meanwhile "Mut"
    // is an ordinary German word, so notes about courage crowd the results.
    // Scoring passage text alone left the person out entirely.
    // ---- The hybrid path ----

    /// A stand-in for a real model: a hand-built space just big enough to test
    /// the plumbing. Words that mean the same thing sit on the same axis, so
    /// "Beglaubigung" is close to "Notarisierung" without sharing a letter.
    ///
    /// This tests the fusion, the fallback and the wiring. It says nothing
    /// about whether real embeddings help a German vault: only a real model on
    /// a real vault answers that.
    struct ToyModel;

    const AXES: &[&[&str]] = &[
        &["notarisierung", "beglaubigung", "apple", "freigabe"],
        &["zertifikat", "signatur", "schluessel", "p12"],
        &["farbe", "rot", "schattierung"],
        &["import", "feed", "autor"],
    ];

    fn toy_vector(text: &str) -> Vec<f32> {
        let lower = text.to_lowercase();
        AXES.iter()
            .map(|axis| axis.iter().filter(|w| lower.contains(**w)).count() as f32)
            .collect()
    }

    impl Similarity for ToyModel {
        fn id(&self) -> &str {
            "toy-v1"
        }
        fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
            Ok(toy_vector(text))
        }
        fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts.iter().map(|t| toy_vector(t)).collect())
        }
    }

    #[test]
    fn meaning_reaches_what_words_cannot() {
        // The same query as the lexical-only test above, which misses.
        let dir = fixture("hybrid");
        let got = retrieve_with(
            &dir,
            "beglaubigung durch den hersteller",
            5,
            Some(&ToyModel),
        )
        .unwrap();
        let paths: Vec<&str> = got.passages.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&"Signieren.md"), "{paths:?}");
        assert!(got.semantic);
    }

    #[test]
    fn the_hybrid_does_not_lose_what_words_already_found() {
        // Adding meaning must not cost precision. Every evaluation case has to
        // survive the fusion, or the second half is making things worse.
        let dir = fixture("hybrideval");
        let mut missed = Vec::new();
        for (question, expected) in CASES {
            let got = retrieve_with(&dir, question, 5, Some(&ToyModel)).unwrap();
            let paths: Vec<&str> = got.passages.iter().map(|p| p.path.as_str()).collect();
            if !paths.contains(expected) {
                missed.push(format!("{question:?} -> {paths:?}, wanted {expected:?}"));
            }
        }
        assert!(missed.is_empty(), "{}", missed.join("\n"));
    }

    #[test]
    fn a_broken_model_costs_quality_not_answers() {
        struct Broken;
        impl Similarity for Broken {
            fn id(&self) -> &str {
                "broken"
            }
            fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
                Err("weights not loaded".into())
            }
            fn embed_passages(&self, _: &[String]) -> Result<Vec<Vec<f32>>, String> {
                Err("weights not loaded".into())
            }
        }
        let dir = fixture("broken");
        let got = retrieve_with(&dir, "notarisierung warteschlange", 5, Some(&Broken)).unwrap();
        assert!(!got.semantic, "a failed model must say so");
        assert_eq!(
            got.passages[0].path, "Signieren.md",
            "lexical must still work"
        );
    }

    #[test]
    fn a_model_returning_the_wrong_count_is_refused() {
        // The dangerous failure, because it does not look like one: too few
        // vectors and every passage silently takes someone else's meaning.
        struct Short;
        impl Similarity for Short {
            fn id(&self) -> &str {
                "short"
            }
            fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
                Ok(vec![1.0, 0.0])
            }
            fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
                Ok(vec![vec![1.0, 0.0]; texts.len().saturating_sub(1)])
            }
        }
        let dir = fixture("short");
        let got = retrieve_with(&dir, "notarisierung", 5, Some(&Short)).unwrap();
        assert!(!got.semantic);
        assert_eq!(got.passages[0].path, "Signieren.md");
    }

    #[test]
    fn the_hybrid_order_is_the_same_twice() {
        let dir = fixture("hybridstable");
        let key = |r: &Retrieval| -> Vec<(String, usize)> {
            r.passages
                .iter()
                .map(|p| (p.path.clone(), p.line))
                .collect()
        };
        let a = retrieve_with(&dir, "zertifikat exportieren", 10, Some(&ToyModel)).unwrap();
        let b = retrieve_with(&dir, "zertifikat exportieren", 10, Some(&ToyModel)).unwrap();
        assert_eq!(key(&a), key(&b));
    }

    #[test]
    fn without_a_model_nothing_claims_to_be_semantic() {
        let dir = fixture("nomodel");
        assert!(!retrieve(&dir, "notarisierung", 3).unwrap().semantic);
    }

    #[test]
    fn a_note_is_findable_by_its_own_name() {
        let dir = tmp_vault("byname");
        write(
            &dir,
            "Alexander Mut.md",
            "Developer, der am Magma Projekt mitarbeitet.\n",
        );
        write(
            &dir,
            "KI-Strategie.md",
            "# Strategie\n\nEs braucht Mut, eine Strategie zu aendern.\n",
        );
        write(
            &dir,
            "EU-Whitepaper.md",
            "# Whitepaper\n\nDer Entwurf fordert Mut zur Regulierung.\n",
        );
        write(
            &dir,
            "Fussball.md",
            "# Schlager\n\nDer Torhueter zeigte Mut im Strafraum.\n",
        );

        let got = retrieve(&dir, "alexander mut", 5).unwrap();
        let paths: Vec<&str> = got.passages.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths.first(), Some(&"Alexander Mut.md"), "{paths:?}");
    }

    // The other half of the same blind spot: a heading is lifted out of the
    // text and handed back as provenance, so a passage under "## Notarisierung"
    // was unreachable by that word unless the body happened to repeat it. Every
    // note in the fixture above does repeat it, which is why nothing caught this.
    #[test]
    fn a_heading_makes_its_own_passage_findable() {
        let dir = tmp_vault("byheading");
        write(
            &dir,
            "Signieren.md",
            "# Signieren\n\n## Notarisierung\n\nApple laesst sich damit Zeit.\n\n## Zertifikat\n\nKommt aus dem Schluesselbund.\n",
        );
        let got = retrieve(&dir, "notarisierung", 5).unwrap();
        assert_eq!(
            got.passages.first().map(|p| p.heading.as_str()),
            Some("Notarisierung"),
            "{:#?}",
            got.passages
        );
    }

    #[test]
    fn a_passage_carries_the_heading_it_came_from() {
        let dir = fixture("provenance");
        let got = retrieve(&dir, "notarisierung warteschlange", 3).unwrap();
        let top = &got.passages[0];
        assert_eq!(top.path, "Signieren.md");
        assert_eq!(top.heading, "Notarisierung");
        assert!(top.line > 1);
        // The passage is a section, not the whole note: the certificate half
        // must not ride along.
        assert!(!top.text.contains("Meine Zertifikate"));
    }

    #[test]
    fn a_long_rambling_note_does_not_win_on_length() {
        // BM25's length normalisation is the point: Sammelsurium mentions
        // "Farbe" once in four hundred words of filler, Farben.md is about it.
        let dir = fixture("length");
        let got = retrieve(&dir, "farbe", 5).unwrap();
        assert_eq!(
            got.passages[0].path, "Graph/Farben.md",
            "{:#?}",
            got.passages
        );
    }

    #[test]
    fn an_empty_note_is_not_reported_as_offline() {
        // "Not on disk" and "genuinely empty" both read as empty text, and only
        // the first means the vault has more to say than the result shows.
        let dir = tmp_vault("empty");
        write(&dir, "Leer.md", "");
        write(
            &dir,
            "Voll.md",
            "# Voll\n\nHier steht etwas ueber Farben.\n",
        );
        let got = retrieve(&dir, "farben", 5).unwrap();
        assert_eq!(got.notes_scanned, 1);
        assert_eq!(got.offline, 0);
    }

    #[test]
    fn the_same_query_gives_the_same_order_twice() {
        // Vault order comes from the file system otherwise, so ties would
        // shuffle between runs and a result would not be reproducible.
        let dir = fixture("stable");
        let a = retrieve(&dir, "farbe uebernommen", 10).unwrap();
        let b = retrieve(&dir, "farbe uebernommen", 10).unwrap();
        let key = |r: &Retrieval| -> Vec<(String, usize)> {
            r.passages
                .iter()
                .map(|p| (p.path.clone(), p.line))
                .collect()
        };
        assert_eq!(key(&a), key(&b));
    }

    #[test]
    fn nothing_comes_back_for_an_empty_query() {
        let dir = fixture("emptyquery");
        assert!(retrieve(&dir, "   ", 5).unwrap().passages.is_empty());
    }

    // The gap that word overlap cannot close, pinned so it stays visible.
    // "Beglaubigung" is a synonym nobody wrote down, so no amount of stemming
    // reaches the notarisation note. The sibling test below shows the hybrid
    // path finding it; this one exists to show that the lexical half alone
    // still cannot, which is the reason both halves are run.
    #[test]
    fn a_synonym_is_out_of_reach_for_words_alone() {
        let dir = fixture("synonym");
        let got = retrieve(&dir, "beglaubigung durch den hersteller", 5).unwrap();
        let paths: Vec<&str> = got.passages.iter().map(|p| p.path.as_str()).collect();
        assert!(
            !paths.contains(&"Signieren.md"),
            "a synonym now reaches the note: if embeddings landed, this test is \
             the one to update. paths: {paths:?}"
        );
    }

    #[test]
    fn decomposed_and_composed_umlauts_are_the_same_term() {
        // macOS stored decomposed text for years. Without NFC, "über" typed in
        // a query is a different byte sequence from "über" on disk.
        let composed = "\u{fc}ber";
        let decomposed = "u\u{308}ber";
        assert_ne!(composed, decomposed);
        assert_eq!(tokenize(composed), tokenize(decomposed));
    }
}
