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

pub use cache::Cached;
pub use download::{ensure_model, DownloadProgress, ModelFiles, ModelSpec, DEFAULT_MODEL};
pub use model::Embedder;

use magma_core::Similarity;
use std::path::{Path, PathBuf};

/// Where the model and its cache live under the app's data directory.
pub fn model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models").join(DEFAULT_MODEL.id)
}

/// The vector cache for one vault.
///
/// Per vault, because two vaults share no passages, and keyed by the vault's
/// path so switching between them does not throw the other's work away.
pub fn cache_path(app_data: &Path, vault: &Path) -> PathBuf {
    let key = vault.to_string_lossy();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    app_data
        .join("vectors")
        .join(format!("{}-{hash:016x}.bin", DEFAULT_MODEL.id))
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
    fn two_vaults_do_not_share_a_cache_file() {
        let app = Path::new("/tmp/app");
        let a = cache_path(app, Path::new("/home/alex/Notizen"));
        let b = cache_path(app, Path::new("/home/alex/Archiv"));
        assert_ne!(a, b);
        assert_eq!(a, cache_path(app, Path::new("/home/alex/Notizen")));
    }
}
