use std::{
    ops::Range,
    time::{Duration, Instant},
};

use candle_core::{Device, Result, Shape, Tensor};
use protocol::Session;
use tokio::sync::mpsc::Sender;

use crate::{ChatCfg, ChatModel, ModelLayersWorker};

pub struct FakeModel<W: ModelLayersWorker<(Tensor, u32)>> {
    device: Device,
    layers_worker: W,
}

impl<W: ModelLayersWorker<(Tensor, u32)>> FakeModel<W> {
    pub async fn new(device: Device, layers_worker: W) -> Self {
        Self { device, layers_worker }
    }
}

#[async_trait::async_trait]
impl<W: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static> ChatModel for FakeModel<W> {
    async fn chat(&self, session: Session, cfg: ChatCfg, _prompt: &str, tx: Sender<String>) -> Result<()> {
        self.layers_worker.start(session).await;
        let start_gen = Instant::now();
        for index in 0..cfg.max_len {
            let tensor = Tensor::from_vec(vec![index], Shape::from_dims(&[1]), &self.device).unwrap();
            let (_output, _) = self.layers_worker.forward(session, index, (tensor, index), index).await?;
            tx.send(format!("{index} ")).await.unwrap();
        }
        let dt = start_gen.elapsed();
        println!("\n\n{} tokens generated ({} token/s)\n", cfg.max_len, (cfg.max_len - 1) as f64 / dt.as_secs_f64(),);
        self.layers_worker.finish(session).await;
        Ok(())
    }
}

pub struct FakeLayersWorker {
    range: Range<u32>,
}

impl FakeLayersWorker {
    pub fn new(range: Range<u32>) -> Self {
        Self { range }
    }
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for FakeLayersWorker {
    async fn start(&self, session: Session) {}

    async fn forward(&self, session: Session, _step: u32, xs: (Tensor, u32), index_pos: u32) -> Result<(Tensor, u32)> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(xs)
    }

    async fn finish(&self, session: Session) {}
}
