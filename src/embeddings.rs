//! Embedding backends. Only the trait and a deterministic test double live here;
//! the real model is wired in during Phase 3.

use crate::Result;

pub trait Embedder {
    fn model_id(&self) -> &str;
    fn dim(&self) -> usize;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
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
