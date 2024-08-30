use candle_core::{quantized::gguf_file, Device, Tensor};
use models::{
    get_device,
    phi3::{model_path, Phi3LayersWorker, Phi3Model},
    remote::TensorBuf,
    ModelLayersWorker,
};
use protocol::{ModelLayersRanger, Session};
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let device = get_device(false).unwrap();
    let layers_worker = VirtualRemoteLayersWorker::new(&device).await;
    let phi3 = Phi3Model::new(device, layers_worker).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        phi3.chat(Session::new(), 299792458, 500, "Write function max(x1, x2) in Rust", tx).await.unwrap();
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
    layers_worker: Phi3LayersWorker,
    device: Device,
}

impl VirtualRemoteLayersWorker {
    async fn new(device: &Device) -> Self {
        let mut model_file = std::fs::File::open(model_path().await).unwrap();
        let model = gguf_file::Content::read(&mut model_file).unwrap();
        let layers_worker = Phi3LayersWorker::new(false, ModelLayersRanger::new(0, 31), &model, &mut model_file, &device).unwrap();
        Self {
            layers_worker,
            device: device.clone(),
        }
    }
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for VirtualRemoteLayersWorker {
    fn layers(&self) -> ModelLayersRanger {
        self.layers_worker.layers()
    }

    async fn start(&self, session: protocol::Session) {
        self.layers_worker.start(session).await;
    }

    async fn forward(&self, session: Session, step: u32, (tensor, seq_len): (Tensor, u32), index_pos: u32) -> candle_core::Result<(Tensor, u32)> {
        let tensor_buf = TensorBuf::from(tensor).to_vec();
        // println!("convert req to buf {}", buf.len());
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
