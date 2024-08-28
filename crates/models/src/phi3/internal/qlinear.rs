use candle_core::{quantized::gguf_file, Device, Result, Tensor};
use candle_nn::Module;

#[derive(Debug, Clone)]
pub struct QLinear {
    inner: candle_core::quantized::QMatMul,
    span: tracing::Span,
}

impl QLinear {
    pub fn new<R: std::io::Read + std::io::Seek>(ct: &gguf_file::Content, r: &mut R, name: &str, device: &Device) -> Result<Self> {
        let span = tracing::span!(tracing::Level::TRACE, "qmatmul");
        let w = ct.tensor(r, &format!("{name}.weight"), device)?;
        let inner = candle_core::quantized::QMatMul::from_qtensor(w)?;
        Ok(Self { inner, span })
    }
}

impl Module for QLinear {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let _enter = self.span.enter();
        self.inner.forward(xs)
    }
}
