#[derive(Debug, Clone)]
pub struct ChatCfg {
    pub seed: u64,
    pub temperature: f64,
    pub top_k: Option<usize>,
    pub top_p: Option<f64>,
    pub max_len: u32,
    pub repeat_penalty: f32,
    pub repeat_last_n: usize,
}

impl Default for ChatCfg {
    fn default() -> Self {
        Self {
            seed: 1234,
            temperature: 0.8,
            top_k: None,
            top_p: None,
            max_len: 1024,
            repeat_penalty: 1.1,
            repeat_last_n: 128,
        }
    }
}
