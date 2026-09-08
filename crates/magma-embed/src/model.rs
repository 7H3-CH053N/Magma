//! The model itself: a small multilingual sentence encoder, run on the CPU.

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use magma_core::Similarity;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use crate::download::ModelFiles;

/// Longest passage the encoder sees, in tokens. The model's own limit is 512;
/// a passage of ~220 words fits comfortably, and truncating rather than
/// erroring means an unusually long one still contributes something.
const MAX_TOKENS: usize = 512;

/// Passages are embedded in batches of this many. Purely a memory ceiling: a
/// whole vault at once would allocate a tensor the size of the vault.
const BATCH: usize = 16;

/// E5 models are trained with these prefixes and lose accuracy without them.
/// The loss is invisible — results still come back, only worse — which is why
/// [`Similarity`] splits query from passage instead of trusting a comment.
const QUERY_PREFIX: &str = "query: ";
const PASSAGE_PREFIX: &str = "passage: ";

/// A loaded sentence encoder.
pub struct Embedder {
    model: BertModel,
    /// `Tokenizer::encode_batch` needs `&mut` to set padding per batch, and
    /// [`Similarity`] is shared across threads, so the lock is not optional.
    /// Uncontended in practice: retrieval embeds one batch at a time.
    tokenizer: Mutex<Tokenizer>,
    device: Device,
    id: String,
}

impl Embedder {
    /// Load from files already on disk. See [`crate::ensure_model`] for getting
    /// them there.
    pub fn load(files: &ModelFiles, id: &str) -> Result<Self, String> {
        let device = Device::Cpu;
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(&files.config).map_err(|e| format!("config: {e}"))?,
        )
        .map_err(|e| format!("config is not a BERT config: {e}"))?;

        let mut tokenizer =
            Tokenizer::from_file(&files.tokenizer).map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| format!("tokenizer truncation: {e}"))?;

        // Safety: `from_mmaped_safetensors` maps the file; the weights must not
        // be replaced underneath us. They live in our own app data directory and
        // are written once, atomically, before this is called.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[files.weights.clone()], DType::F32, &device)
                .map_err(|e| format!("weights: {e}"))?
        };
        let model = BertModel::load(vb, &config).map_err(|e| format!("model: {e}"))?;

        Ok(Self {
            model,
            tokenizer: Mutex::new(tokenizer),
            device,
            id: id.to_string(),
        })
    }

    /// True when every file the model needs is already on disk.
    pub fn is_present(dir: &Path) -> bool {
        ModelFiles::in_dir(dir).all_present()
    }

    fn encode(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(texts.len());
        for group in texts.chunks(BATCH) {
            out.extend(self.encode_batch(group)?);
        }
        Ok(out)
    }

    fn encode_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let encodings = {
            let mut tok = self.tokenizer.lock().map_err(|_| "tokenizer poisoned")?;
            // Pad to the longest of this batch rather than to the model's
            // maximum: a batch of short passages should not pay for 512 tokens.
            tok.with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                ..Default::default()
            }));
            tok.encode_batch(texts.to_vec(), true)
                .map_err(|e| format!("tokenize: {e}"))?
        };

        let ids: Vec<u32> = encodings
            .iter()
            .flat_map(|e| e.get_ids().to_vec())
            .collect();
        let mask: Vec<u32> = encodings
            .iter()
            .flat_map(|e| e.get_attention_mask().to_vec())
            .collect();
        let rows = encodings.len();
        let cols = ids.len() / rows.max(1);

        let ids = Tensor::from_vec(ids, (rows, cols), &self.device).map_err(err)?;
        let mask = Tensor::from_vec(mask, (rows, cols), &self.device).map_err(err)?;
        let types = ids.zeros_like().map_err(err)?;

        let hidden = self.model.forward(&ids, &types, Some(&mask)).map_err(err)?;

        mean_pool(&hidden, &mask)
    }
}

impl Similarity for Embedder {
    fn id(&self) -> &str {
        &self.id
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let one = vec![format!("{QUERY_PREFIX}{text}")];
        self.encode(&one)?
            .pop()
            .ok_or_else(|| "no vector returned".to_string())
    }

    fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let prefixed: Vec<String> = texts
            .iter()
            .map(|t| format!("{PASSAGE_PREFIX}{t}"))
            .collect();
        self.encode(&prefixed)
    }
}

/// Average the token vectors of each text, counting only real tokens, then
/// scale to unit length.
///
/// Masking matters: padding tokens carry a vector like any other, and averaging
/// them in makes a short text's meaning drift toward whatever padding encodes.
/// Normalising matters because retrieval compares by cosine, and a unit vector
/// makes that a dot product with no length left to bias it.
pub(crate) fn mean_pool(hidden: &Tensor, mask: &Tensor) -> Result<Vec<Vec<f32>>, String> {
    let (_, _, width) = hidden.dims3().map_err(err)?;
    let m = mask
        .to_dtype(DType::F32)
        .map_err(err)?
        .unsqueeze(2)
        .map_err(err)?
        .broadcast_as(hidden.shape())
        .map_err(err)?;
    let summed = (hidden * &m).map_err(err)?.sum(1).map_err(err)?;
    let counts = m
        .sum(1)
        .map_err(err)?
        .clamp(1e-9, f32::INFINITY)
        .map_err(err)?;
    let mean = (summed / counts).map_err(err)?;

    let norm = mean
        .sqr()
        .map_err(err)?
        .sum_keepdim(1)
        .map_err(err)?
        .sqrt()
        .map_err(err)?
        .clamp(1e-12, f32::INFINITY)
        .map_err(err)?
        .broadcast_as((mean.dims2().map_err(err)?.0, width))
        .map_err(err)?;
    let unit = (mean / norm).map_err(err)?;
    unit.to_vec2::<f32>().map_err(err)
}

fn err(e: candle_core::Error) -> String {
    e.to_string()
}
