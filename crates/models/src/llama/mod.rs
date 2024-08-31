use std::path::PathBuf;

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::VarBuilder;
use hf_hub::{api::tokio::Api, Repo, RepoType};
use internal::{Config, LlamaConfig, LlamaEosToks};
use protocol::{ModelLayersRanger, Session};
use tokenizers::Tokenizer;
use tokio::sync::mpsc::Sender;

mod internal;
mod layers_worker;

pub use layers_worker::LlamaLayersWorker;

const EOS_TOKEN: &str = "</s>";
const USE_KV_CACHE: bool = true;

use crate::{
    logits_processor::{LogitsProcessor, Sampling},
    token_output_stream::TokenOutputStream,
    utils::{apply_repeat_penalty, hub_load_safetensors},
    ModelLayersWorker,
};

async fn tokenizer_path() -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision("meta-llama/Meta-Llama-3.1-8B-Instruct".to_string(), RepoType::Model, "main".to_string()));
    repo.get("tokenizer.json").await.unwrap()
}

async fn config_path() -> PathBuf {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision("meta-llama/Meta-Llama-3.1-8B-Instruct".to_string(), RepoType::Model, "main".to_string()));
    repo.get("config.json").await.unwrap()
}

pub async fn model_filenames() -> Vec<PathBuf> {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision("meta-llama/Meta-Llama-3.1-8B-Instruct".to_string(), RepoType::Model, "main".to_string()));
    hub_load_safetensors(&repo, "model.safetensors.index.json").await.unwrap()
}

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

pub struct LlamaModel<W: ModelLayersWorker<(Tensor, u32)>> {
    device: Device,
    tokenizer: Tokenizer,
    layers_worker: W,
    config: Config,
}

impl<W: ModelLayersWorker<(Tensor, u32)>> LlamaModel<W> {
    pub async fn new(device: Device, layers_worker: W, use_flash_attn: bool) -> Self {
        let tokenizer_filename = tokenizer_path().await;
        let tokenizer = Tokenizer::from_file(tokenizer_filename).unwrap();

        let config_filename = config_path().await;
        let config: LlamaConfig = serde_json::from_slice(&std::fs::read(config_filename).unwrap()).unwrap();
        let config = config.into_config(use_flash_attn);

        Self {
            device,
            tokenizer,
            layers_worker,
            config,
        }
    }

    pub async fn chat(&self, session: Session, cfg: ChatCfg, prompt: &str, tx: Sender<String>) -> Result<()> {
        let eos_token_id = self.config.eos_token_id.clone().or_else(|| self.tokenizer.token_to_id(EOS_TOKEN).map(LlamaEosToks::Single));
        let mut tokens = self.tokenizer.encode(prompt, true).unwrap().get_ids().to_vec();
        let mut tokenizer = TokenOutputStream::new(self.tokenizer.clone());

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
            let seq_len = input.dims2().unwrap().1;
            let (logits, _) = self.layers_worker.forward(session, 0, (input, seq_len as u32), context_index).await?;
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
                tx.send(t).await.unwrap();
            }
        }
        if let Some(rest) = tokenizer.decode_rest().unwrap() {
            tx.send(rest).await.unwrap();
        }
        let dt = start_gen.elapsed();
        println!("\n\n{} tokens generated ({} token/s)\n", token_generated, (token_generated - 1) as f64 / dt.as_secs_f64(),);

        Ok(())
    }
}

pub async fn new_layers(dtype: DType, device: Device, use_flash_attn: bool, range: ModelLayersRanger) -> LlamaLayersWorker {
    let config_filename = config_path().await;
    let config: LlamaConfig = serde_json::from_slice(&std::fs::read(config_filename).unwrap()).unwrap();
    let config = config.into_config(use_flash_attn);

    let filenames = model_filenames().await;
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device).unwrap() };
    LlamaLayersWorker::new(range, vb, config, dtype, device).unwrap()
}
