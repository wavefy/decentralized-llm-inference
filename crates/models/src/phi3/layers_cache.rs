use std::sync::Arc;

use candle_nn::kv_cache::KvCache;
use spin::Mutex;
use utils::shared_map::SharedHashMap;

use crate::Session;

pub struct LayersCache {
    layers_cache: Vec<SharedHashMap<Session, Arc<Mutex<KvCache>>>>,
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

    pub fn add_cache(&self, idx: usize, session: Session) {
        self.layers_cache[idx].insert(session, Arc::new(Mutex::new(KvCache::new(self.dim, self.max_seq_len))));
    }

    pub fn get_cache(&self, idx: usize, session: Session) -> Arc<Mutex<KvCache>> {
        self.layers_cache[idx].get_clone(&session).unwrap()
    }

    pub fn del_cache(&self, idx: usize, session: Session) {
        self.layers_cache[idx].remove(&session);
    }
}
