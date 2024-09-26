use candle_core::{DType, Device, Result, Tensor};
use models::{
    get_device,
    llama::{new_layers, LlamaLayersWorker, LlamaModel},
    remote::TensorBuf,
    ChatCfg, ChatModel, ModelLayersWorker,
};
use protocol::Session;
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let device = get_device(false).unwrap();
    let layers_worker = VirtualRemoteLayersWorker::new(device.clone()).await;
    let llama = LlamaModel::new(device, DType::F16, layers_worker, false).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        llama.chat(Session::new(), ChatCfg::default(), "hello", tx).await.unwrap();
    });

    let begin = Instant::now();
    let mut count = 0;
    while let Some(text) = rx.recv().await {
        print!("{text}");
        count += 1;
    }
    println!(
        "\n{count} tokens in {:2} seconds => speed {:2}/s",
        begin.elapsed().as_secs_f32(),
        count as f32 / begin.elapsed().as_secs_f32()
    );
}

struct VirtualRemoteLayersWorker {
    layers_worker: LlamaLayersWorker,
    device: Device,
}

impl VirtualRemoteLayersWorker {
    async fn new(device: Device) -> Self {
        let layers_worker = new_layers(DType::F16, device.clone(), false, 0..16).await;
        Self { layers_worker, device }
    }
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for VirtualRemoteLayersWorker {
    async fn start(&self, session: protocol::Session) -> Result<()> {
        self.layers_worker.start(session).await;
        Ok(())
    }

    async fn forward(&self, session: Session, step: u32, (tensor, seq_len): (Tensor, u32), index_pos: u32) -> candle_core::Result<(Tensor, u32)> {
        let tensor_buf = TensorBuf::from(tensor).to_vec();
        //convert back to request
        let tensor = TensorBuf::try_from(tensor_buf).unwrap().to_tensor(&self.device)?;

        // println!("convert req from buf");

        let (res_tensor, _) = self.layers_worker.forward(session, step, (tensor, seq_len), index_pos).await?;

        //convert to bytes
        let res_tensor_buf = TensorBuf::from(res_tensor).to_vec();

        // println!("convert res to buf {}", buf.len());

        //convert back to response
        let res_tensor = TensorBuf::try_from(res_tensor_buf).unwrap().to_tensor(&self.device)?;

        // println!("convert res from buf");

        Ok((res_tensor, seq_len))
    }

    async fn finish(&self, session: Session) {
        self.layers_worker.finish(session).await;
    }
}
