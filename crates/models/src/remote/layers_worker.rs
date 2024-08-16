use candle_core::{Error, Result, Tensor};

use crate::{ModelLayersRanger, ModelLayersWorker, Session};

use super::{LayersRequester, RpcRequest};

pub struct LayersWorkerRequester<R: LayersRequester> {
    remote: R,
}

impl<R: LayersRequester> ModelLayersWorker<(Tensor, usize)> for LayersWorkerRequester<R> {
    fn layers(&self) -> ModelLayersRanger {
        todo!()
    }

    async fn forward(
        &self,
        session: Session,
        (tensor, seq_len): (Tensor, usize),
        index_pos: usize,
    ) -> Result<(Tensor, usize)> {
        let res = self
            .remote
            .request(
                session,
                RpcRequest {
                    tensor,
                    seq_len,
                    index_pos,
                },
            )
            .await
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, e)))?;

        Ok((res.tensor, seq_len))
    }

    async fn finish(&self, session: Session) {
        if let Err(_e) = self.remote.finish(session).await {
            todo!()
        }
    }
}
