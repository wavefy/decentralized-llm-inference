use std::path::PathBuf;

use candle_core::{
    quantized::{gguf_file, QTensor},
    Device, Result, Tensor,
};
use candle_nn::RmsNorm;
use hf_hub::{api::tokio::Api, Repo, RepoType};
pub use layers_worker::Phi3LayersWorker;
pub use postprocessing::Phi3Postprocessor;
pub use preprocessing::Phi3Preprocessor;
use tokenizers::Tokenizer;
use tokio::sync::mpsc::Sender;

use crate::{
    logits_processor::{LogitsProcessor, Sampling},
    token_output_stream::TokenOutputStream,
    utils, ChatCfg, ChatCompletionRequest, ChatModel, ModelLayersWorker, ModelPostprocessor, ModelPreprocessor, Session,
};

mod internal;
mod layers_cache;
mod layers_worker;
mod postprocessing;
mod preprocessing;

fn rms_norm(w: QTensor, eps: f64) -> Result<RmsNorm> {
    let w = w.dequantize(&w.device())?;
    let rms = RmsNorm::new(w, eps);
    Ok(rms)
}

async fn tokenizer_path() -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision("microsoft/Phi-3-mini-4k-instruct".to_string(), RepoType::Model, "main".to_string()));
    repo.get("tokenizer.json").await.unwrap()
}

pub async fn model_path() -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision("microsoft/Phi-3-mini-4k-instruct-gguf".to_string(), RepoType::Model, "main".to_string()));
    repo.get("Phi-3-mini-4k-instruct-q4.gguf").await.unwrap()
}

pub struct Phi3Model<W: ModelLayersWorker<(Tensor, u32)>> {
    device: Device,
    tokenizer: Tokenizer,
    preprocessor: Phi3Preprocessor,
    layers_worker: W,
    postprocessor: Phi3Postprocessor,
}

impl<W: ModelLayersWorker<(Tensor, u32)>> Phi3Model<W> {
    pub async fn new(device: Device, layers_worker: W) -> Self {
        let tokenizer = Tokenizer::from_file(tokenizer_path().await).unwrap();
        let mut model_file = std::fs::File::open(model_path().await).unwrap();
        let model = gguf_file::Content::read(&mut model_file).unwrap();
        let preprocessor = Phi3Preprocessor::new(&model, &mut model_file, &device).unwrap();
        let postprocessor = Phi3Postprocessor::new(&model, &mut model_file, &device).unwrap();
        Self {
            device,
            tokenizer,
            preprocessor,
            layers_worker,
            postprocessor,
        }
    }
}

#[async_trait::async_trait]
impl<W: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static> ChatModel for Phi3Model<W> {
    fn build_prompt(&self, request: &ChatCompletionRequest) -> String {
        let mut prompt = String::new();
        for message in &request.messages {
            for content in message.content.contents() {
                match message.role.as_str() {
                    "system" => {
                        prompt.push_str(&format!("<|system|>\n{content}<|end|>"));
                    }
                    "user" => {
                        prompt.push_str(&format!("<|user|>\n{content}<|end|>"));
                    }
                    "assistant" => {
                        prompt.push_str(&format!("<|assistant|>\n{content}<|end|>"));
                    }
                    _ => {
                        log::warn!("unsupported role: {}", message.role)
                    }
                }
            }
        }
        prompt.push_str("<|assistant|>\n");
        prompt
    }

    async fn chat(&self, session: Session, cfg: ChatCfg, prompt: &str, tx: Sender<String>) -> Result<()> {
        log::info!("chatting with Phi3 model");
        log::info!("prompt: {prompt}");
        let mut tos = TokenOutputStream::new(self.tokenizer.clone());
        let tokens = tos.tokenizer().encode(prompt, true).unwrap();
        let mut all_tokens = vec![];
        let mut logits_processor = {
            let temperature = cfg.temperature;
            let sampling = if temperature <= 0. {
                Sampling::ArgMax
            } else {
                match (cfg.top_k, cfg.top_p) {
                    (None, None) => Sampling::All { temperature },
                    (Some(k), None) => Sampling::TopK { k, temperature },
                    (None, Some(p)) => Sampling::TopP { p, temperature },
                    (Some(k), Some(p)) => Sampling::TopKThenTopP { k, p, temperature },
                }
            };
            LogitsProcessor::from_sampling(cfg.seed, sampling)
        };
        let tokens = tokens.get_ids();

        if let Err(e) = self.layers_worker.start(session, cfg.clone()).await {
            log::error!("failed to start layers worker: {e}");
            return Err(e);
        }

        // for first cycle, process input prompt
        // we split it into tokens, and then process each token one by one for avoiding big message size
        let mut next_token = {
            let mut next_token = 0;
            for (pos, token) in tokens.iter().enumerate() {
                let input = Tensor::new(&[*token], &self.device)?.unsqueeze(0)?;
                let step1 = self.preprocessor.forward(session, input).await?;
                let step2 = self.layers_worker.forward(session, 0, step1, pos as u32).await?;
                let logits = self.postprocessor.forward(session, step2).await?;
                let logits = logits.squeeze(0)?;
                next_token = logits_processor.sample(&logits)?
            }
            next_token
        };

        all_tokens.push(next_token);
        if let Some(t) = tos.next_token(next_token)? {
            tx.send(t).await.unwrap();
        }
        let eos_token = *tos.tokenizer().get_vocab(true).get("<|endoftext|>").unwrap();

        for index in 0..(cfg.max_len - tokens.len() as u32) {
            let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let step1 = self.preprocessor.forward(session, input).await?;
            let step2 = self.layers_worker.forward(session, index as u32 + 1, step1, tokens.len() as u32 + index).await?;
            let logits = self.postprocessor.forward(session, step2).await?;
            let logits = logits.squeeze(0)?;
            let logits = if cfg.repeat_penalty == 1. {
                logits
            } else {
                let start_at = all_tokens.len().saturating_sub(cfg.repeat_last_n);
                utils::apply_repeat_penalty(&logits, cfg.repeat_penalty, &all_tokens[start_at..])?
            };
            next_token = logits_processor.sample(&logits)?;
            all_tokens.push(next_token);
            if let Some(t) = tos.next_token(next_token)? {
                if let Err(e) = tx.send(t).await {
                    log::error!("error sending message: {}", e);
                    break;
                }
            }
            if next_token == eos_token {
                break;
            };
        }
        if let Some(rest) = tos.decode_rest().map_err(candle_core::Error::msg)? {
            if let Err(e) = tx.send(rest).await {
                log::error!("error sending message: {}", e);
            }
        }
        self.layers_worker.finish(session).await;
        Ok(())
    }
}
