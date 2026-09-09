//! The reranker: a cross-encoder that reads question and passage together.
//!
//! **Never run against real weights from where this was written.** The
//! architecture below follows the model card and candle's
//! `XLMRobertaForSequenceClassification`, but huggingface.co is unreachable
//! from this machine, so the file names, the tensor layout and the number of
//! labels are all read from what the repository actually ships rather than
//! assumed — and [`Reranker::self_check`] refuses a model that loads and then
//! talks nonsense. A reranker that scores at random would quietly make every
//! answer worse, which is precisely the failure this project keeps paying for.

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::xlm_roberta::{Config, XLMRobertaForSequenceClassification};
use magma_core::Rerank;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use crate::download::RerankFiles;

/// Longest question-and-passage pair the model sees, in tokens.
///
/// Longer than the embedder's 256 because this reads *both* sides at once and
/// a passage that loses its tail here loses it in the only place that can see
/// the question. Still bounded: attention costs the square of the length in
/// time and in memory both, and a laptop pushed into swap looks frozen rather
/// than slow. That lesson was expensive once already.
const MAX_TOKENS: usize = 320;

/// Pairs per forward pass.
const BATCH: usize = 4;

pub struct Reranker {
    model: XLMRobertaForSequenceClassification,
    tokenizer: Mutex<Tokenizer>,
    device: Device,
    /// How many numbers the model returns per pair. One is a plain score; two
    /// is a not-relevant/relevant pair whose difference is the score.
    labels: usize,
    id: String,
}

impl Reranker {
    pub fn load(files: &RerankFiles, id: &str) -> Result<Self, String> {
        let device = Device::Cpu;
        let raw = std::fs::read_to_string(&files.config).map_err(|e| format!("config: {e}"))?;
        let config: Config = serde_json::from_str(&raw)
            .map_err(|e| format!("config is not an XLM-RoBERTa config: {e}"))?;

        // Read rather than assume: a cross-encoder may score with one number or
        // with a pair, and reading the second of one number is a panic, while
        // reading the first of two is silently the wrong sign.
        let labels = serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| {
                v.get("num_labels").and_then(|n| n.as_u64()).or_else(|| {
                    v.get("id2label")
                        .and_then(|m| m.as_object())
                        .map(|m| m.len() as u64)
                })
            })
            .unwrap_or(1) as usize;
        if labels == 0 || labels > 2 {
            return Err(format!(
                "a reranker with {labels} labels is not one this understands"
            ));
        }

        let mut tokenizer =
            Tokenizer::from_file(&files.tokenizer).map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                // The question must survive whole; it is the shorter side and
                // the one the passage is being judged against.
                strategy: tokenizers::TruncationStrategy::OnlySecond,
                ..Default::default()
            }))
            .map_err(|e| format!("tokenizer truncation: {e}"))?;

        // Safety: as in the embedder, the weights are written once, atomically,
        // into our own app data directory before this runs.
        let vb = if files.weights_are_safetensors {
            unsafe {
                VarBuilder::from_mmaped_safetensors(&[files.weights.clone()], DType::F32, &device)
                    .map_err(|e| format!("weights: {e}"))?
            }
        } else {
            // Older sentence-transformers repositories ship only a pickle.
            VarBuilder::from_pth(&files.weights, DType::F32, &device)
                .map_err(|e| format!("weights: {e}"))?
        };
        let model = XLMRobertaForSequenceClassification::new(labels, &config, vb)
            .map_err(|e| format!("model: {e}"))?;

        Ok(Self {
            model,
            tokenizer: Mutex::new(tokenizer),
            device,
            labels,
            id: id.to_string(),
        })
    }

    pub fn is_present(dir: &Path) -> bool {
        RerankFiles::in_dir(dir).is_some()
    }

    fn score_batch(&self, pairs: &[(String, String)]) -> Result<Vec<f32>, String> {
        let encodings = {
            let mut tok = self.tokenizer.lock().map_err(|_| "tokenizer poisoned")?;
            tok.with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                ..Default::default()
            }));
            tok.encode_batch(pairs.to_vec(), true)
                .map_err(|e| format!("tokenize: {e}"))?
        };

        let rows = encodings.len();
        let ids: Vec<u32> = encodings
            .iter()
            .flat_map(|e| e.get_ids().to_vec())
            .collect();
        let mask: Vec<u32> = encodings
            .iter()
            .flat_map(|e| e.get_attention_mask().to_vec())
            .collect();
        let cols = ids.len() / rows.max(1);

        let ids = Tensor::from_vec(ids, (rows, cols), &self.device).map_err(err)?;
        let mask = Tensor::from_vec(mask, (rows, cols), &self.device).map_err(err)?;
        // XLM-RoBERTa has a single segment; the type ids exist but are all zero.
        let types = ids.zeros_like().map_err(err)?;

        let logits = self.model.forward(&ids, &mask, &types).map_err(err)?;
        let logits: Vec<Vec<f32>> = logits.to_vec2::<f32>().map_err(err)?;
        Ok(logits
            .into_iter()
            .map(|row| {
                if self.labels == 2 {
                    // Relevant minus not-relevant: monotonic in the softmax
                    // probability, without computing one.
                    row.get(1).copied().unwrap_or(0.0) - row.first().copied().unwrap_or(0.0)
                } else {
                    row.first().copied().unwrap_or(0.0)
                }
            })
            .collect())
    }
}

