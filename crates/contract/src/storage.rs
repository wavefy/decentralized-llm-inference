use utils::shared_map::SharedHashMap;

#[derive(Clone)]
pub struct OnChainStorage {
    sessions: SharedHashMap<u64, u64>,
}

impl OnChainStorage {
    pub fn new() -> Self {
        Self { sessions: Default::default() }
    }

    //TODO: make this atomic
    pub fn increase(&self, chat_id: u64, token_count: u64) {
        let count = self.sessions.get_clone(&chat_id).unwrap_or(0);
        self.sessions.insert(chat_id, count + token_count);
    }

    pub fn get(&self, chat_id: u64) -> u64 {
        self.sessions.get_clone(&chat_id).unwrap_or_default()
    }

    pub fn finish(&self, chat_id: u64) -> u64 {
        self.sessions.remove(&chat_id).unwrap_or_default()
    }

    pub fn sum(&self) -> u64 {
        self.sessions.values_clone().iter().sum()
    }
}
