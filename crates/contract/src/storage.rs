use utils::shared_map::SharedHashMap;

#[derive(Clone)]
pub struct OnChainStorage {
    chat_token_count: SharedHashMap<u64, u64>,
}

impl OnChainStorage {
    pub fn new() -> Self {
        Self { chat_token_count: Default::default() }
    }

    pub fn get_chat_token_count(&self, chat_id: u64) -> u64 {
        self.chat_token_count.get_clone(&chat_id).unwrap_or(0)
    }

    pub fn update_chat_token_count(&self, chat_id: u64, token_count: u64) {
        self.chat_token_count.insert(chat_id, token_count);
    }

    //TODO: make this atomic
    pub fn increment_chat_token_count(&self, chat_id: u64, token_count: u64) {
        let count = self.get_chat_token_count(chat_id);
        self.update_chat_token_count(chat_id, count + token_count);
    }

    pub fn remove_chat_count(&self, chat_id: u64) {
        self.chat_token_count.remove(&chat_id);
    }
}
