use std::{collections::HashMap, sync::Arc};

use candle_nn::kv_cache::KvCache;
use spin::{Mutex, RwLock};

use crate::Session;

pub struct LayersCache {
    layers_cache: Vec<RwLock<HashMap<Session, Arc<Mutex<KvCache>>>>>,
    dim: usize,
    max_seq_len: usize,
}

impl LayersCache {
    pub fn new(len: usize, dim: usize, max_seq_len: usize) -> Self {
        let mut layers_cache = Vec::with_capacity(len);
        for _ in 0..len {
            layers_cache.push(Default::default());
        }
        Self { layers_cache, dim, max_seq_len }
    }

    pub fn get_cache(&self, idx: usize, session: Session) -> Arc<Mutex<KvCache>> {
        let slot = &self.layers_cache[idx];

        if !slot.read().contains_key(&session) {
            slot.write().insert(session, Arc::new(Mutex::new(KvCache::new(self.dim, self.max_seq_len))));
        }

        slot.read().get(&session).unwrap().clone()
    }

    pub fn rm_cache(&self, idx: usize, session: Session) {
        self.layers_cache[idx].write().remove(&session);
    }
}
