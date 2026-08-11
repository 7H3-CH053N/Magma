//! The concept graph: what your notes are *about*, rather than what links to
//! what.
//!
//! Magma's normal graph draws notes and the `[[links]]` you wrote. This one
//! draws terms and the fact that they keep turning up near each other. It
//! shows the shape of a vault whose links were never drawn — which is most
//! vaults, most of the time.
//!
//! Scope, stated as plainly as `related.rs` states its own: **these are words
//! that co-occur, not meanings that relate.** Nothing here understands German
//! or English. A cluster is a set of terms that share sentences, and it earns
//! the name "topic" only in the reader's head. No model is downloaded, nothing
//! leaves the machine, and the whole thing is a few hundred lines of counting.
//!
//! The hard part is not the graph, it is German. "Notiz", "Notizen" and
//! "Notizen" are one idea; left alone they are three nodes with a third of the
//! weight each, and the graph turns into a haze of near-duplicates. So every
//! term is folded to a crude stem for *grouping*, while the label shown is the
//! spelling you actually write most often.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

use crate::vault;

/// Words that say nothing about what a note is about. This is on top of the
/// TF-IDF stopword list in `related`, which is deliberately short because
/// TF-IDF already discounts anything that appears everywhere. A co-occurrence
/// graph has no such defence: a verb everyone uses becomes a hub connected to
/// everything, and the layout collapses around it.
const FILLER: &[&str] = &[
    // German — verbs and adverbs that carry no subject matter
    "aber",
    "allein",
    "allerdings",
    "andere",
    "anderen",
    "beim",
    "besser",
    "bereits",
    "bitte",
    "brauchen",
    "dabei",
    "dafür",
    "damit",
    "dann",
    "daran",
    "darauf",
    "dass",
    "davon",
    "dazu",
    "denn",
    "deshalb",
    "durch",
    "eben",
    "eher",
    "eigentlich",
    "einfach",
    "einmal",
    "etwa",
    "etwas",
    "gab",
    "geben",
    "gemacht",
    "gerade",
    "gibt",
    "ging",
    "gleich",
    "gut",
    "gute",
    "haben",
    "hier",
    "hinter",
    "immer",
    "jede",
    "jeden",
    "jetzt",
    "kann",
    "kannst",
    "keine",
    "kommt",
    "können",
    "lassen",
    "machen",
    "macht",
    "mal",
    "muss",
    "müssen",
    "nachdem",
    "neben",
    "neue",
    "neuen",
    "nichts",
    "oben",
    "ohne",
    "schon",
    "sehr",
    "seit",
    "selbst",
    "sollte",
    "sowie",
    "stehen",
    "steht",
    "viel",
    "viele",
    "vielleicht",
    "wann",
    "warum",
    "weil",
    "weiter",
    "weitere",
    "welche",
    "wenig",
    "weniger",
    "wieder",
    "wirklich",
    "wobei",
    "wollen",
    "worden",
    "wurde",
    "wurden",
    "zwar",
    "zwischen",
    // English
    "also",
    "another",
    "because",
    "been",
    "before",
    "being",
    "between",
    "both",
    "could",
    "does",
    "done",
    "each",
    "even",
    "every",
    "får",
    "get",
    "gets",
    "give",
    "good",
    "great",
    "here",
    "into",
    "just",
    "keep",
    "know",
    "like",
    "made",
    "make",
    "makes",
    "many",
    "much",
    "must",
    "need",
    "needs",
    "only",
    "other",
    "over",
    "same",
    "should",
    "since",
    "some",
    "still",
    "such",
    "sure",
    "take",
    "them",
    "very",
    "want",
    "well",
    "were",
    "what",
    "where",
    "while",
    "would",
    "your",
];

/// Endings that mark case, number or a weak adjective and nothing else. Order
/// matters: the longest that still leaves a real stem wins.
const SUFFIXES: &[&str] = &[
    "ernes", "ernen", "ernem", "erner", "ens", "ern", "est", "em", "en", "er", "es", "e", "n", "s",
];

