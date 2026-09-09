//! Local sentence embeddings: the model half of M8's hybrid retrieval.
//!
//! Kept out of `magma-core` on purpose. This crate pulls in candle and a
//! tokenizer, some 146 dependencies and about six megabytes of linked code, and
//! `magma-core` is what the MCP server, the importer and the tests build on.
//! Only the desktop shell needs a model, so only the desktop shell pays.
//!
//! Everything here is optional at runtime. With no model on disk, retrieval
//! stays lexical and Magma works exactly as it did — see
//! [`magma_core::retrieve_with`], which takes the model as an `Option`.
//!
//! **Nothing below the trait boundary has been run against real weights.**
//! It compiles, its cache is tested, and its shape follows the model card; but
//! huggingface.co was unreachable from where it was written, so the first
//! genuine proof that the encoder loads and produces sane German vectors is a
//! real machine downloading a real model.

mod cache;
mod download;
mod model;
mod reranker;

pub use cache::Cached;
pub use download::{
    ensure_model, ensure_rerank, probe_size, DownloadProgress, ModelFiles, ModelSpec, RerankFiles,
    DEFAULT_MODEL, RERANK_MODEL,
};
pub use model::Embedder;
pub use reranker::Reranker;

use magma_core::Similarity;
use std::path::{Path, PathBuf};

/// Where the downloaded weights live under the app's data directory.
///
/// Keyed by the weights, not by the vector-space id: how text is cut before it
/// reaches the model changes the vectors but not the files, and a user should
/// not re-download half a gigabyte because a truncation limit moved.
pub fn model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(DEFAULT_MODEL.weights_dir)
}

/// The vector cache for one vault.
///
/// Per vault, because two vaults share no passages, and keyed by the vault's
/// path so switching between them does not throw the other's work away.
pub fn cache_path(app_data: &Path, vault: &Path) -> PathBuf {
    app_data
        .join("vectors")
        .join(format!("{}-{}.bin", DEFAULT_MODEL.id, vault_key(vault)))
}

/// A short, stable name for a vault path, for the cache file it owns.
///
/// FNV-1a in shape, and **not** FNV-1a in fact: the multiplier below is
/// `0x1000_0000_01b3`, while the real prime is `0x100_0000_01b3` — one nibble
/// shorter. That was a typo, and it stays. Nothing here needs FNV's properties;
/// it needs a deterministic name, and it has one. Correcting the constant would
/// rename every existing cache file, orphaning every index anyone has built —
/// and orphaning it invisibly, because the cleanup in [`discard_superseded`]
/// matches on this very suffix and would no longer recognise the old file as
/// belonging to the vault.
///
/// Recorded because it cost an hour once: a cache file name was compared
/// against a hash computed with the real prime, they differed, and that was
/// read as evidence that the app had indexed some other folder. It had not.
/// Verify against this function, never against a reimplementation of FNV.
fn vault_key(vault: &Path) -> String {
    let key = vault.to_string_lossy();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Remove this vault's vectors from spaces that are no longer used, returning
/// how many files went.
///
/// Changing how a passage is cut or labelled changes the id vectors are stamped
/// with, and with it the file they live in. The old file is then dead weight —
/// eleven megabytes for a vault this size — and nothing would ever read it
/// again or ever delete it.
///
/// Deliberately not called when opening the model, which happens at startup and
/// would make this a deletion nobody asked for. It runs at the start of an
/// indexing run, where the user has just chosen to build the new space and the
/// old one is superseded by that choice.
pub fn discard_superseded(app_data: &Path, vault: &Path) -> usize {
    let dir = app_data.join("vectors");
    let keep = cache_path(app_data, vault);
    // The vault's own hash, so another vault's index is never touched.
    let mine = format!("-{}.bin", vault_key(vault));
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut gone = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let is_mine = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(&mine));
        if is_mine && std::fs::remove_file(&path).is_ok() {
            gone += 1;
        }
    }
    gone
}

/// True when the model is already downloaded, so a caller can tell "off"
/// apart from "not fetched yet" without starting half a gigabyte of traffic.
pub fn is_ready(app_data: &Path) -> bool {
    Embedder::is_present(&model_dir(app_data))
}

/// Load the encoder for a vault, ready to use, with its cache attached.
///
/// Returns `Ok(None)` when the model has not been downloaded: that is the
/// ordinary state of a fresh install, not an error, and retrieval carries on
/// lexically.
pub fn open(app_data: &Path, vault: &Path) -> Result<Option<Cached<Embedder>>, String> {
    let dir = model_dir(app_data);
    if !Embedder::is_present(&dir) {
        return Ok(None);
    }
    let files = ModelFiles::in_dir(&dir);
    let embedder = Embedder::load(&files, DEFAULT_MODEL.id)?;
    Ok(Some(Cached::open(embedder, cache_path(app_data, vault))))
}

/// Where the reranker's weights live. Separate from the encoder's, because they
/// are separate downloads and either may be absent.
pub fn rerank_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(RERANK_MODEL.weights_dir)
}

/// True when the reranker is on disk.
pub fn rerank_ready(app_data: &Path) -> bool {
    Reranker::is_present(&rerank_dir(app_data))
}

