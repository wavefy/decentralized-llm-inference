use candle_core::{
    quantized::{gguf_file, QTensor},
    Device, IndexOp, Result, Tensor,
};
use candle_nn::{Module, RmsNorm};

use crate::{ModelPostprocessor, Session};

use super::{internal::qlinear::QLinear, rms_norm};

pub struct Phi3Postprocessor {
    output: QLinear,
    output_norm: RmsNorm,
    span: tracing::Span,
}

impl Phi3Postprocessor {
    pub fn new<R: std::io::Seek + std::io::Read>(
        ct: &gguf_file::Content,
        reader: &mut R,
        device: &Device,
    ) -> Result<Self> {
        let md_get = |s: &str| match ct.metadata.get(s) {
            None => candle_core::bail!("cannot find {s} in metadata"),
            Some(v) => Ok(v),
        };

        let span = tracing::span!(tracing::Level::TRACE, "postprocessing");
        let rms_eps = md_get("phi3.attention.layer_norm_rms_epsilon")?.to_f32()? as f64;
        let output_norm = rms_norm(ct.tensor(reader, "output_norm.weight", device)?, rms_eps)?;
        let output = QLinear::new(&ct, reader, "output", device)?;

        Ok(Self {
            span,
            output,
            output_norm,
        })
    }
}

impl ModelPostprocessor<(Tensor, usize), Tensor> for Phi3Postprocessor {
    async fn forward(&self, session: Session, (xs, seq_len): (Tensor, usize)) -> Result<Tensor> {
        let xs = xs.apply(&self.output_norm)?.i((.., seq_len - 1, ..))?;
        let _enter = self.span.enter();
        self.output.forward(&xs)
    }

    async fn finish(&self, session: Session) {}
}
