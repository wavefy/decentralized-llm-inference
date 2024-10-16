use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ModelId(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum OfferError {}

#[derive(Debug, PartialEq, Eq)]
pub enum AnswerError {
    Remote(String),
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDistribution {
    pub layers: Vec<usize>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub layers: usize,
    pub memory: usize,
}

pub const SUPPORTED_MODELS: [ModelInfo; 3] = [
    ModelInfo {
        id: "llama32-1b",
        layers: 16,
        memory: 3,
    },
    ModelInfo {
        id: "llama32-3b",
        layers: 28,
        memory: 8,
    },
    ModelInfo {
        id: "phi3",
        layers: 32,
        memory: 4,
    },
];

pub fn get_model_info(id: &str) -> Option<&ModelInfo> {
    SUPPORTED_MODELS.iter().find(|m| m.id == id)
}