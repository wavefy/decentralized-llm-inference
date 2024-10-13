use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StringOrVecContent {
    String(String),
    Vec(Vec<MessageContent>),
}

impl StringOrVecContent {
    pub fn contents(&self) -> Vec<&str> {
        match self {
            StringOrVecContent::String(c) => vec![c],
            StringOrVecContent::Vec(vec) => vec.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageContent {
    #[serde(rename = "type")]
    pub _type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: StringOrVecContent,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub plain_text: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<Model>,
}
