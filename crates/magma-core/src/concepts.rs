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

/// Blank out anything that looks like a URL or a bare domain.
fn strip_urls(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for chunk in line.split_whitespace() {
        let lower = chunk.to_lowercase();
        let is_url = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.")
            || (lower.contains('/') && lower.contains('.') && !lower.starts_with('/'));
        if !is_url {
            out.push_str(chunk);
        }
        out.push(' ');
    }
    out
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
fn terms(text: &str) -> Vec<Term> {
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
        // A pasted link contributes "https", "www", "com" and every path
        // segment of somebody else's site. None of that is what the note is
        // about, and all of it is frequent enough to survive any filter.
        let line = strip_urls(line);
        // Position matters now, so the line is walked rather than split: a word
        // is only evidence of being a noun if it is capitalised *mid*-sentence.
        let mut opener = true;
        let mut word = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i <= chars.len() {
            let c = if i < chars.len() { chars[i] } else { ' ' };
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else {
                if !word.is_empty() {
                    push_term(&mut out, &word, !opener);
                    opener = false;
                    word.clear();
                }
                // A new sentence starts after terminal punctuation, and every
                // list bullet or heading marker opens one too.
                if ".!?:;•*#>|-–—".contains(c) {
                    opener = true;
                }
            }
            i += 1;
        }
    }
    out
}

/// One occurrence of a word, with the two things the graph needs to know
/// about it beyond the word itself.
struct Term {
    surface: String,
    folded: String,
    /// True when the word did not open a sentence. Only these occurrences say
    /// anything about capitalisation: at the start of a sentence, every word
    /// is capitalised and the signal is worthless.
    mid_sentence: bool,
}

fn push_term(out: &mut Vec<Term>, surface: &str, mid_sentence: bool) {
    if surface.chars().count() < 3 || surface.chars().all(|c| c.is_numeric()) {
        return;
    }
    let lower = surface.nfc().collect::<String>().to_lowercase();
    if is_filler(&lower) {
        return;
    }
    out.push(Term {
        surface: surface.to_string(),
        folded: fold(surface),
        mid_sentence,
    });
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
    /// Drop any term appearing in more than this share of notes.
    ///
    /// This is the load-bearing filter, and the reason the stopword lists
    /// above stay short. A hand-written list of function words is a bucket
    /// with no bottom: German alone will supply "willst", "hast", "täglich",
    /// "ganz", "jemand" for as long as you keep adding to it. But grammar has
    /// a signature no subject matter has — it turns up in nearly every note.
    /// Measuring that needs no dictionary and works in any language.
    pub max_doc_ratio: f32,
    /// Require a term to read like a noun before it counts as subject matter.
    ///
    /// German capitalises its nouns, and subject matter is overwhelmingly
    /// nouns — which is why a real vault filled up with "hätte", "willst",
    /// "brauchst" and "passiert" while every actual topic in the same picture
    /// (Infrastruktur, Vertrauen, Entscheidungen, Plattform) was capitalised.
    /// Frequency filters cannot separate those; orthography can.
    ///
    /// Only occurrences away from the start of a sentence count, since there
    /// everything is capitalised. And the rule disables itself on a vault that
    /// does not work this way — English capitalises almost nothing, so too few
    /// terms qualifying is read as "wrong language" rather than "empty graph".
    pub require_noun_case: bool,
    /// Strongest edges kept per term.
    ///
    /// Without this the graph is a hairball: everything frequent co-occurs
    /// with everything else frequent, every node ends up connected to every
    /// other, clustering collapses into one blob and the layout has thousands
    /// of springs to solve per frame. Keeping each term's strongest few
    /// connections leaves a graph with a shape.
    pub max_edges_per_node: usize,
}

