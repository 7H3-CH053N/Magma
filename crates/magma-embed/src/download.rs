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
    /// What a cached vector is stamped with. Change this whenever the weights
    /// change, or old vectors will be compared against new ones.
    pub id: &'static str,
    /// Roughly what the download costs, for telling the user before they start.
    pub approx_bytes: u64,
}

/// Multilingual because the vault this is built for is German, and an
/// English-only encoder — which would be a quarter of the size — cannot place
/// "Beglaubigung" anywhere near "Notarisierung".
pub const DEFAULT_MODEL: ModelSpec = ModelSpec {
    repo: "intfloat/multilingual-e5-small",
    revision: "main",
    id: "multilingual-e5-small",
    approx_bytes: 471_000_000,
};

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
        let url = format!("{HOST}/{}/resolve/{}/{name}", spec.repo, spec.revision);
        fetch(&url, &target, name, on_progress)?;
    }

    let files = ModelFiles::in_dir(dir);
    if !files.all_present() {
        return Err("model files are still missing after downloading".into());
    }
    Ok(files)
}

fn fetch(
    url: &str,
    target: &Path,
    name: &str,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        // Half a gigabyte over a slow line takes a while, and a read timeout
        // measures the gap between bytes rather than the whole transfer, so
        // this is generous without being unbounded.
        .timeout_read(Duration::from_secs(120))
        .user_agent(concat!(
            "Magma/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/7H3-CH053N/Magma)"
        ))
        .build();

    let response = agent
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
