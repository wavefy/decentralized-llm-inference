use candle_core::{quantized::gguf_file, Device, Result, Tensor};
use candle_nn::{Embedding, Module};

use crate::{ModelPreprocessor, Session};

pub struct Phi3Preprocessor {
    tok_embeddings: Embedding,
    span: tracing::Span,
}

impl Phi3Preprocessor {
    pub fn new<R: std::io::Seek + std::io::Read>(
        ct: &gguf_file::Content,
        reader: &mut R,
        device: &Device,
    ) -> Result<Self> {
        let md_get = |s: &str| match ct.metadata.get(s) {
            None => candle_core::bail!("cannot find {s} in metadata"),
            Some(v) => Ok(v),
        };

        let embedding_length = md_get("phi3.embedding_length")?.to_u32()? as usize;
        let tok_embeddings = ct.tensor(reader, "token_embd.weight", device)?;
        let tok_embeddings = tok_embeddings.dequantize(device)?;

        let span = tracing::span!(tracing::Level::TRACE, "preprocessor");
        Ok(Self {
            tok_embeddings: Embedding::new(tok_embeddings, embedding_length),
            span,
        })
    }
}

impl ModelPreprocessor<Tensor, (Tensor, usize)> for Phi3Preprocessor {
    async fn forward(&self, _session: Session, xs: Tensor) -> Result<(Tensor, usize)> {
        let (_b_sz, seq_len) = xs.dims2()?;
        let _enter = self.span.enter();
        self.tok_embeddings.forward(&xs).map(|e| (e, seq_len))
    }

    async fn finish(&self, _session: Session) {}
}
