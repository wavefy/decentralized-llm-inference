use candle_core::{quantized::gguf_file, Device, Tensor};
use models::{
    get_device,
    phi3::{model_path, Phi3LayersWorker, Phi3Model},
    remote::{RpcRequest, RpcResponse},
    ModelLayersWorker,
};
use protocol::{ModelLayersRanger, Session};
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let device = get_device(false).unwrap();
    let layers_worker = VirtualRemoteLayersWorker::new(&device).await;
    let phi3 = Phi3Model::new(&device, layers_worker).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        phi3.chat(Session::new(), &device, 299792458, 500, "Write function max(x1, x2) in Rust", tx).await.unwrap();
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

impl ModelLayersWorker<(Tensor, usize)> for VirtualRemoteLayersWorker {
    fn layers(&self) -> ModelLayersRanger {
        self.layers_worker.layers()
    }

    async fn forward(&self, session: Session, (tensor, seq_len): (Tensor, usize), index_pos: usize) -> candle_core::Result<(Tensor, usize)> {
        let req = RpcRequest { tensor, seq_len, index_pos };
        let buf: Vec<u8> = req.into();

        // println!("convert req to buf {}", buf.len());

        //convert back to request
        let req: RpcRequest = (buf, self.device.clone()).try_into()?;

        // println!("convert req from buf");

        let (res_tensor, _) = self.layers_worker.forward(session, (req.tensor, req.seq_len), req.index_pos).await?;

        //convert to bytes
        let res = RpcResponse { tensor: res_tensor };
        let buf: Vec<u8> = res.into();

        // println!("convert res to buf {}", buf.len());

        //convert back to response
        let res: RpcResponse = (buf, self.device.clone()).try_into()?;

        // println!("convert res from buf");

        Ok((res.tensor, seq_len))
    }

    async fn finish(&self, session: Session) {
        self.layers_worker.finish(session).await;
    }
}
