use candle_core::{Result, Tensor, D};
use candle_nn::Module;

use super::qlinear::QLinear;

#[derive(Debug, Clone)]
pub struct Mlp {
    pub ffn_up: QLinear,
    pub ffn_down: QLinear,
    pub i_size: usize,
}

impl Module for Mlp {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let up_states = xs.apply(&self.ffn_up)?;
        let gate = up_states.narrow(D::Minus1, 0, self.i_size)?;
        let up_states = up_states.narrow(D::Minus1, self.i_size, self.i_size)?;
        let up_states = (up_states * gate.silu()?)?;
        up_states.apply(&self.ffn_down)
    }
}
