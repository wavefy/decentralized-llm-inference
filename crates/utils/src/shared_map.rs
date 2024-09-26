use std::{collections::HashMap, hash::Hash, sync::Arc};

#[derive(Clone)]
pub struct SharedHashMap<K, V> {
    map: Arc<spin::RwLock<HashMap<K, V>>>,
}

impl<K: Clone + Eq + Hash, V: Clone> Default for SharedHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + Eq + Hash, V: Clone> SharedHashMap<K, V> {
    pub fn new() -> Self {
        Self {
            map: Arc::new(spin::RwLock::new(HashMap::new())),
        }
    }

    pub fn get_clone(&self, key: &K) -> Option<V> {
        self.map.read().get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.map.write().insert(key, value)
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.map.write().remove(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.map.read().contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.read().is_empty()
    }

    pub fn keys_clone(&self) -> Vec<K> {
        self.map.read().keys().cloned().collect()
    }

    pub fn values_clone(&self) -> Vec<V> {
        self.map.read().values().cloned().collect()
    }
}