/// Load the reranker, checking that it actually ranks before handing it over.
///
/// `Ok(None)` when it has not been downloaded — the ordinary state, and
/// retrieval simply keeps the fused order. An `Err` means it is there and
/// unusable, which is worth saying rather than silently ignoring: the user paid
/// half a gigabyte for it.
pub fn open_rerank(app_data: &Path) -> Result<Option<Reranker>, String> {
    let dir = rerank_dir(app_data);
    let Some(files) = RerankFiles::in_dir(&dir) else {
        return Ok(None);
    };
    let model = Reranker::load(&files, RERANK_MODEL.id)?;
    // Before anyone's search depends on it. A reranker has the last word on
    // the order, so one with its labels the wrong way round would put the worst
    // candidate first and still look like a working feature.
    reranker::self_check(&model)?;
    Ok(Some(model))
}

/// Download the reranker if it is not there yet.
pub fn fetch_rerank(
    app_data: &Path,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), String> {
    ensure_rerank(&rerank_dir(app_data), &RERANK_MODEL, on_progress).map(|_| ())
}

/// What each download really costs, asked of the server.
///
/// `None` means the server would not say, and a caller should show that as
/// unknown rather than quoting a number nobody measured. The figures in the
/// specs are guesses written without network access.
pub fn download_size(spec: &ModelSpec) -> Option<u64> {
    let names: &[&str] = if spec.id == RERANK_MODEL.id {
        &["model.safetensors", "pytorch_model.bin"]
    } else {
        &["model.safetensors"]
    };
    probe_size(spec, names)
}

/// How far along indexing is, for a progress bar.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub done: usize,
    pub total: usize,
}

/// Encode every passage of a vault, so searching can be a lookup.
///
/// This is the work that must not happen inside a query. It takes minutes on a
/// real vault, it reports as it goes, and the cache is written along the way —
/// so a run that is cut short still leaves everything it finished, and running
/// it again picks up where it stopped.
pub fn index_vault(
    app_data: &Path,
    vault: &Path,
    on_progress: &mut dyn FnMut(IndexProgress),
) -> Result<usize, String> {
    let model = open(app_data, vault)?.ok_or("the model has not been downloaded yet")?;
    discard_superseded(app_data, vault);
    let texts = passage_texts(vault).map_err(|e| format!("cannot read the vault: {e}"))?;
    let total = texts.len();

    // Drop vectors for passages that no longer exist, or the file only grows.
    model.retain_only(&texts);

    let mut todo = model.missing(&texts);
    // Encode similar lengths together. A batch is padded to its longest member
    // and attention costs the square of that length, so one long passage among
    // seven short ones makes the other seven cost what it does.
    todo.sort_by_key(|t| t.len());

    let mut done = total - todo.len();
    on_progress(IndexProgress { done, total });

    let lanes = lane_count();
    for group in todo.chunks(INDEX_BATCH * lanes * 4) {
        let per_lane = group.len().div_ceil(lanes);
        let (tx, rx) = std::sync::mpsc::channel::<usize>();

        // Plain scoped threads rather than a work-stealing pool. The encoder
        // parallelises inside itself; nesting one pool in another oversubscribes
        // the machine and makes progress impossible to reason about.
        let parts: Vec<Result<Vec<Vec<f32>>, String>> = std::thread::scope(|scope| {
            let workers: Vec<_> = group
                .chunks(per_lane.max(1))
                .map(|slice| {
                    let tx = tx.clone();
                    let encoder = model.inner();
                    scope.spawn(move || {
                        let mut out = Vec::with_capacity(slice.len());
                        for batch in slice.chunks(INDEX_BATCH) {
                            out.extend(encoder.embed_passages(batch)?);
                            // Report per batch, not per group. Minutes without a
                            // moving number is indistinguishable from a hang,
                            // and this project has now learned that three times.
                            let _ = tx.send(batch.len());
                        }
                        Ok(out)
                    })
                })
                .collect();
            drop(tx);

            for encoded in rx {
                done += encoded;
                on_progress(IndexProgress { done, total });
            }

            workers
                .into_iter()
                .map(|w| {
                    w.join()
                        .unwrap_or_else(|_| Err("an indexing thread died".into()))
                })
                .collect()
        });

        let mut vectors = Vec::with_capacity(group.len());
        for part in parts {
            vectors.extend(part?);
        }
        model.insert_many(group, vectors)?;
        model.save_if_due()?;
    }
    model.save()?;
    Ok(total)
}

/// How many passages are encoded side by side.
///
/// Capped well below the core count on purpose. Each lane holds its own
/// attention matrices, so lanes multiply memory as readily as they divide time,
/// and the first attempt at this — one lane per core — drove a real machine
/// into swap and looked like a freeze. Four is enough to use a laptop without
/// taking it over.
fn lane_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 4)
}

/// Passages per forward pass.
const INDEX_BATCH: usize = 4;

