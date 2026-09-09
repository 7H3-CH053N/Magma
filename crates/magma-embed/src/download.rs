//! Fetching the model, once, into the app's own data directory.
//!
//! The weights are not shipped in the installer. They are roughly half a
//! gigabyte, the installer is twelve megabytes, and most of what Magma does
//! needs neither. So this is opt-in and one-time, and everything works without
//! it — retrieval simply stays lexical.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the files come from. Hugging Face serves them over plain HTTPS with no
/// account needed; `resolve/main` follows to the CDN.
const HOST: &str = "https://huggingface.co";

/// Which weights to fetch.
pub struct ModelSpec {
    /// Hugging Face repository, e.g. `intfloat/multilingual-e5-small`.
    pub repo: &'static str,
    /// The revision to fetch.
    ///
    /// This should be a commit sha, not a branch. A branch moves, and the cache
    /// stamps vectors with the model *id* rather than with what was actually
    /// downloaded, so weights changing under a populated cache would leave a
    /// vault embedded by one model and queried by another, with nothing looking
    /// wrong from the outside.
    ///
    /// It currently says `main`, which is exactly that hazard. The sha could
    /// not be looked up from where this was written, because huggingface.co is
    /// unreachable there. Pin it on the first machine that downloads the model
    /// successfully — the value is in the response, and it is a one-line change.
    pub revision: &'static str,
    /// What a cached vector is stamped with.
    ///
    /// Separate from [`Self::weights_dir`] because these two identify different
    /// things. This one says which *space* a vector lives in, and that depends
    /// on how the text was cut as well as on the weights: truncating at 256
    /// instead of 512 gives different vectors from the same model. Change it
    /// whenever either changes.
    pub id: &'static str,
    /// The folder the weights live in.
    ///
    /// Tied to the download, not to the vector space, so changing how text is
    /// fed to the model does not make Magma fetch half a gigabyte it already
    /// has.
    pub weights_dir: &'static str,
    /// The fallback size, for when the server will not say what a download
    /// costs. [`probe_size`] asks it; this is only what stands in when that
    /// fails, and a figure here may never have been checked against anything.
    pub approx_bytes: u64,
}

/// Multilingual because the vault this is built for is German, and an
/// English-only encoder — which would be a quarter of the size — cannot place
/// "Beglaubigung" anywhere near "Notarisierung".
pub const DEFAULT_MODEL: ModelSpec = ModelSpec {
    repo: "intfloat/multilingual-e5-small",
    revision: "main",
    // The suffix is not decoration. Vectors are stamped with this id, and the
    // same weights produce different vectors when the text reaching them
    // changes — a different truncation length, a different label in front of
    // the passage. Mixing two such sets would compare passages measured two
    // ways and look perfectly fine while doing it.
    //
    // `t256` is the truncation. `h` is what precedes a passage: its heading,
    // since the whole note path was measured on a real vault and found to push
    // a short passage from rank 2 to rank 77. See `magma_core::embedding_text`.
    id: "multilingual-e5-small-t256-h",
    // Unverified, unlike the reranker's below: this one was guessed before
    // there was any way to ask, and by the time there was, the model was long
    // downloaded and the panel no longer asks about it.
    weights_dir: "multilingual-e5-small",
    approx_bytes: 471_000_000,
};

/// Which weights to rerank with.
///
/// A separate download, and separate on purpose: someone who already waited for
/// half a gigabyte of encoder should not have to fetch it again to try this,
/// and someone who does not want a slower query should not have to fetch this
/// at all.
///
/// Multilingual, because the vault is German and its prompts and technical
/// terms are English — the pair this has to bridge is precisely "lockigen
/// schwarzen Haaren" against "curly black long hair".
pub const RERANK_MODEL: ModelSpec = ModelSpec {
    repo: "cross-encoder/mmarco-mMiniLMv2-L12-H384-v1",
    revision: "main",
    id: "mmarco-mminilmv2-l12-t320",
    weights_dir: "mmarco-mMiniLMv2-L12",
    // Measured on a real machine: 650 MB, against the 470 guessed here first —
    // thirty-eight percent out, which is why the panel asks [`probe_size`]
    // rather than reading this. It is the fallback for when the network will
    // not answer, nothing more.
    approx_bytes: 650_000_000,
};

/// The two names a set of weights may go by.
///
/// Newer repositories publish safetensors; older sentence-transformers ones
/// still ship only a pickle. Which of the two this particular repository has
/// could not be checked from where this was written, so both are handled rather
/// than one being assumed and failing after half a gigabyte of download.
const WEIGHT_NAMES: [&str; 2] = ["model.safetensors", "pytorch_model.bin"];

/// The files a reranker needs, and which kind of weights it found.
pub struct RerankFiles {
    pub config: PathBuf,
    pub tokenizer: PathBuf,
    pub weights: PathBuf,
    pub weights_are_safetensors: bool,
}

impl RerankFiles {
    /// What is on disk, or `None` when something is missing.
    pub fn in_dir(dir: &Path) -> Option<Self> {
        let config = dir.join("config.json");
        let tokenizer = dir.join("tokenizer.json");
        if !config.is_file() || !tokenizer.is_file() {
            return None;
        }
        let (weights, safetensors) = WEIGHT_NAMES
            .iter()
            .map(|n| (dir.join(n), *n == WEIGHT_NAMES[0]))
            .find(|(p, _)| p.is_file())?;
        Some(Self {
            config,
            tokenizer,
            weights,
            weights_are_safetensors: safetensors,
        })
    }
}

