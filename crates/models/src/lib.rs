use candle_core::utils::{cuda_is_available, metal_is_available};
use candle_core::{Device, Result};
use protocol::{ChatCfg, ChatCompletionRequest, Session};
use tokio::sync::mpsc::Sender;

pub mod fake;
pub mod llama;
mod logits_processor;
pub mod phi3;
pub mod remote;
mod token_output_stream;
mod utils;

#[async_trait::async_trait]
pub trait ChatModel: Send + Sync + 'static {
    fn build_prompt(&self, request: &ChatCompletionRequest) -> String;
    async fn chat(&self, session: Session, cfg: ChatCfg, prompt: &str, tx: Sender<String>) -> Result<()>;
}

#[async_trait::async_trait]
pub trait ModelPreprocessor<IN, OUT> {
    async fn start(&self, session: Session);
    /// Async function for allowing remote execute
    /// This function convert from raw (text or other) to embedding
    async fn forward(&self, session: Session, input: IN) -> Result<OUT>;
    async fn finish(&self, session: Session);
}

#[async_trait::async_trait]
pub trait ModelLayersWorker<E>: Send + Sync + 'static {
    async fn start(&self, session: Session, config: ChatCfg) -> Result<()>;
    /// Async function for allowing remote execute
    /// This function calculate from input to output embedding
    async fn forward(&self, session: Session, step: u32, embedding: E, index_pos: u32) -> Result<E>;
    async fn finish(&self, session: Session);
}

#[async_trait::async_trait]
pub trait ModelPostprocessor<IN, OUT> {
    async fn start(&self, session: Session);
    /// Async function for allowing remote execute
    /// This function convert embedding to output
    async fn forward(&self, session: Session, input: IN) -> Result<OUT>;
    async fn finish(&self, session: Session);
}

pub fn get_device(cpu: bool) -> Result<Device> {
    if cpu {
        Ok(Device::Cpu)
    } else if cuda_is_available() {
        Ok(Device::new_cuda(0)?)
    } else if metal_is_available() {
        Ok(Device::new_metal(0)?)
    } else {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            println!("Running on CPU, to run on GPU(metal), build this example with `--features metal`");
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            println!("Running on CPU, to run on GPU, build this example with `--features cuda`");
        }
        Ok(Device::Cpu)
    }
}
