use candle_core::{DType, Module, Result, Tensor, D};
use candle_nn::{kv_cache::KvCache, RmsNorm};

use super::mlp::Mlp;
use super::qlinear::QLinear;

#[derive(Debug, Clone)]
pub struct LayerWeights {
    pub attn_qkv: QLinear,
    pub attn_output: QLinear,
    pub attn_norm: RmsNorm,
    pub ffn_norm: RmsNorm,
    pub mlp: Mlp,
    pub n_head: usize,
    pub n_kv_head: usize,
    pub head_dim: usize,
    pub cos: Tensor,
    pub sin: Tensor,
    pub neg_inf: Tensor,
    pub use_flash_attn: bool,
    pub span_attn: tracing::Span,
    pub span_rot: tracing::Span,
}

fn masked_fill(on_false: &Tensor, mask: &Tensor, on_true: &Tensor) -> Result<Tensor> {
    let shape = mask.shape();
    let m = mask.where_cond(&on_true.broadcast_as(shape.dims())?, on_false)?;
    Ok(m)
}

impl LayerWeights {
    fn apply_rotary_emb(&self, xs: &Tensor, index_pos: usize) -> Result<Tensor> {
        let _enter = self.span_rot.enter();
        let (_b_sz, _h, seq_len, _n_embd) = xs.dims4()?;
        let cos = self.cos.narrow(0, index_pos, seq_len)?;
        let sin = self.sin.narrow(0, index_pos, seq_len)?;
        candle_nn::rotary_emb::rope(&xs.contiguous()?, &cos, &sin)
    }

    pub fn forward_attn(&self, x: &Tensor, mask: Option<&Tensor>, index_pos: usize, kv_cache: &mut KvCache) -> Result<Tensor> {
        let _enter = self.span_attn.enter();
        let (b_sz, seq_len, n_embd) = x.dims3()?;
        let qkv = self.attn_qkv.forward(x)?;

        let query_pos = self.n_head * self.head_dim;
        let q = qkv.narrow(D::Minus1, 0, query_pos)?;
        let k = qkv.narrow(D::Minus1, query_pos, self.n_kv_head * self.head_dim)?;
        let v = qkv.narrow(D::Minus1, query_pos + self.n_kv_head * self.head_dim, self.n_kv_head * self.head_dim)?;

        let q = q.reshape((b_sz, seq_len, self.n_head, self.head_dim))?.transpose(1, 2)?;
        let k = k.reshape((b_sz, seq_len, self.n_head, self.head_dim))?.transpose(1, 2)?;
        let v = v.reshape((b_sz, seq_len, self.n_kv_head, self.head_dim))?.transpose(1, 2)?;

        let q = self.apply_rotary_emb(&q, index_pos)?.contiguous()?;
        let k = self.apply_rotary_emb(&k, index_pos)?;

        let (k, v) = kv_cache.append(&k.contiguous()?, &v.contiguous()?)?;
        log::info!("[LayerWeights] add tensor to kv_cache => {:?} {:?}", k.shape(), v.shape());

        let k = crate::utils::repeat_kv(k, self.n_head / self.n_kv_head)?;
        let v = crate::utils::repeat_kv(v, self.n_head / self.n_kv_head)?;

        let y = if self.use_flash_attn {
            // flash-attn expects (b_sz, seq_len, nheads, head_dim)
            let q = q.to_dtype(DType::BF16)?.transpose(1, 2)?;
            let k = k.to_dtype(DType::BF16)?.transpose(1, 2)?;
            let v = v.to_dtype(DType::BF16)?.transpose(1, 2)?;
            let softmax_scale = 1f32 / (self.head_dim as f32).sqrt();
            flash_attn(&q, &k, &v, softmax_scale, seq_len > 1)?.to_dtype(DType::F32)?.transpose(1, 2)?
        } else {
            let att = (q.matmul(&k.t()?)? / (self.head_dim as f64).sqrt())?;
            let att = match mask {
                None => att,
                Some(mask) => {
                    let mask = mask.broadcast_as(att.shape())?;
                    masked_fill(&att, &mask, &self.neg_inf)?
                }
            };
            let att = candle_nn::ops::softmax_last_dim(&att)?;
            // Convert to contiguous as matmul doesn't support strided vs for now.
            att.matmul(&v)?
        };
        let y = y.transpose(1, 2)?.reshape(&[b_sz, seq_len, n_embd])?;
        let y = self.attn_output.forward(&y)?;
        Ok(y)
    }
}

#[cfg(feature = "flash-attn")]
fn flash_attn(q: &Tensor, k: &Tensor, v: &Tensor, softmax_scale: f32, causal: bool) -> Result<Tensor> {
    candle_flash_attn::flash_attn(q, k, v, softmax_scale, causal)
}

#[cfg(not(feature = "flash-attn"))]
fn flash_attn(_: &Tensor, _: &Tensor, _: &Tensor, _: f32, _: bool) -> Result<Tensor> {
    unimplemented!("compile with '--features flash-attn'")
}
