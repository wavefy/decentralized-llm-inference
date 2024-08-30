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
    ModelLayersWorker, ModelPostprocessor, ModelPreprocessor, Session,
};

mod internal;
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

    pub async fn chat(&self, session: Session, seed: u64, max_len: u32, prompt: &str, tx: Sender<String>) -> Result<()> {
        let mut tos = TokenOutputStream::new(self.tokenizer.clone());
        let tokens = tos.tokenizer().encode(prompt, true).unwrap();
        let mut all_tokens = vec![];
        let mut logits_processor = LogitsProcessor::from_sampling(seed, Sampling::ArgMax);
        let tokens = tokens.get_ids();

        self.layers_worker.start(session).await;

        // for first cycle, process input prompt
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

        for index in 0..max_len {
            let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let step1 = self.preprocessor.forward(session, input).await?;
            let step2 = self.layers_worker.forward(session, index as u32 + 1, step1, tokens.len() as u32 + index).await?;
            let logits = self.postprocessor.forward(session, step2).await?;
            let logits = logits.squeeze(0)?;
            next_token = logits_processor.sample(&logits)?;
            all_tokens.push(next_token);
            if let Some(t) = tos.next_token(next_token)? {
                tx.send(t).await.unwrap();
            }
            if next_token == eos_token {
                break;
            };
        }
        if let Some(rest) = tos.decode_rest().map_err(candle_core::Error::msg)? {
            tx.send(rest).await.unwrap();
        }
        self.layers_worker.finish(session).await;
        Ok(())
    }
}
