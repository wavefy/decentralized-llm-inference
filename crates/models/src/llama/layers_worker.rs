use std::{collections::HashMap, ops::Range, sync::Arc};

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::VarBuilder;
use spin::{Mutex, RwLock};

use crate::{ModelLayersWorker, Session};

use super::{
    internal::{Cache, Config, LlamaLayers},
    USE_KV_CACHE,
};

pub struct LlamaLayersWorker {
    caches: RwLock<HashMap<Session, Arc<Mutex<Cache>>>>,
    llama: LlamaLayers,
    cfg: Config,
    dtype: DType,
    device: Device,
}

impl LlamaLayersWorker {
    pub fn new(range: Range<u32>, vb: VarBuilder, cfg: Config, dtype: DType, device: Device) -> Result<Self> {
        let llama = LlamaLayers::load(vb, &cfg, range)?;
        Ok(Self {
            caches: Default::default(),
            llama,
            cfg,
            dtype,
            device,
        })
    }
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for LlamaLayersWorker {
    async fn start(&self, session: Session) -> Result<()> {
        let cache = Cache::new(USE_KV_CACHE, self.dtype, &self.cfg, &self.device).unwrap();
        self.caches.write().insert(session, Arc::new(Mutex::new(cache)));
        Ok(())
    }

    async fn forward(&self, session: Session, _step: u32, (xs, seq_len): (Tensor, u32), index_pos: u32) -> Result<(Tensor, u32)> {
        let cache = self.caches.read().get(&session).cloned().unwrap();
        let mut cache_mut = cache.lock();
        let res = self.llama.forward(xs, index_pos as usize, &mut cache_mut)?;
        Ok((res, seq_len))
    }

    async fn finish(&self, session: Session) {
        self.caches.write().remove(&session);
    }
}