/// A stem shorter than this is not a stem, it is a coincidence. Four also
/// happens to be what keeps English intact: "notes" folds to "note", while
/// "note" itself is left alone because dropping its "e" would leave three.
const MIN_STEM: usize = 4;

/// Fold a word to the key it is grouped under.
///
/// NFC first, for the same reason note names need it: a vault carried over
/// from an older Mac holds decomposed umlauts that look identical and compare
/// unequal. Then umlauts are flattened — "Häuser" and "Haus" are one word and
/// only agree once `ä` becomes `a` — and one inflectional ending is removed.
///
/// This is deliberately not a real stemmer. A real one needs a dictionary,
/// gets German compounds wrong anyway, and would trade a visible small error
/// for an invisible large one.
pub fn fold(word: &str) -> String {
    let base: String = word.nfc().collect::<String>().to_lowercase();
    let flat: String = base
        .chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            c => c,
        })
        .collect();
    let flat = flat.replace('ß', "ss");
    for suffix in SUFFIXES {
        if let Some(stem) = flat.strip_suffix(suffix) {
            if stem.chars().count() >= MIN_STEM {
                return stem.to_string();
            }
        }
    }
    flat
}

fn is_filler(lower: &str) -> bool {
    FILLER.contains(&lower) || crate::related::is_stopword(lower)
}

/// Split note text into `(surface, folded)` pairs in reading order.
///
/// Order is kept because co-occurrence is about nearness, and the surface form
/// is kept because a node labelled "notiz" when you always write "Notizen"
/// reads like a bug. YAML frontmatter and fenced code are skipped: `author: ai`
/// is bookkeeping, and a code sample would contribute its language's keywords
/// as if they were your ideas.
fn terms(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut in_frontmatter = text.starts_with("---");
    let mut in_code = false;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if in_frontmatter {
            if i > 0 && (trimmed == "---" || trimmed == "...") {
                in_frontmatter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        for raw in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
            let surface = raw.trim();
            if surface.chars().count() < 3 {
                continue;
            }
            // A bare number is a date or a page count, never a subject.
            if surface.chars().all(|c| c.is_numeric()) {
                continue;
            }
            let lower = surface.nfc().collect::<String>().to_lowercase();
            if is_filler(&lower) {
                continue;
            }
            out.push((surface.to_string(), fold(surface)));
        }
    }
    out
}

/// How the graph is cut down to something a person can look at.
#[derive(Clone, Copy)]
pub struct ConceptOptions {
    /// Terms kept, most frequent first. Beyond a couple of hundred nodes a
    /// force layout is a cloud, not a picture.
    pub max_nodes: usize,
    /// A term appearing fewer times than this is noise, not a theme.
    pub min_count: usize,
    /// How many words apart two terms may be and still count as co-occurring.
    pub window: usize,
    /// Edges below this weight are dropped — one shared sentence is a
    /// coincidence.
    pub min_edge: usize,
}