/// Every passage of the vault, in the exact form retrieval will ask for.
///
/// It has to be the *same* text, or the cache keys will not match and the
/// indexing run would leave a query no better off than before.
fn passage_texts(vault: &Path) -> std::io::Result<Vec<String>> {
    let mut out = Vec::new();
    for note in magma_core::list_notes(vault)? {
        let content = magma_core::vault::read_for_scan(&vault.join(&note.path));
        if content.is_empty() {
            continue;
        }
        for chunk in magma_core::chunk_note(&content) {
            out.push(magma_core::embedding_text(&note.path, &chunk));
        }
    }
    Ok(out)
}

/// Download the model if it is not there yet.
pub fn fetch(app_data: &Path, on_progress: &mut dyn FnMut(DownloadProgress)) -> Result<(), String> {
    ensure_model(&model_dir(app_data), &DEFAULT_MODEL, on_progress).map(|_| ())
}

/// Sanity check on a loaded encoder, for the moment just after a download.
///
/// Two texts that mean the same thing must land closer together than two that
/// do not. It is a crude property, but it is the one thing that distinguishes a
/// working encoder from one that loaded and returns noise — and a model that
/// returns noise degrades retrieval silently, which is the failure this project
/// keeps paying for.
pub fn self_check(model: &dyn Similarity) -> Result<(), String> {
    let vectors = model.embed_passages(&[
        "Die Notarisierung durch Apple dauert manchmal lange.".to_string(),
        "Die Beglaubigung durch den Hersteller zieht sich hin.".to_string(),
        "Rote Farbe im Graphen für den Ordner Blog.".to_string(),
    ])?;
    if vectors.len() != 3 {
        return Err("the model did not answer for every text".into());
    }
    let near = cosine(&vectors[0], &vectors[1]);
    let far = cosine(&vectors[0], &vectors[2]);
    if !(near > far) {
        return Err(format!(
            "the model does not separate meaning: related {near:.3} vs unrelated {far:.3}"
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Similarity for Fake {
        fn id(&self) -> &str {
            "fake"
        }
        fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
            Ok(vec![1.0, 0.0])
        }
        fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts
                .iter()
                .map(|t| {
                    if t.to_lowercase().contains("farbe") {
                        vec![0.0, 1.0]
                    } else {
                        vec![1.0, 0.0]
                    }
                })
                .collect())
        }
    }

    struct Noise;
    impl Similarity for Noise {
        fn id(&self) -> &str {
            "noise"
        }
        fn embed_query(&self, _: &str) -> Result<Vec<f32>, String> {
            Ok(vec![1.0, 0.0])
        }
        fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            // Every text identical: loads fine, separates nothing.
            Ok(vec![vec![1.0, 0.0]; texts.len()])
        }
    }

    #[test]
    fn the_self_check_passes_a_model_that_separates_meaning() {
        assert!(self_check(&Fake).is_ok());
    }

    #[test]
    fn the_self_check_catches_a_model_that_does_not() {
        // This is the failure worth catching: it loads, it answers, and every
        // answer is the same. Retrieval would just quietly get worse.
        let err = self_check(&Noise).unwrap_err();
        assert!(err.contains("separate meaning"), "{err}");
    }

    #[test]
    fn an_old_vector_space_is_cleared_out_but_only_for_this_vault() {
        // Changing how a passage is labelled changes the id, and with it the
        // file. Nothing would ever read the old one again and nothing would
        // ever delete it, so it sits there for good.
        let app = std::env::temp_dir().join("magma-embed-superseded");
        let _ = std::fs::remove_dir_all(&app);
        let vault = Path::new("/home/alex/Notizen");
        let other = Path::new("/home/alex/Archiv");
        std::fs::create_dir_all(app.join("vectors")).unwrap();

        let current = cache_path(&app, vault);
        let stale = app
            .join("vectors")
            .join(format!("some-older-space-{}.bin", vault_key(vault)));
        let theirs = cache_path(&app, other);
        for f in [&current, &stale, &theirs] {
            std::fs::write(f, b"x").unwrap();
        }

        assert_eq!(discard_superseded(&app, vault), 1);
        assert!(current.exists(), "the space in use must survive");
        assert!(!stale.exists(), "the superseded one must not");
        assert!(
            theirs.exists(),
            "another vault's index is none of our business"
        );

        let _ = std::fs::remove_dir_all(&app);
    }

    #[test]
    fn a_vaults_cache_name_is_pinned_to_a_known_value() {
        // Pinned deliberately. The multiplier in `vault_key` is a mistyped FNV
        // prime, and someone will one day notice and "fix" it — which renames
        // every cache file on every machine and orphans every index built so
        // far, silently. This is the test that stops them, and the comment on
        // `vault_key` says why the typo stays.
        assert_eq!(
            vault_key(Path::new("/Users/alexjanuschewsky/Documents/Magma")),
            "e0ca9f1e047817e3",
            "the cache file name changed; every existing index would be orphaned"
        );
    }

    #[test]
    fn two_vaults_do_not_share_a_cache_file() {
        let app = Path::new("/tmp/app");
        let a = cache_path(app, Path::new("/home/alex/Notizen"));
        let b = cache_path(app, Path::new("/home/alex/Archiv"));
        assert_ne!(a, b);
        assert_eq!(a, cache_path(app, Path::new("/home/alex/Notizen")));
    }
}
