use candle_core::utils::{cuda_is_available, metal_is_available};
use candle_core::{Device, Result};
use protocol::{ModelLayersRanger, Session};

mod layers_cache;
mod logits_processor;
pub mod phi3;
mod quantized_var_builder;
pub mod remote;
mod token_output_stream;
mod utils;

#[allow(async_fn_in_trait)]
pub trait ModelPreprocessor<IN, OUT> {
    /// Async function for allowing remote execute
    /// This function convert from raw (text or other) to embedding
    async fn forward(&self, session: Session, input: IN) -> Result<OUT>;
    async fn finish(&self, session: Session);
}

#[allow(async_fn_in_trait)]
pub trait ModelLayersWorker<E> {
    fn layers(&self) -> ModelLayersRanger;
    /// Async function for allowing remote execute
    /// This function calculate from input to output embedding
    async fn forward(&self, session: Session, embedding: E, index_pos: usize) -> Result<E>;
    async fn finish(&self, session: Session);
}

#[allow(async_fn_in_trait)]
pub trait ModelPostprocessor<IN, OUT> {
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
            println!(
                "Running on CPU, to run on GPU(metal), build this example with `--features metal`"
            );
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            println!("Running on CPU, to run on GPU, build this example with `--features cuda`");
        }
        Ok(Device::Cpu)
    }
}