impl Default for ConceptOptions {
    fn default() -> Self {
        Self {
            max_nodes: 150,
            min_count: 3,
            window: 4,
            min_edge: 2,
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConceptNode {
    /// The folded stem — stable, and what edges refer to.
    pub id: String,
    /// The spelling this term is written with most often in the vault.
    pub label: String,
    /// Total occurrences across the vault; drives node size.
    pub weight: usize,
    /// Index of the cluster this term was grouped into.
    pub cluster: usize,
    /// How many notes contain the term.
    pub note_count: usize,
    /// A sample of those notes, for a tooltip. Capped — a common term is in
    /// hundreds of notes and the graph does not need to carry all of them.
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConceptEdge {
    pub source: String,
    pub target: String,
    /// Times the two terms appeared within the window of each other.
    pub weight: usize,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConceptGraph {
    pub nodes: Vec<ConceptNode>,
    pub edges: Vec<ConceptEdge>,
    /// Terms that met `min_count` but did not fit in `max_nodes`. Reported
    /// rather than dropped quietly, so a trimmed graph never passes for a
    /// complete one.
    pub omitted: usize,
}

const NOTE_SAMPLE: usize = 25;

/// Build the concept graph for a whole vault.
pub fn concept_graph(
    vault: &Path,
    exclude: &[String],
    opts: ConceptOptions,
) -> std::io::Result<ConceptGraph> {
    let hidden: Vec<String> = exclude
        .iter()
        .map(|f| f.trim_matches('/').to_lowercase())
        .filter(|f| !f.is_empty())
        .collect();

    // Interned term ids keep the whole vault's token streams in memory as
    // numbers, so co-occurrence can be counted in a second pass without
    // reading every note twice.
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut surfaces: Vec<HashMap<String, usize>> = Vec::new();
    let mut in_notes: Vec<Vec<String>> = Vec::new();
    let mut streams: Vec<Vec<u32>> = Vec::new();

    for meta in vault::list_notes(vault)? {
        let lower = meta.path.to_lowercase();
        if hidden
            .iter()
            .any(|h| lower == *h || lower.starts_with(&format!("{h}/")))
        {
            continue;
        }
        let Ok(note) = vault::read_note(vault, &meta.path) else {
            continue;
        };
        let mut stream = Vec::new();
        let mut seen_here: Vec<u32> = Vec::new();
        for (surface, folded) in terms(&note.content) {
            let id = match ids.get(&folded) {
                Some(id) => *id,
                None => {
                    let id = counts.len() as u32;
                    ids.insert(folded.clone(), id);
                    counts.push(0);
                    surfaces.push(HashMap::new());
                    in_notes.push(Vec::new());
                    id
                }
            };
            counts[id as usize] += 1;
            *surfaces[id as usize].entry(surface).or_insert(0) += 1;
            if !seen_here.contains(&id) {
                seen_here.push(id);
                in_notes[id as usize].push(meta.path.clone());
            }
            stream.push(id);
        }
        streams.push(stream);
    }

    // Keep the most frequent terms. Ties break on the stem so the same vault
    // always produces the same graph.
    let by_stem: HashMap<u32, &String> = ids.iter().map(|(k, v)| (*v, k)).collect();
    let mut ranked: Vec<u32> = (0..counts.len() as u32)
        .filter(|id| counts[*id as usize] >= opts.min_count)
        .collect();
    ranked.sort_by(|a, b| {
        counts[*b as usize]
            .cmp(&counts[*a as usize])
            .then_with(|| by_stem[a].cmp(by_stem[b]))
    });
    let omitted = ranked.len().saturating_sub(opts.max_nodes);
    ranked.truncate(opts.max_nodes);

    let mut rank_of: HashMap<u32, usize> = HashMap::new();
    for (slot, id) in ranked.iter().enumerate() {
        rank_of.insert(*id, slot);
    }

    // Second pass: co-occurrence among the kept terms only.
    let mut pairs: HashMap<(usize, usize), usize> = HashMap::new();
    for stream in &streams {
        let kept: Vec<usize> = stream
            .iter()
            .filter_map(|id| rank_of.get(id).copied())
            .collect();
        for (i, a) in kept.iter().enumerate() {
            let upto = (i + opts.window).min(kept.len().saturating_sub(1));
            for b in &kept[i + 1..=upto.max(i)] {
                if a == b {
                    continue;
                }
                let key = if a < b { (*a, *b) } else { (*b, *a) };
                *pairs.entry(key).or_insert(0) += 1;
            }
        }
    }
    pairs.retain(|_, w| *w >= opts.min_edge);

    let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); ranked.len()];
    for ((a, b), w) in &pairs {
        adjacency[*a].push((*b, *w));
        adjacency[*b].push((*a, *w));
    }
    let clusters = label_propagation(&adjacency);

    let mut nodes: Vec<ConceptNode> = ranked
        .iter()
        .enumerate()
        .map(|(slot, id)| {
            let i = *id as usize;
            let label = surfaces[i]
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(s, _)| s.clone())
                .unwrap_or_else(|| by_stem[id].clone());
            ConceptNode {
                id: by_stem[id].clone(),
                label,
                weight: counts[i],
                cluster: clusters[slot],
                note_count: in_notes[i].len(),
                notes: in_notes[i].iter().take(NOTE_SAMPLE).cloned().collect(),
            }
        })
        .collect();
    nodes.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.id.cmp(&b.id)));

