use candle_core::{quantized::gguf_file, DType, Device, Result, Tensor};
use candle_nn::Module;

use crate::{ModelLayersRanger, ModelLayersWorker, Session};

use super::internal::{layer_weights::LayerWeights, mlp::Mlp, qlinear::QLinear};
use super::layers_cache::LayersCache;
use super::{model_path, rms_norm};

pub struct Phi3LayersWorker {
    range: ModelLayersRanger,
    layers: Vec<LayerWeights>,
    caches: LayersCache,
    span: tracing::Span,
}

impl Phi3LayersWorker {
    pub async fn new(use_flash_attn: bool, range: ModelLayersRanger, device: &Device) -> Result<Self> {
        let mut reader_f = std::fs::File::open(model_path().await).unwrap();
        let ct = gguf_file::Content::read(&mut reader_f).unwrap();
        let reader = &mut reader_f;

        let md_get = |s: &str| match ct.metadata.get(s) {
            None => candle_core::bail!("cannot find {s} in metadata"),
            Some(v) => Ok(v),
        };

        let max_seq_len = md_get("phi3.context_length")?.to_u32()? as usize;
        let head_count = md_get("phi3.attention.head_count")?.to_u32()? as usize;
        let head_count_kv = md_get("phi3.attention.head_count_kv")?.to_u32()? as usize;
        let i_size = md_get("phi3.feed_forward_length")?.to_u32()? as usize;
        let rms_eps = md_get("phi3.attention.layer_norm_rms_epsilon")?.to_f32()? as f64;
        let embedding_length = md_get("phi3.embedding_length")?.to_u32()? as usize;
        let head_dim = embedding_length / head_count;
        let rope_dim = md_get("phi3.rope.dimension_count")?.to_u32()? as usize;
        let (cos, sin) = precomput_freqs_cis(rope_dim, max_seq_len, 10_000., device)?;
        let neg_inf = Tensor::new(f32::NEG_INFINITY, device)?;

        let mut layers = Vec::with_capacity(range.len());
        for layer_idx in range.from..=range.to {
            println!("load layer {layer_idx}");
            let prefix = format!("blk.{layer_idx}");
            let ffn_up = QLinear::new(&ct, reader, &format!("{prefix}.ffn_up"), device).unwrap();
            let ffn_down = QLinear::new(&ct, reader, &format!("{prefix}.ffn_down"), device).unwrap();
            let mlp = Mlp { ffn_up, ffn_down, i_size };
            let attn_norm = rms_norm(ct.tensor(reader, &format!("{prefix}.attn_norm.weight"), device)?, rms_eps)?;
            let ffn_norm = rms_norm(ct.tensor(reader, &format!("{prefix}.ffn_norm.weight"), device)?, rms_eps)?;
            let span_attn = tracing::span!(tracing::Level::TRACE, "attn");
            let span_rot = tracing::span!(tracing::Level::TRACE, "attn-rot");
            layers.push(LayerWeights {
                attn_qkv: QLinear::new(&ct, reader, &format!("{prefix}.attn_qkv"), device)?,
                attn_output: QLinear::new(&ct, reader, &format!("{prefix}.attn_output"), device)?,
                attn_norm,
                ffn_norm,
                mlp,
                n_head: head_count,
                n_kv_head: head_count_kv,
                head_dim,
                cos: cos.clone(),
                sin: sin.clone(),
                neg_inf: neg_inf.clone(),
                use_flash_attn,
                span_attn,
                span_rot,
            })
        }

        let span = tracing::span!(tracing::Level::TRACE, "layers_worker");

        Ok(Self {
            range,
            layers,
            caches: LayersCache::new(range.len(), 2, max_seq_len),
            span,
        })
    }

    fn mask(&self, t: usize, device: &Device) -> Result<Tensor> {
        //TODO use LRU
        let mask: Vec<_> = (0..t).flat_map(|i| (0..t).map(move |j| u8::from(j > i))).collect();
        let mask = Tensor::from_slice(&mask, (t, t), device)?;
        Ok(mask)
    }
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for Phi3LayersWorker {
    fn layers(&self) -> ModelLayersRanger {
        self.range
    }

    async fn start(&self, session: Session) {
        for (idx, _) in self.layers.iter().enumerate() {
            self.caches.add_cache(idx, session);
        }
    }

    async fn forward(&self, session: Session, _step: u32, (mut xs, seq_len): (Tensor, u32), index_pos: u32) -> Result<(Tensor, u32)> {
        let _span = self.span.enter();
        let mask = if seq_len == 1 {
            None
        } else {
            Some(self.mask(seq_len as usize, xs.device())?)
        };
        for (idx, layer) in self.layers.iter().enumerate() {
            let residual = &xs;
            let ys = xs.apply(&layer.attn_norm)?;
            let kv_cache = self.caches.get_cache(idx, session);
            let ys = layer.forward_attn(&ys, mask.as_ref(), index_pos as usize, &mut kv_cache.lock())?;
            let ys = (ys + residual)?;
            let residual = &ys;
            let ys = ys.apply(&layer.ffn_norm)?;
            let ys = layer.mlp.forward(&ys)?;
            xs = (ys + residual)?;
        }
        Ok((xs, seq_len))
    }

    async fn finish(&self, session: Session) {
        for (idx, _) in self.layers.iter().enumerate() {
            self.caches.del_cache(idx, session);
        }
    }
}

fn precomput_freqs_cis(head_dim: usize, max_seq_len: usize, freq_base: f32, device: &Device) -> Result<(Tensor, Tensor)> {
    let theta: Vec<_> = (0..head_dim).step_by(2).map(|i| 1f32 / freq_base.powf(i as f32 / head_dim as f32)).collect();
    let theta = Tensor::new(theta.as_slice(), device)?;
    let idx_theta = Tensor::arange(0, max_seq_len as u32, device)?
        .to_dtype(DType::F32)?
        .reshape((max_seq_len, 1))?
        .matmul(&theta.reshape((1, theta.elem_count()))?)?;
    let cos = idx_theta.cos()?;
    let sin = idx_theta.sin()?;
    Ok((cos, sin))
}
