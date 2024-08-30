use candle_core::{Device, Result, Tensor};
use models::{remote::TensorBuf, ModelLayersWorker};
use protocol::Session;
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{SessionReq, SessionRes};

pub struct VirtualModelLayers {
    pub device: Device,
    pub session_control: Sender<(Session, SessionReq, oneshot::Sender<SessionRes>)>,
}

#[async_trait::async_trait]
impl ModelLayersWorker<(Tensor, u32)> for VirtualModelLayers {
    fn layers(&self) -> protocol::ModelLayersRanger {
        todo!()
    }

    async fn start(&self, session: Session) {
        log::info!("[VirtualModel] starting ..");
        let (tx, rx) = oneshot::channel();
        self.session_control.send((session, SessionReq::Start, tx)).await.unwrap();
        let res = rx.await.unwrap();
        if let SessionRes::Started(_) = res {
            log::info!("[VirtualModel] started ..");
        } else {
            log::warn!("invalid response for start request {res:?}");
        }
    }

    async fn forward(&self, session: Session, step: u32, (tensor, seq_len): (Tensor, u32), index_pos: u32) -> Result<(Tensor, u32)> {
        let tensor_buf = TensorBuf::from(tensor).to_vec();
        log::info!("[VirtualModel] forwarding {} bytes ..", tensor_buf.len());
        let (tx, rx) = oneshot::channel();
        self.session_control.send((session, SessionReq::Forward(step, tensor_buf, seq_len, index_pos), tx)).await.unwrap();
        let res = rx.await.unwrap();
        if let SessionRes::Backward(_, res_tensor_buf, seq_len, index_pos) = res {
            log::info!("[VirtualModel] forwarded got {} bytes ..", res_tensor_buf.len());
            let res_tensor = TensorBuf::try_from(res_tensor_buf).unwrap().to_tensor(&self.device)?;
            Ok((res_tensor, seq_len))
        } else {
            panic!("invalid response for forward request {res:?}");
        }
    }

    async fn finish(&self, session: Session) {
        log::info!("[VirtualModel] finishing ..");
        let (tx, rx) = oneshot::channel();
        self.session_control.send((session, SessionReq::Stop, tx)).await.unwrap();
        let res = rx.await.unwrap();
        if let SessionRes::Stopped(_) = res {
            log::info!("[VirtualModel] finished..");
        } else {
            log::warn!("invalid response for stop request {res:?}");
        }
    }
}