    let mut edges: Vec<ConceptEdge> = pairs
        .iter()
        .map(|((a, b), w)| ConceptEdge {
            source: by_stem[&ranked[*a]].clone(),
            target: by_stem[&ranked[*b]].clone(),
            weight: *w,
        })
        .collect();
    edges.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.target.cmp(&b.target))
    });

    Ok(ConceptGraph {
        nodes,
        edges,
        omitted,
    })
}

/// Group nodes by label propagation: everyone repeatedly adopts whichever
/// label carries the most edge weight among their neighbours.
///
/// Chosen over Louvain because it is short enough to read, needs no tuning
/// parameter, and the difference does not show at this graph size. It is
/// normally randomised; here the order is fixed and ties break on the lower
/// label, so the same vault always draws the same clusters — a graph that
/// re-colours itself on every open would look broken.
fn label_propagation(adjacency: &[Vec<(usize, usize)>]) -> Vec<usize> {
    let mut labels: Vec<usize> = (0..adjacency.len()).collect();
    for _ in 0..20 {
        let mut changed = false;
        for node in 0..adjacency.len() {
            if adjacency[node].is_empty() {
                continue;
            }
            let mut weight_by_label: HashMap<usize, usize> = HashMap::new();
            for (other, w) in &adjacency[node] {
                *weight_by_label.entry(labels[*other]).or_insert(0) += w;
            }
            let best = weight_by_label
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(l, _)| *l)
                .unwrap_or(labels[node]);
            if best != labels[node] {
                labels[node] = best;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Renumber so the biggest cluster is 0. Colours then stay put as the vault
    // grows, instead of every cluster shifting hue because one term was added.
    let mut sizes: HashMap<usize, usize> = HashMap::new();
    for l in &labels {
        *sizes.entry(*l).or_insert(0) += 1;
    }
    let mut order: Vec<usize> = sizes.keys().copied().collect();
    order.sort_by(|a, b| sizes[b].cmp(&sizes[a]).then_with(|| a.cmp(b)));
    let slot: HashMap<usize, usize> = order.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    labels.iter().map(|l| slot[l]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_inflection_folds_to_one_stem() {
        // The whole reason this module exists: without folding these are four
        // nodes, each with a quarter of the weight.
        for w in ["Notiz", "Notizen", "Notizes", "NOTIZEN"] {
            assert_eq!(fold(w), "notiz", "{w}");
        }
        assert_eq!(fold("Idee"), fold("Ideen"));
        assert_eq!(fold("Gedanke"), fold("Gedanken"));
        assert_eq!(fold("Wohnung"), fold("Wohnungen"));
    }

    #[test]
    fn umlaut_plurals_reach_their_singular() {
        assert_eq!(fold("Haus"), fold("Häuser"));
        assert_eq!(fold("Buch"), fold("Bücher"));
        // …and the two encodings of the same umlaut agree, as everywhere else
        // in Magma.
        assert_eq!(fold("Bücher"), fold("Bu\u{0308}cher"));
    }

    #[test]
    fn english_plurals_fold_without_eating_the_singular() {
        // "note" must not become "not": the minimum stem length is what stops
        // the German "-e" rule from wrecking English.
        assert_eq!(fold("notes"), "note");
        assert_eq!(fold("note"), "note");
        assert_eq!(fold("graph"), "graph");
    }

    #[test]
    fn short_words_are_left_whole() {
        // Folding these would leave two or three letters, which is noise.
        assert_eq!(fold("Tag"), "tag");
        assert_eq!(fold("Ende"), "ende");
    }

    #[test]
    fn frontmatter_and_code_are_not_subject_matter() {
        let text = "---\nauthor: ai\ntags: [privat]\n---\n\nDie Zinsberechnung ist wichtig.\n\n```rust\nfn zinsberechnung() { let unrelated = 1; }\n```\n";
        let found: Vec<String> = terms(text).into_iter().map(|(s, _)| s).collect();
        assert!(found.contains(&"Zinsberechnung".to_string()));
        assert!(!found.contains(&"author".to_string()), "frontmatter leaked");
        assert!(!found.contains(&"unrelated".to_string()), "code leaked");
        // The term inside the fence must not inflate the real occurrence.
        assert_eq!(found.iter().filter(|w| w.starts_with("Zins")).count(), 1);
    }

    #[test]
    fn filler_verbs_do_not_become_hubs() {
        let found: Vec<String> = terms("Man kann das machen und es gibt immer eine Zinsberechnung")
            .into_iter()
            .map(|(_, f)| f)
            .collect();
        assert_eq!(found, vec!["zinsberechnung"]);
    }

    fn tmp_vault(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("magma-concepts-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn builds_a_graph_of_terms_not_files() {
        let dir = tmp_vault("basic");
        write(
            &dir,
            "a.md",
            "Die Zinsberechnung der Hypothek ist komplex. Zinsberechnung und Hypothek gehören zusammen. Eine Hypothek braucht Zinsberechnung.",
        );
        write(
            &dir,
            "b.md",
            "Hypothek und Zinsberechnung wieder. Die Hypothek der Zinsberechnung.",
        );
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();

        let labels: Vec<&str> = g.nodes.iter().map(|n| n.label.as_str()).collect();
        assert!(labels.contains(&"Zinsberechnung"), "{labels:?}");
        assert!(labels.contains(&"Hypothek"), "{labels:?}");
        // Two terms that keep appearing together get an edge, though no
        // `[[link]]` exists anywhere in the vault.
        assert!(
            g.edges
                .iter()
                .any(|e| e.source.contains("hypothek") || e.target.contains("hypothek")),
            "{:?}",
            g.edges
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_term_written_three_ways_is_one_node() {
        // The failure this module is built to prevent, checked end to end.
        let dir = tmp_vault("folding");
        write(&dir, "a.md", "Notiz Notiz Ablage Ablage Ablage");
        write(&dir, "b.md", "Notizen Notizen Ablage Ablage");
        write(&dir, "c.md", "Der Notizen Ablage Ablage Ablage");
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();

        let notiz: Vec<&ConceptNode> = g.nodes.iter().filter(|n| n.id == "notiz").collect();
        assert_eq!(notiz.len(), 1, "one concept, one node: {:?}", g.nodes);
        assert_eq!(notiz[0].weight, 5, "all spellings counted together");
        // The label is the spelling actually used most, not the stem.
        assert_eq!(notiz[0].label, "Notizen");
        assert_eq!(notiz[0].note_count, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn separate_subjects_land_in_separate_clusters() {
        let dir = tmp_vault("clusters");
        for i in 0..3 {
            write(
                &dir,
                &format!("kochen{i}.md"),
                "Risotto Safran Risotto Safran Risotto Safran Reis Reis Reis",
            );
            write(
                &dir,
                &format!("steuer{i}.md"),
                "Umsatzsteuer Voranmeldung Umsatzsteuer Voranmeldung Umsatzsteuer Voranmeldung Finanzamt Finanzamt Finanzamt",
            );
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        // Asserted on the label, not the folded id: the stem is an internal
        // detail and the light stemmer trims some words further than a
        // linguist would ("Safran" folds to "safra"). That costs nothing as
        // long as it is consistent — which is exactly what these tests check.
        let cluster_of = |label: &str| g.nodes.iter().find(|n| n.label == label).map(|n| n.cluster);

        assert_eq!(cluster_of("Risotto"), cluster_of("Safran"));
        assert!(cluster_of("Risotto").is_some(), "{:?}", g.nodes);
        assert_eq!(cluster_of("Umsatzsteuer"), cluster_of("Voranmeldung"));
        assert_ne!(
            cluster_of("Risotto"),
            cluster_of("Umsatzsteuer"),
            "unrelated subjects must not share a cluster: {:?}",
            g.nodes
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_same_vault_always_draws_the_same_graph() {
        // Label propagation is normally randomised. A graph that re-colours
        // itself every time it opens reads as a bug, so ours must not.
        let dir = tmp_vault("stable");
        write(&dir, "a.md", "Alpha Beta Gamma Alpha Beta Gamma Alpha Beta");
        write(&dir, "b.md", "Delta Epsilon Delta Epsilon Delta Epsilon");
        let first = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        let again = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        assert_eq!(first, again);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trimmed_graph_says_how_much_it_left_out() {
        let dir = tmp_vault("omitted");
        let body: String = (0..40)
            .map(|i| format!("thema{i} thema{i} thema{i}\n"))
            .collect();
        write(&dir, "a.md", &body);
        let opts = ConceptOptions {
            max_nodes: 10,
            ..ConceptOptions::default()
        };
        let g = concept_graph(&dir, &[], opts).unwrap();
        assert_eq!(g.nodes.len(), 10);
        assert_eq!(g.omitted, 30, "silently dropping 30 terms would be a lie");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn excluded_folders_stay_out() {
        let dir = tmp_vault("exclude");
        std::fs::create_dir_all(dir.join("Vorlagen")).unwrap();
        write(&dir, "a.md", "Hypothek Hypothek Hypothek");
        write(
            &dir.join("Vorlagen"),
            "t.md",
            "Platzhalter Platzhalter Platzhalter",
        );
        let g = concept_graph(&dir, &["Vorlagen".into()], ConceptOptions::default()).unwrap();
        assert!(g.nodes.iter().any(|n| n.id == "hypothek"));
        assert!(
            !g.nodes.iter().any(|n| n.label.starts_with("Platzhalter")),
            "template folder leaked into the graph"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod speed {
    use super::*;
    use crate::vault;

    /// The expensive shape is a big vocabulary, not a big word count: pair
    /// counting grows with how many distinct terms survive the frequency
    /// floor, and the node cap only binds once there are more than 150 of
    /// them. So this vault has 400 subjects, inflected three ways each, which
    /// is also what makes the folding do real work.
    ///
    /// Measured on this vault: 95 ms release, 602 ms debug. The 1500 ms bound
    /// sits comfortably above the debug figure so a loaded CI runner will not
    /// trip it, and far below the seconds it would take if the second pass
    /// started counting pairs over the whole vocabulary instead of the kept
    /// terms.
    #[test]
    fn a_large_german_vault_stays_interactive() {
        let mut v = std::env::temp_dir();
        v.push("magma-concepts-speed");
        let _ = std::fs::remove_dir_all(&v);
        std::fs::create_dir_all(&v).unwrap();
        let forms = ["", "en", "s"];
        for i in 0..800 {
            let body: String = (0..80)
                .map(|j| {
                    let w = (i * 7 + j * 13) % 400;
                    format!("Thematik{w}{} ", forms[j % forms.len()])
                })
                .collect();
            vault::write_note(&v, &format!("Notizen/Notiz {i}.md"), &body).unwrap();
        }
        let start = std::time::Instant::now();
        let g = concept_graph(&v, &[], ConceptOptions::default()).unwrap();
        let ms = start.elapsed().as_millis();
        std::fs::remove_dir_all(&v).ok();

        assert_eq!(g.nodes.len(), 150, "the node cap should bind here");
        assert!(g.omitted > 0, "and it should say so");
        assert!(
            ms < 1500,
            "800 notes over 400 subjects took {ms} ms — the concept graph is \
             no longer something you can open on a whim"
        );
    }
}