impl Rerank for Reranker {
    fn id(&self) -> &str {
        &self.id
    }

    fn scores(&self, query: &str, passages: &[String]) -> Result<Vec<f32>, String> {
        if passages.is_empty() {
            return Ok(Vec::new());
        }
        let pairs: Vec<(String, String)> = passages
            .iter()
            .map(|p| (query.to_string(), p.clone()))
            .collect();
        let mut out = Vec::with_capacity(pairs.len());
        for group in pairs.chunks(BATCH) {
            out.extend(self.score_batch(group)?);
        }
        Ok(out)
    }
}

/// Does this reranker actually rank?
///
/// The same guard the embedder has, and for the same reason: a model that loads
/// and returns noise degrades every answer silently. Here it is worse than for
/// the embedder, because a reranker has the last word on the order — a wrong
/// sign would put the worst candidate first and look like a working feature.
pub fn self_check(model: &dyn Rerank) -> Result<(), String> {
    let query = "Wer hat den Updater beigesteuert?";
    let scores = model.scores(
        query,
        &[
            "Mitgewirkt\nDen In-App-Updater hat Alexander beigesteuert.".to_string(),
            "Foodfotografie\nMidjourney fotografiert Essen besser als mancher Profi.".to_string(),
        ],
    )?;
    if scores.len() != 2 {
        return Err("the reranker did not answer for every passage".into());
    }
    if !(scores[0] > scores[1]) {
        return Err(format!(
            "the reranker does not rank: related {:.3} vs unrelated {:.3}. \
             Wrong label order or wrong weights — leaving it off rather than \
             reordering every answer by nonsense.",
            scores[0], scores[1]
        ));
    }
    Ok(())
}

fn err(e: candle_core::Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ranks;
    impl Rerank for Ranks {
        fn id(&self) -> &str {
            "ranks"
        }
        fn scores(&self, _: &str, passages: &[String]) -> Result<Vec<f32>, String> {
            Ok(passages
                .iter()
                .map(|p| if p.contains("Updater") { 1.0 } else { 0.0 })
                .collect())
        }
    }

    /// The failure that matters most, and the reason `open_rerank` checks
    /// before handing the model over: the labels the wrong way round. It loads,
    /// it answers, every answer is backwards, and the worst candidate is put
    /// first — while the feature looks like it is working.
    struct Backwards;
    impl Rerank for Backwards {
        fn id(&self) -> &str {
            "backwards"
        }
        fn scores(&self, _: &str, passages: &[String]) -> Result<Vec<f32>, String> {
            Ok(passages
                .iter()
                .map(|p| if p.contains("Updater") { 0.0 } else { 1.0 })
                .collect())
        }
    }

    #[test]
    fn the_self_check_passes_a_reranker_that_ranks() {
        assert!(self_check(&Ranks).is_ok());
    }

    #[test]
    fn the_self_check_catches_reversed_labels() {
        let err = self_check(&Backwards).unwrap_err();
        assert!(err.contains("does not rank"), "{err}");
    }

    #[test]
    fn safetensors_are_preferred_and_a_pickle_still_works() {
        let dir = std::env::temp_dir().join("magma-embed-rerankfiles");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Nothing but the two small files: not a model.
        std::fs::write(dir.join("config.json"), b"{}").unwrap();
        std::fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        assert!(
            RerankFiles::in_dir(&dir).is_none(),
            "weights are not optional"
        );

        // A repository that ships only the older pickle is still usable, and
        // saying so here is the point: assuming safetensors would fail after
        // half a gigabyte of download rather than before it.
        std::fs::write(dir.join("pytorch_model.bin"), b"x").unwrap();
        let found = RerankFiles::in_dir(&dir).expect("a pickle is enough");
        assert!(!found.weights_are_safetensors);

        // With both present the safetensors win: they are memory-mapped rather
        // than unpickled.
        std::fs::write(dir.join("model.safetensors"), b"x").unwrap();
        let found = RerankFiles::in_dir(&dir).unwrap();
        assert!(found.weights_are_safetensors);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