/// Fetch the reranker, whichever of the two weight formats the repository has.
pub fn ensure_rerank(
    dir: &Path,
    spec: &ModelSpec,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<RerankFiles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    if let Some(files) = RerankFiles::in_dir(dir) {
        return Ok(files);
    }

    for name in ["config.json", "tokenizer.json"] {
        let target = dir.join(name);
        if target.is_file() {
            continue;
        }
        fetch(&url_for(spec, name), &target, name, on_progress)?;
    }

    if WEIGHT_NAMES.iter().all(|n| !dir.join(n).is_file()) {
        // Try each in turn. A repository has one or the other, and asking for
        // the wrong one is a 404, not a corrupt download.
        let mut trouble = Vec::new();
        for name in WEIGHT_NAMES {
            match fetch(&url_for(spec, name), &dir.join(name), name, on_progress) {
                Ok(()) => break,
                Err(e) => trouble.push(e),
            }
        }
        if WEIGHT_NAMES.iter().all(|n| !dir.join(n).is_file()) {
            return Err(format!(
                "no weights could be fetched: {}",
                trouble.join("; ")
            ));
        }
    }

    RerankFiles::in_dir(dir).ok_or_else(|| "reranker files are still missing".to_string())
}

/// What a download will really cost, asked of the server rather than guessed.
///
/// `None` when the network will not say, which a caller should show as unknown
/// rather than substituting a number nobody measured.
pub fn probe_size(spec: &ModelSpec, weight_names: &[&str]) -> Option<u64> {
    // Its own agent, with its own patience. A download may wait two minutes
    // between bytes; asking how large it is may not — a caller wants a label on
    // a button, and would rather have none than a frozen panel.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .user_agent(USER_AGENT)
        .build();
    let mut total = 0u64;
    for name in ["config.json", "tokenizer.json"] {
        total += size_of(&agent, &url_for(spec, name))?;
    }
    // Whichever set of weights exists; the first that answers is the one that
    // would be downloaded.
    let weights = weight_names
        .iter()
        .find_map(|n| size_of(&agent, &url_for(spec, n)))?;
    Some(total + weights)
}

fn size_of(agent: &ureq::Agent, url: &str) -> Option<u64> {
    let response = agent.head(url).call().ok()?;
    // Hugging Face serves large files through LFS: `content-length` can be the
    // size of the pointer, and the real one comes in its own header.
    response
        .header("x-linked-size")
        .or_else(|| response.header("content-length"))
        .and_then(|v| v.parse().ok())
}

fn url_for(spec: &ModelSpec, name: &str) -> String {
    format!("{HOST}/{}/resolve/{}/{name}", spec.repo, spec.revision)
}

/// The three files an encoder needs on disk.
pub struct ModelFiles {
    pub config: PathBuf,
    pub tokenizer: PathBuf,
    pub weights: PathBuf,
}

impl ModelFiles {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            config: dir.join("config.json"),
            tokenizer: dir.join("tokenizer.json"),
            weights: dir.join("model.safetensors"),
        }
    }

    pub fn all_present(&self) -> bool {
        [&self.config, &self.tokenizer, &self.weights]
            .iter()
            .all(|p| p.is_file())
    }

    fn names() -> [&'static str; 3] {
        ["config.json", "tokenizer.json", "model.safetensors"]
    }
}

/// How far along a download is, for a progress bar.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    /// The file being fetched, e.g. `model.safetensors`.
    pub file: String,
    pub done: u64,
    /// `None` when the server did not say how large the file is.
    pub total: Option<u64>,
}

/// Make sure the model is on disk, downloading whatever is missing.
///
/// Each file is written to a `.part` beside it and renamed only once it is
/// complete. A half-written `model.safetensors` left behind by a lost
/// connection would otherwise look like a present model and fail at load time,
/// which is a much harder failure to read than a missing file.
pub fn ensure_model(
    dir: &Path,
    spec: &ModelSpec,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<ModelFiles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let files = ModelFiles::in_dir(dir);
    if files.all_present() {
        return Ok(files);
    }

    for name in ModelFiles::names() {
        let target = dir.join(name);
        if target.is_file() {
            continue;
        }
        fetch(&url_for(spec, name), &target, name, on_progress)?;
    }

    let files = ModelFiles::in_dir(dir);
    if !files.all_present() {
        return Err("model files are still missing after downloading".into());
    }
    Ok(files)
}

const USER_AGENT: &str = concat!(
    "Magma/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/7H3-CH053N/Magma)"
);

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        // Half a gigabyte over a slow line takes a while, and a read timeout
        // measures the gap between bytes rather than the whole transfer, so
        // this is generous without being unbounded.
        .timeout_read(Duration::from_secs(120))
        .user_agent(USER_AGENT)
        .build()
}

fn fetch(
    url: &str,
    target: &Path,
    name: &str,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), String> {
    let response = agent()
        .get(url)
        .call()
        .map_err(|e| format!("could not fetch {name}: {e}"))?;
    let total: Option<u64> = response
        .header("content-length")
        .and_then(|v| v.parse().ok());

    let part = target.with_extension("part");
    let mut file = std::fs::File::create(&part)
        .map_err(|e| format!("cannot write {}: {e}", part.display()))?;
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|e| format!("download of {name} broke off: {e}"))?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..read])
            .map_err(|e| format!("cannot write {name}: {e}"))?;
        done += read as u64;
        on_progress(DownloadProgress {
            file: name.to_string(),
            done,
            total,
        });
    }
    // Flush before the rename, or the file can be complete in name only.
    std::io::Write::flush(&mut file).map_err(|e| format!("cannot finish {name}: {e}"))?;
    drop(file);

    if let Some(expected) = total {
        if done != expected {
            let _ = std::fs::remove_file(&part);
            return Err(format!(
                "{name} arrived incomplete: {done} of {expected} bytes"
            ));
        }
    }
    std::fs::rename(&part, target).map_err(|e| format!("cannot finish {name}: {e}"))
}