impl Default for ConceptOptions {
    fn default() -> Self {
        Self {
            max_nodes: 150,
            min_count: 3,
            window: 4,
            min_edge: 2,
            max_doc_ratio: 0.35,
            require_noun_case: true,
            max_edges_per_node: 6,
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
    // Capitalisation evidence, counted only away from sentence starts.
    let mut mid_total: Vec<usize> = Vec::new();
    let mut mid_caps: Vec<usize> = Vec::new();

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
        for term in terms(&note.content) {
            let id = match ids.get(&term.folded) {
                Some(id) => *id,
                None => {
                    let id = counts.len() as u32;
                    ids.insert(term.folded.clone(), id);
                    counts.push(0);
                    surfaces.push(HashMap::new());
                    in_notes.push(Vec::new());
                    mid_total.push(0);
                    mid_caps.push(0);
                    id
                }
            };
            counts[id as usize] += 1;
            if term.mid_sentence {
                mid_total[id as usize] += 1;
                if term
                    .surface
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_uppercase())
                {
                    mid_caps[id as usize] += 1;
                }
            }
            *surfaces[id as usize].entry(term.surface).or_insert(0) += 1;
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
    // Grammar turns up nearly everywhere; subject matter does not. Dropping
    // terms above a share of the vault removes "willst", "täglich" and "ganz"
    // without anyone having to list them, and does it in whatever language the
    // notes happen to be written in.
    let doc_ceiling = ((streams.len() as f32) * opts.max_doc_ratio).ceil() as usize;
    let candidates: Vec<u32> = (0..counts.len() as u32)
        .filter(|id| {
            let i = *id as usize;
            counts[i] >= opts.min_count && (streams.len() < 8 || in_notes[i].len() <= doc_ceiling)
        })
        .collect();
    // Reads like a noun: capitalised in most of the places it appears away
    // from a sentence start. A term never seen mid-sentence has cast no vote
    // and is kept rather than punished for it.
    let noun_like = |id: &u32| -> bool {
        let i = *id as usize;
        mid_total[i] == 0 || mid_caps[i] * 2 >= mid_total[i]
    };
    let nouns: Vec<u32> = candidates.iter().copied().filter(noun_like).collect();
    // A vault that does not capitalise its nouns — English, mostly — would be
    // emptied by the rule, so it switches itself off instead of switching the
    // graph off.
    let use_case = opts.require_noun_case && nouns.len() * 4 >= candidates.len();
    let mut ranked: Vec<u32> = if use_case { nouns } else { candidates };
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
    sparsify(&mut pairs, &ranked, &counts, opts.max_edges_per_node);

    let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); ranked.len()];
    for ((a, b), w) in &pairs {
        adjacency[*a].push((*b, *w));
        adjacency[*b].push((*a, *w));
    }
    let clusters = louvain(&adjacency);

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

/// Keep only each term's strongest connections.
///
/// Raw co-occurrence counts favour whatever is frequent: two common words
/// share sentences often simply because both are everywhere. Ranking by
/// `w / sqrt(count_a * count_b)` asks instead how much of each term's life is
/// spent next to the other, which is what "these belong together" means.
///
/// An edge survives if *either* end considers it one of its strongest. That
/// keeps a small specific term attached to the big one it hangs off, instead
/// of stranding it because the big term has more interesting company.
fn sparsify(
    pairs: &mut HashMap<(usize, usize), usize>,
    ranked: &[u32],
    counts: &[usize],
    per_node: usize,
) {
    let strength = |a: usize, b: usize, w: usize| -> f64 {
        let ca = counts[ranked[a] as usize] as f64;
        let cb = counts[ranked[b] as usize] as f64;
        w as f64 / (ca * cb).sqrt().max(1.0)
    };
    let mut best: Vec<Vec<(f64, (usize, usize))>> = vec![Vec::new(); ranked.len()];
    for ((a, b), w) in pairs.iter() {
        let s = strength(*a, *b, *w);
        best[*a].push((s, (*a, *b)));
        best[*b].push((s, (*a, *b)));
    }
    let mut keep: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for edges in best.iter_mut() {
        edges.sort_by(|x, y| {
            y.0.partial_cmp(&x.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| x.1.cmp(&y.1))
        });
        for (_, key) in edges.iter().take(per_node) {
            keep.insert(*key);
        }
    }
    pairs.retain(|key, _| keep.contains(key));
}

/// Group nodes into communities by Louvain modularity optimisation.
///
/// This replaced label propagation, which was the wrong tool and was chosen on
/// an assumption that a real vault disproved: on a connected graph — which a
/// vault's vocabulary always is — propagation converges on a single label and
/// reports one topic for everything. Modularity does not have that failure
/// mode, because it asks whether a grouping beats chance rather than who
/// shouts loudest.
///
/// Deterministic on purpose: nodes are visited in index order and ties keep
/// the current community, so the same vault always draws the same colours. A
/// graph that re-groups itself on every open reads as a bug.
fn louvain(adjacency: &[Vec<(usize, usize)>]) -> Vec<usize> {
    let n = adjacency.len();
    if n == 0 {
        return Vec::new();
    }
    // `node_of` maps original nodes to the current level's super-nodes, so the
    // answer can be expanded back down once the levels stop improving.
    let mut node_of: Vec<usize> = (0..n).collect();
    let mut level: Vec<Vec<(usize, f64)>> = adjacency
        .iter()
        .map(|es| es.iter().map(|(j, w)| (*j, *w as f64)).collect())
        .collect();
    let mut self_loops: Vec<f64> = vec![0.0; n];

    loop {
        let size = level.len();
        let two_m: f64 = level
            .iter()
            .flat_map(|es| es.iter().map(|(_, w)| *w))
            .sum::<f64>()
            + self_loops.iter().sum::<f64>() * 2.0;
        if two_m <= 0.0 {
            break;
        }
        let degree: Vec<f64> = (0..size)
            .map(|i| level[i].iter().map(|(_, w)| *w).sum::<f64>() + 2.0 * self_loops[i])
            .collect();

        let mut community: Vec<usize> = (0..size).collect();
        let mut tot: Vec<f64> = degree.clone();
        let mut moved = false;

        for _ in 0..20 {
            let mut changed = false;
            for i in 0..size {
                let from = community[i];
                tot[from] -= degree[i];
                // Weight from `i` into each neighbouring community.
                let mut into: HashMap<usize, f64> = HashMap::new();
                for (j, w) in &level[i] {
                    if *j != i {
                        *into.entry(community[*j]).or_insert(0.0) += *w;
                    }
                }
                let gain = |c: usize| -> f64 {
                    into.get(&c).copied().unwrap_or(0.0) - tot[c] * degree[i] / two_m
                };
                let mut best = from;
                let mut best_gain = gain(from);
                let mut targets: Vec<usize> = into.keys().copied().collect();
                targets.sort_unstable();
                for c in targets {
                    let g = gain(c);
                    if g > best_gain + 1e-12 {
                        best_gain = g;
                        best = c;
                    }
                }
                tot[best] += degree[i];
                if best != from {
                    community[i] = best;
                    changed = true;
                    moved = true;
                }
            }
            if !changed {
                break;
            }
        }
        if !moved {
            break;
        }

        // Renumber the communities found at this level, then fold the graph so
        // each becomes one node and the next level can group *those*.
        let mut slot: HashMap<usize, usize> = HashMap::new();
        for c in &community {
            let next = slot.len();
            slot.entry(*c).or_insert(next);
        }
        let compact: Vec<usize> = community.iter().map(|c| slot[c]).collect();
        for owner in node_of.iter_mut() {
            *owner = compact[*owner];
        }

        let groups = slot.len();
        let mut folded: Vec<HashMap<usize, f64>> = vec![HashMap::new(); groups];
        let mut folded_loops = vec![0.0; groups];
        for i in 0..size {
            folded_loops[compact[i]] += self_loops[i];
            for (j, w) in &level[i] {
                let (a, b) = (compact[i], compact[*j]);
                if a == b {
                    // Each internal edge is seen from both ends.
                    folded_loops[a] += w / 2.0;
                } else {
                    *folded[a].entry(b).or_insert(0.0) += *w;
                }
            }
        }
        if groups == size {
            break; // nothing merged; another level would repeat this one
        }
        level = folded
            .into_iter()
            .map(|m| {
                let mut es: Vec<(usize, f64)> = m.into_iter().collect();
                es.sort_unstable_by_key(|(j, _)| *j);
                es
            })
            .collect();
        self_loops = folded_loops;
    }

    // Biggest community first, so colours stay put as a vault grows instead of
    // every group shifting hue because one term was added.
    let mut sizes: HashMap<usize, usize> = HashMap::new();
    for c in &node_of {
        *sizes.entry(*c).or_insert(0) += 1;
    }
    let mut order: Vec<usize> = sizes.keys().copied().collect();
    order.sort_by(|a, b| sizes[b].cmp(&sizes[a]).then_with(|| a.cmp(b)));
    let slot: HashMap<usize, usize> = order.iter().enumerate().map(|(i, c)| (*c, i)).collect();
    node_of.iter().map(|c| slot[c]).collect()
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
        let found: Vec<String> = terms(text).into_iter().map(|t| t.surface).collect();
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
            .map(|t| t.folded)
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
    fn words_that_are_everywhere_are_grammar_and_get_dropped() {
        // The failure a real vault showed: the graph filled up with "willst",
        // "täglich", "ganz" and "jemand" — none of them on any stopword list,
        // all of them in nearly every note. No list will ever be complete, so
        // the share of notes a word appears in has to do the work.
        let dir = tmp_vault("everywhere");
        let subjects = ["Hypothek", "Risotto", "Umsatzsteuer", "Fahrrad", "Klavier"];
        for (i, subject) in subjects.iter().enumerate() {
            for n in 0..4 {
                write(
                    &dir,
                    &format!("note{i}{n}.md"),
                    // The chatter appears in every note; the subject in a fifth.
                    &format!(
                        "täglich willst ganz jemand {subject} {subject} {subject} täglich willst ganz jemand"
                    ),
                );
            }
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        let labels: Vec<&str> = g.nodes.iter().map(|n| n.label.as_str()).collect();

        for chatter in ["täglich", "willst", "ganz", "jemand"] {
            assert!(
                !labels.iter().any(|l| l.eq_ignore_ascii_case(chatter)),
                "{chatter:?} is in every note — that makes it grammar, not a subject: {labels:?}"
            );
        }
        assert!(labels.contains(&"Hypothek"), "{labels:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pasted_links_do_not_become_subjects() {
        // "www", "com" and "uploads" showed up as concepts in a real vault.
        let dir = tmp_vault("urls");
        write(
            &dir,
            "a.md",
            "Zur Hypothek siehe https://www.example.com/wp-content/uploads/zinsen.pdf und \
             www.beispiel.de/rechner — die Hypothek bleibt das Thema. Hypothek.",
        );
        let g = concept_graph(
            &dir,
            &[],
            ConceptOptions {
                min_count: 1,
                ..ConceptOptions::default()
            },
        )
        .unwrap();
        let labels: Vec<&str> = g.nodes.iter().map(|n| n.label.as_str()).collect();
        for junk in ["www", "com", "uploads", "https", "example"] {
            assert!(
                !labels.iter().any(|l| l.eq_ignore_ascii_case(junk)),
                "{junk:?} came out of a URL, not out of a thought: {labels:?}"
            );
        }
        assert!(labels.contains(&"Hypothek"), "{labels:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bleeding_vault_does_not_become_a_hairball() {
        // The other half of what a real vault showed: with nothing pruned,
        // every frequent term ends up wired to every other, clustering
        // collapses into one blob, and the layout has thousands of springs to
        // solve per frame — which is what "it stutters" meant.
        //
        // What produces it is bleed: notes mostly about one thing that mention
        // the others in passing. Ten subjects here rather than four, so no
        // single one is in enough notes to look like grammar.
        let dir = tmp_vault("hairball");
        let groups = [
            ["Hypothek", "Tilgung", "Grundbuch"],
            ["Risotto", "Safran", "Parmesan"],
            ["Umsatzsteuer", "Voranmeldung", "Finanzamt"],
            ["Fahrrad", "Kette", "Bremse"],
            ["Gitarre", "Saite", "Stimmung"],
            ["Garten", "Beet", "Kompost"],
            ["Kamera", "Blende", "Belichtung"],
            ["Reise", "Fahrplan", "Gepäck"],
            ["Vertrag", "Kündigung", "Frist"],
            ["Rechner", "Speicher", "Kühlung"],
        ];
        for (gi, group) in groups.iter().enumerate() {
            for n in 0..6 {
                let mut body = String::new();
                for _ in 0..4 {
                    body.push_str(&format!("{} {} {} ", group[0], group[1], group[2]));
                }
                // A passing mention of two neighbours — enough to connect
                // everything to everything if nothing prunes it.
                for step in 1..=2 {
                    let other = &groups[(gi + step) % groups.len()];
                    body.push_str(&format!("{} ", other[n % other.len()]));
                }
                write(&dir, &format!("g{gi}n{n}.md"), &body);
            }
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();

        assert!(
            g.nodes.len() >= 20,
            "subjects were filtered away: {:?}",
            g.nodes
        );
        let per_node = g.edges.len() as f32 / g.nodes.len() as f32;
        assert!(
            per_node <= 6.0,
            "{per_node:.1} edges per node is a hairball, not a picture"
        );
        let clusters: std::collections::HashSet<usize> =
            g.nodes.iter().map(|n| n.cluster).collect();
        assert!(
            clusters.len() >= 3,
            "ten subjects collapsed into {} cluster(s)",
            clusters.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn german_verbs_lose_to_german_nouns() {
        // What a real vault filled up with: "hätte", "willst", "brauchst",
        // "passiert" — under the everywhere-threshold, on no stopword list,
        // and useless. Every actual topic in the same picture was capitalised,
        // because German capitalises its nouns. That is the signal.
        //
        // The verbs sit in four of twenty notes — a fifth of the vault, well
        // under the everywhere-threshold — and the subjects in four each. So
        // frequency cannot separate them here. Only case can, which is what
        // makes this a test of the rule rather than of the filter above it.
        let dir = tmp_vault("nouncase");
        let groups = [
            ["Infrastruktur", "Plattform", "Vertrauen"],
            ["Hypothek", "Tilgung", "Grundbuch"],
            ["Risotto", "Safran", "Parmesan"],
            ["Kamera", "Blende", "Belichtung"],
            ["Gitarre", "Saite", "Stimmung"],
        ];
        for (gi, group) in groups.iter().enumerate() {
            for n in 0..4 {
                let mut body = String::new();
                for _ in 0..4 {
                    body.push_str(&format!("{} {} {} ", group[0], group[1], group[2]));
                }
                // Chatter, mid-sentence and lowercase, in under a third of the
                // vault — exactly the case the frequency filter cannot catch.
                if gi < 1 {
                    body.push_str(
                        "Man hätte das anders bauen können, wenn du willst; du \
                         brauchst es aber. Was passiert dann, hätte, willst, \
                         brauchst, passiert.",
                    );
                }
                write(&dir, &format!("g{gi}n{n}.md"), &body);
            }
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        let labels: Vec<&str> = g.nodes.iter().map(|n| n.label.as_str()).collect();
        assert!(!labels.is_empty(), "graph came back empty");

        for verb in ["hätte", "willst", "brauchst", "passiert"] {
            assert!(
                !labels.iter().any(|l| l.eq_ignore_ascii_case(verb)),
                "{verb:?} is a verb, not a subject: {labels:?}"
            );
        }
        for noun in ["Infrastruktur", "Plattform", "Vertrauen"] {
            assert!(labels.contains(&noun), "{noun} is the subject: {labels:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_english_vault_is_not_emptied_by_the_noun_rule() {
        // English capitalises almost nothing, so the rule has to notice it does
        // not apply here rather than hand back an empty graph.
        let dir = tmp_vault("english");
        let groups = [
            ["mortgage", "repayment", "schedule"],
            ["risotto", "saffron", "parmesan"],
            ["camera", "aperture", "exposure"],
            ["guitar", "strings", "tuning"],
            ["bicycle", "chain", "brakes"],
        ];
        for (gi, group) in groups.iter().enumerate() {
            for n in 0..4 {
                let mut body = String::new();
                for _ in 0..4 {
                    body.push_str(&format!("{} {} {} ", group[0], group[1], group[2]));
                }
                write(&dir, &format!("g{gi}n{n}.md"), &body);
            }
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        let labels: Vec<String> = g.nodes.iter().map(|n| n.label.to_lowercase()).collect();
        for noun in ["mortgage", "repayment", "schedule"] {
            assert!(
                labels.iter().any(|l| l.starts_with(&noun[..5])),
                "{noun} vanished — the noun-case rule did not switch itself off: {labels:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_connected_vault_is_still_split_into_topics() {
        // A guard that topics come back separate at all.
        //
        // Stated plainly, because it would be easy to read more into it: this
        // does *not* prove Louvain beats label propagation. Propagation passes
        // it too. The collapse that motivated the switch needs the hundreds of
        // heavily cross-linked terms a real vault has, and every attempt to
        // build that here ran into the everywhere-filter instead — bridge the
        // groups hard enough for propagation to drown and every term is in
        // every note, so nothing survives to cluster.
        //
        // The evidence for Louvain is a real 600-note vault whose legend read
        // "Thema 1" and nothing else, twice, plus propagation's known
        // behaviour on connected graphs. This test only holds the floor.
        let dir = tmp_vault("connected");
        let groups = [
            ["Hypothek", "Tilgung", "Grundbuch"],
            ["Risotto", "Safran", "Parmesan"],
            ["Umsatzsteuer", "Voranmeldung", "Finanzamt"],
            ["Fahrrad", "Kette", "Bremse"],
            ["Gitarre", "Saite", "Stimmung"],
            ["Kamera", "Blende", "Belichtung"],
        ];
        for (gi, group) in groups.iter().enumerate() {
            for n in 0..6 {
                let mut body = String::new();
                for _ in 0..4 {
                    body.push_str(&format!("{} {} {} ", group[0], group[1], group[2]));
                }
                // The bridge that makes the whole graph one component.
                let next = &groups[(gi + 1) % groups.len()];
                body.push_str(&format!("{} ", next[n % next.len()]));
                write(&dir, &format!("g{gi}n{n}.md"), &body);
            }
        }
        let g = concept_graph(&dir, &[], ConceptOptions::default()).unwrap();
        let clusters: std::collections::HashSet<usize> =
            g.nodes.iter().map(|n| n.cluster).collect();
        assert!(
            clusters.len() >= 4,
            "six subjects came back as {} topic(s) — the legend would read \
             \"Thema 1\" and nothing else: {:?}",
            clusters.len(),
            g.nodes
        );
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
