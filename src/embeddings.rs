//! Embedding backends. The real model is ONNX (fastembed + ort, loaded
//! dynamically from the library the flake provides); tests use a deterministic
//! test double so they never need a model or network.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::{MyError, Result};

pub trait Embedder {
    fn model_id(&self) -> &str;
    fn dim(&self) -> usize;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[cfg(feature = "semantic")]
pub const ONNX_MODEL_ID: &str = "BAAI/bge-small-en-v1.5";
#[cfg(feature = "semantic")]
pub const ONNX_DIM: usize = 384;

/// Loaded once per process and shared by the warm-up and indexing threads.
#[cfg(feature = "semantic")]
static ONNX_MODEL: OnceLock<std::result::Result<Mutex<fastembed::TextEmbedding>, String>> =
    OnceLock::new();

#[cfg(feature = "semantic")]
pub struct OnnxEmbedder {
    model: &'static Mutex<fastembed::TextEmbedding>,
}

#[cfg(feature = "semantic")]
impl OnnxEmbedder {
    /// Loads the model on first use. Weights are cached under
    /// `<db dir>/models`, which is `stdpath("data")/mesh/models` in practice.
    pub fn load(db_file: &Path) -> Result<Self> {
        let cache = db_file.parent().unwrap_or(Path::new(".")).join("models");
        let loaded = ONNX_MODEL.get_or_init(|| {
            fastembed::TextEmbedding::try_new(
                fastembed::TextInitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                    .with_cache_dir(cache)
                    .with_show_download_progress(false),
            )
            .map(Mutex::new)
            .map_err(|error| error.to_string())
        });
        match loaded {
            Ok(model) => Ok(Self { model }),
            Err(error) => Err(MyError::String(format!(
                "embedding model unavailable: {error}"
            ))),
        }
    }
}

#[cfg(feature = "semantic")]
impl Embedder for OnnxEmbedder {
    fn model_id(&self) -> &str {
        ONNX_MODEL_ID
    }

    fn dim(&self) -> usize {
        ONNX_DIM
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| MyError::String("embedding model lock poisoned".to_string()))?;
        model
            .embed(texts, Some(32))
            .map_err(|error| MyError::String(format!("embedding failed: {error}")))
    }
}

#[cfg(test)]
pub struct FakeEmbedder {
    pub model: &'static str,
    pub dim: usize,
}

#[cfg(test)]
impl FakeEmbedder {
    pub fn standard() -> Self {
        Self {
            model: "fake",
            dim: 16,
        }
    }
}

#[cfg(test)]
impl Embedder for FakeEmbedder {
    fn model_id(&self) -> &str {
        self.model
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0f32; self.dim];
                for word in text.split_whitespace() {
                    let bucket = rapidhash::v3::rapidhash_v3(word.as_bytes()) as usize;
                    vector[bucket % self.dim] += 1.0;
                }
                let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
                vector.iter().map(|x| x / norm).collect()
            })
            .collect())
    }
}
