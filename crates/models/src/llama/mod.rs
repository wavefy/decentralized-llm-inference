use std::{ops::Range, path::PathBuf};

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::VarBuilder;
use hf_hub::{api::tokio::Api, Repo, RepoType};
use internal::{Config, LlamaConfig, LlamaEosToks, LlamaPost, LlamaPre};
use protocol::Session;
use tokenizers::Tokenizer;
use tokio::sync::mpsc::Sender;

mod internal;
mod layers_worker;

pub use layers_worker::LlamaLayersWorker;

const EOS_TOKEN: &str = "</s>";
const USE_KV_CACHE: bool = true;

use crate::{
    http_api::ChatCompletionRequest,
    logits_processor::{LogitsProcessor, Sampling},
    token_output_stream::TokenOutputStream,
    utils::{apply_repeat_penalty, hub_load_safetensors},
    ChatCfg, ChatModel, ModelLayersWorker,
};

async fn tokenizer_path(repo: &str, filename: &str) -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(repo.to_string(), RepoType::Model, "main".to_string()));
    repo.get(filename).await.unwrap()
}

async fn config_path(repo: &str, filename: &str) -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(repo.to_string(), RepoType::Model, "main".to_string()));
    repo.get(filename).await.unwrap()
}

pub async fn model_filenames(repo: &str, filename: &str) -> Vec<PathBuf> {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(repo.to_string(), RepoType::Model, "main".to_string()));
    if filename.ends_with(".js") {
        hub_load_safetensors(&repo, filename).await.unwrap()
    } else {
        vec![repo.get(filename).await.unwrap()]
    }
}

pub struct ModelResource {
    pub repo: String,
    pub tokenizer: String,
    pub config: String,
    pub model: String,
}

pub struct LlamaModel<W: ModelLayersWorker<(Tensor, u32)>> {
    device: Device,
    tokenizer: Tokenizer,
    pre: LlamaPre,
    post: LlamaPost,
    layers_worker: W,
    config: Config,
}

impl<W: ModelLayersWorker<(Tensor, u32)>> LlamaModel<W> {
    pub async fn new(resource: &ModelResource, device: Device, dtype: DType, layers_worker: W, use_flash_attn: bool) -> Self {
        let tokenizer_filename = tokenizer_path(&resource.repo, &resource.tokenizer).await;
        let tokenizer = Tokenizer::from_file(tokenizer_filename).unwrap();

        let config_filename = config_path(&resource.repo, &resource.config).await;
        let config: LlamaConfig = serde_json::from_slice(&std::fs::read(config_filename).unwrap()).unwrap();
        let config = config.into_config(use_flash_attn);

        let filenames = model_filenames(&resource.repo, &resource.model).await;
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device).unwrap() };

        let pre = LlamaPre::load(&vb, &config).unwrap();
        let post = LlamaPost::load(&vb, &config).unwrap();

        Self {
            device,
            tokenizer,
            pre,
            layers_worker,
            post,
            config,
        }
    }
}

#[async_trait::async_trait]
impl<W: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static> ChatModel for LlamaModel<W> {
    fn build_prompt(&self, request: &ChatCompletionRequest) -> String {
        // let mut prompt = String::from("<|begin_of_text|>");
        // for message in &request.messages {
        //     match message.role.as_str() {
        //         "system" => {
        //             prompt.push_str(&format!("<|start_header_id|>system<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
        //         }
        //         "user" => {
        //             prompt.push_str(&format!("<|start_header_id|>user<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
        //         }
        //         "assistant" => {
        //             prompt.push_str(&format!("<|start_header_id|>assistant<|end_header_id|>\n{}<|eot_id|>", message.content[0].text));
        //         }
        //         _ => panic!("unsupported role: {}", message.role),
        //     }
        // }
        // prompt.push_str("<|start_header_id|>assistant<|end_header_id|>");
        // prompt

        // TODO: currently too big prompt cause RPC error, temporary solution is to use only the last user message
        for message in request.messages.iter().rev() {
            if message.role == "user" {
                return message.content[0].text.clone();
            }
        }
        "".to_string()
    }

    async fn chat(&self, session: Session, cfg: ChatCfg, prompt: &str, tx: Sender<String>) -> Result<()> {
        let eos_token_id = self.config.eos_token_id.clone().or_else(|| self.tokenizer.token_to_id(EOS_TOKEN).map(LlamaEosToks::Single));
        let mut tokens = self.tokenizer.encode(prompt, true).unwrap().get_ids().to_vec();
        let mut tokenizer = TokenOutputStream::new(self.tokenizer.clone());
        println!("tokens {tokens:?}");

        let mut logits_processor = {
            let temperature = cfg.temperature;
            let sampling = if cfg.temperature <= 0. {
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

        let mut start_gen = std::time::Instant::now();
        let mut index_pos = 0;
        let mut token_generated = 0;

        if let Err(e) = self.layers_worker.start(session, cfg.clone()).await {
            log::error!("failed to start layers worker: {e}");
            return Err(e);
        }
        for index in 0..cfg.max_len {
            let (context_size, context_index) = if USE_KV_CACHE && index > 0 {
                (1, index_pos)
            } else {
                (tokens.len(), 0)
            };
            if index == 1 {
                start_gen = std::time::Instant::now()
            }
            let ctxt = &tokens[tokens.len().saturating_sub(context_size)..];
            let input = Tensor::new(ctxt, &&self.device)?.unsqueeze(0)?;
            let (input, seq_len) = self.pre.forward(&input)?;
            let (logits, _) = self.layers_worker.forward(session, 0, (input, seq_len as u32), context_index).await?;
            let logits = self.post.forward(&logits, seq_len)?;
            let logits = logits.squeeze(0)?;
            let logits = if cfg.repeat_penalty == 1. {
                logits
            } else {
                let start_at = tokens.len().saturating_sub(cfg.repeat_last_n);
                apply_repeat_penalty(&logits, cfg.repeat_penalty, &tokens[start_at..])?
            };
            index_pos += ctxt.len() as u32;

            let next_token = logits_processor.sample(&logits)?;
            token_generated += 1;
            tokens.push(next_token);

            match eos_token_id {
                Some(LlamaEosToks::Single(eos_tok_id)) if next_token == eos_tok_id => {
                    break;
                }
                Some(LlamaEosToks::Multiple(ref eos_ids)) if eos_ids.contains(&next_token) => {
                    break;
                }
                _ => (),
            }
            if let Some(t) = tokenizer.next_token(next_token)? {
                if let Err(e) = tx.send(t).await {
                    log::error!("error sending message: {}", e);
                    break;
                }
            }
        }
        if let Some(rest) = tokenizer.decode_rest().unwrap() {
            if let Err(e) = tx.send(rest).await {
                log::error!("error sending message: {}", e);
            }
        }
        let dt = start_gen.elapsed();
        println!("\n\n{} tokens generated ({} token/s)\n", token_generated, (token_generated - 1) as f64 / dt.as_secs_f64(),);
        self.layers_worker.finish(session).await;
        Ok(())
    }
}

pub async fn new_layers(resource: &ModelResource, dtype: DType, device: Device, use_flash_attn: bool, range: Range<u32>) -> LlamaLayersWorker {
    let config_filename = config_path(&resource.repo, &resource.config).await;
    let config: LlamaConfig = serde_json::from_slice(&std::fs::read(config_filename).unwrap()).unwrap();
    let config = config.into_config(use_flash_attn);

    let filenames = model_filenames(&resource.repo, &resource.model).await;
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device).unwrap() };
    LlamaLayersWorker::new(range, vb, config, dtype, device).unwrap()
}
