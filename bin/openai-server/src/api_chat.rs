use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use models::{ChatCfg, ChatModel};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    Body, Error, IntoResponse, Response,
};
use protocol::Session;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    io::AsyncRead,
    sync::mpsc::{channel, Receiver, Sender},
};

#[derive(Debug, Deserialize, Serialize)]
struct MessageContent {
    #[serde(rename = "type")]
    _type: String,
    text: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
    role: String,
    content: Vec<MessageContent>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Model {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}

#[derive(Debug, Serialize)]
struct ModelList {
    object: String,
    data: Vec<Model>,
}

const MODELS: [&str; 1] = ["phi3"];

struct AsyncReadRx {
    rx: Receiver<String>,
    buffer: Option<Vec<u8>>, // Store bytes in Vec<u8>
    offset: usize,           // Track how much of the buffer has been read
}

impl AsyncReadRx {
    pub fn new() -> (Self, Sender<String>) {
        let (tx, rx) = channel(10);
        (Self { rx, buffer: None, offset: 0 }, tx)
    }
}

impl AsyncRead for AsyncReadRx {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> Poll<io::Result<()>> {
        let s = self.get_mut();
        loop {
            // If we have data in the buffer, use it first
            if let Some(ref mut buffer) = s.buffer {
                // Determine how many bytes to copy to `buf`
                let available = buffer.len() - s.offset;
                let to_read = std::cmp::min(available, buf.remaining());

                // Copy data to `buf`
                buf.put_slice(&buffer[s.offset..s.offset + to_read]);
                s.offset += to_read;

                // If the buffer is exhausted, clear it
                if s.offset >= buffer.len() {
                    s.buffer = None;
                    s.offset = 0;
                }

                // Return if any data was read
                if to_read > 0 {
                    return Poll::Ready(Ok(()));
                }
            }

            // Poll the receiver for a new message
            match Pin::new(&mut s.rx).poll_recv(cx) {
                Poll::Ready(Some(msg)) => {
                    // Convert the String to bytes and store in buffer
                    s.buffer = Some(msg.into_bytes());
                    s.offset = 0;
                }
                Poll::Ready(None) => {
                    // No more messages in the channel (EOF)
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    // No data yet, return Pending
                    return Poll::Pending;
                }
            }
        }
    }
}

#[handler]
pub async fn list_models() -> Response {
    let models = MODELS
        .iter()
        .map(|&id| Model {
            id: id.to_string(),
            object: "model".to_string(),
            created: 1684275908, // using a fixed date for simplicity
            owned_by: "_".to_string(),
        })
        .collect();

    let model_list = ModelList {
        object: "list".to_string(),
        data: models,
    };

    Response::builder().header("Content-Type", "application/json").body(Body::from_json(&model_list).unwrap())
}

#[handler]
pub async fn get_model(Path(model_id): Path<String>) -> Result<Response, Error> {
    if let Some(&id) = MODELS.iter().find(|&&m| m == model_id) {
        let model = Model {
            id: id.to_string(),
            object: "model".to_string(),
            created: 1684275908, // using a fixed date for simplicity
            owned_by: "_".to_string(),
        };

        Ok(Response::builder().header("Content-Type", "application/json").body(Body::from_json(&model).unwrap()))
    } else {
        Err(Error::from_string("Model not found", StatusCode::NOT_FOUND))
    }
}

#[handler]
pub async fn chat_completions(Json(req): Json<ChatCompletionRequest>, data: Data<&Arc<dyn ChatModel>>) -> impl IntoResponse {
    let mut cfg = ChatCfg::default();
    if let Some(temperature) = req.temperature {
        cfg.temperature = temperature as f64;
    }
    if let Some(max_tokens) = req.max_tokens {
        cfg.max_len = max_tokens as u32;
    }
    let stream = req.stream.unwrap_or(false);

    let prompt = build_prompt(&req.model, req.messages);
    log::info!("prompt: {}", prompt);

    if stream {
        let (stream, stream_tx) = AsyncReadRx::new();
        let model_exe = data.0.clone();
        tokio::spawn(async move {
            let session = Session::new();
            let (tx, mut rx) = channel(1);
            tokio::spawn(async move { model_exe.chat(session, cfg, &prompt, tx).await });
            while let Some(out) = rx.recv().await {
                let response = json!({
                    "choices": [
                        {
                            "delta": {"content": out},
                            "index": 0
                        }
                    ]
                });
                stream_tx.send(response.to_string()).await.unwrap();
            }
        });

        let body = poem::Body::from_async_read(stream);

        Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(body)
    } else {
        todo!()
    }
}

fn build_prompt(model: &str, messages: Vec<Message>) -> String {
    match model {
        "phi3" => {
            let mut prompt = String::new();
            for message in messages {
                match message.role.as_str() {
                    "system" => {
                        prompt.push_str(&format!("<|system|>\n{}<|end|>", message.content[0].text));
                    }
                    "user" => {
                        prompt.push_str(&format!("<|user|>\n{}<|end|>", message.content[0].text));
                    }
                    "assistant" => {
                        prompt.push_str(&format!("<|assistant|>\n{}<|end|>", message.content[0].text));
                    }
                    _ => panic!("unsupported role: {}", message.role),
                }
            }
            prompt.push_str("<|assistant|>\n");
            prompt
        }
        "llama3" => {
            let mut prompt = String::from("<|begin_of_text|>");
            for message in messages {
                match message.role.as_str() {
                    "system" => {
                        prompt.push_str(&format!("<|start_header_id|>system<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
                    }
                    "user" => {
                        prompt.push_str(&format!("<|start_header_id|>user<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
                    }
                    "assistant" => {
                        prompt.push_str(&format!("<|start_header_id|>assistant<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
                    }
                    _ => panic!("unsupported role: {}", message.role),
                }
            }
            prompt.push_str("<|start_header_id|>assistant<|end_header_id|>");
            prompt
        }
        _ => panic!("unsupported model: {}", model),
    }
}
