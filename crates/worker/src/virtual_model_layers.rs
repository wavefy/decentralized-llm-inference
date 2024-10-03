use std::sync::Arc;

use candle_core::{Device, Result, Tensor};
use models::{remote::TensorBuf, ChatCfg, ModelLayersWorker};
use protocol::{
    llm::{EndReq, ForwardReq, StartReq},
    Session,
};

use crate::model_service::ModelService;

pub struct VirtualModelLayers<LW, const MODEL_LAYERS: usize> {
    pub device: Device,
    pub model_service: Arc<ModelService<LW, MODEL_LAYERS>>,
}

#[async_trait::async_trait]
impl<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static, const MODEL_LAYERS: usize> ModelLayersWorker<(Tensor, u32)> for VirtualModelLayers<LW, MODEL_LAYERS> {
    async fn start(&self, session: Session, config: ChatCfg) -> Result<()> {
        let res = self
            .model_service
            .start(StartReq {
                session: session.0,
                chat_id: session.0,
                from_layer: 0,
                metadata: vec![],
                chain_index: 0,
                max_tokens: config.max_len,

            })
            .await;
        if res.success {
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Worker Start Error").into())
        }
    }

    async fn forward(&self, session: Session, step: u32, (tensor, seq_len): (Tensor, u32), index_pos: u32) -> Result<(Tensor, u32)> {
        let embedding = TensorBuf::from(tensor).to_vec();
        let res = self
            .model_service
            .forward(ForwardReq {
                session: session.0,
                embedding,
                step,
                seq_len,
                index_pos,
                metadata: vec![],
                chain_index: 0,
            })
            .await;
        if res.success {
            let res_tensor = TensorBuf::try_from(res.embedding).unwrap().to_tensor(&self.device)?;
            Ok((res_tensor, seq_len))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "RpcError").into())
        }
    }

    async fn finish(&self, session: Session) {
        self.model_service
            .end(EndReq {
                session: session.0,
                metadata: vec![],
                chain_index: 0,
            })
            .await;
    }
}
