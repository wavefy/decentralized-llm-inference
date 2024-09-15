use std::{collections::HashMap, ops::Range, sync::Arc};

use candle_core::{Device, Tensor};
use model_router::RouteTable;
use models::{remote::TensorBuf, ModelLayersWorker};
use p2p_network::addr::NodeId;
use prost::Message;
use protocol::{
    llm::*,
    worker::event::{RpcReq, RpcRes},
    Session,
};
use spin::RwLock;

use crate::{rpc::RpcClientTx, ServiceHandler};

#[derive(Clone)]
struct SessionContainer {
    chat_id: u64,
    local: Option<Range<u32>>,
    remote: Option<(NodeId, Session)>,
}

pub struct ModelService<LW, const MODEL_LAYERS: usize> {
    device: Device,
    layers: LW,
    rpc: RpcClientTx,
    sessions: RwLock<HashMap<Session, SessionContainer>>,
    router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>,
}

impl<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static, const MODEL_LAYERS: usize> ModelService<LW, MODEL_LAYERS> {
    pub fn new(layers: LW, device: Device, rpc: RpcClientTx, router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>) -> Self {
        Self {
            layers,
            device,
            rpc,
            router,
            sessions: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl<LW: ModelLayersWorker<(Tensor, u32)>, const MODEL_LAYERS: usize> ServiceHandler<MODEL_LAYERS> for ModelService<LW, MODEL_LAYERS> {
    async fn on_req(&self, _from: NodeId, req: RpcReq) -> RpcRes {
        match req.cmd.as_str() {
            "START" => {
                let start_req = StartReq::decode(req.payload.as_slice()).unwrap();
                let res = self.start(start_req).await;
                let mut payload = Vec::new();
                res.encode(&mut payload).unwrap();
                RpcRes { seq: req.seq, success: true, payload }
            }
            "FORWARD" => {
                let forward_req = ForwardReq::decode(req.payload.as_slice()).unwrap();
                let res = self.forward(forward_req).await;
                let mut payload = Vec::new();
                res.encode(&mut payload).unwrap();
                RpcRes { seq: req.seq, success: true, payload }
            }
            "END" => {
                let end_req = EndReq::decode(req.payload.as_slice()).unwrap();
                let res = self.end(end_req).await;
                let mut payload = Vec::new();
                res.encode(&mut payload).unwrap();
                RpcRes { seq: req.seq, success: true, payload }
            }
            _ => RpcRes {
                seq: req.seq,
                success: false,
                ..Default::default()
            },
        }
    }
}

impl<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static, const MODEL_LAYERS: usize> ModelService<LW, MODEL_LAYERS> {
    pub async fn start(&self, req: StartReq) -> StartRes {
        log::info!("[ModelService] chat {} start session {} with from_layer {}", req.chat_id, req.session, req.from_layer);
        let chat_id = req.chat_id;
        let remote_session = Session::new();
        let route = if let Some(route) = self.router.read().select_next(req.from_layer) {
            route
        } else {
            return StartRes { success: false };
        };

        self.sessions.write().insert(
            Session(req.session),
            SessionContainer {
                chat_id: req.chat_id,
                local: route.local.clone(),
                remote: route.remote.as_ref().map(|(d, ..)| (d.clone(), remote_session.clone())),
            },
        );

        if let Some(layers) = route.local {
            log::info!("[ModelService] start session {} with local layers {layers:?}", req.session);
            self.layers.start(Session(req.session)).await;
        }
        let res = if let Some((dest, layers, _, _)) = &route.remote {
            log::info!(
                "[ModelService] start session {} with remote {dest:?} layers {layers:?}, remote session {}",
                req.session,
                remote_session.0
            );
            let res = self
                .rpc
                .request(
                    dest.clone(),
                    "START",
                    StartReq {
                        session: remote_session.0,
                        chat_id,
                        from_layer: layers.start,
                    },
                )
                .await
                .unwrap_or(StartRes { success: false });
            log::info!(
                "[ModelService] start session {} with remote {dest:?} layers {layers:?}, remote session {} done",
                req.session,
                remote_session.0
            );
            res
        } else {
            StartRes { success: true }
        };
        log::info!("[ModelService] chat {} start session {} with from_layer {} done", req.chat_id, req.session, req.from_layer);
        res
    }

    pub async fn forward(&self, req: ForwardReq) -> ForwardRes {
        if let Some(container) = self.sessions.read().get(&Session(req.session)).cloned() {
            log::info!("[ModelService] session {} forward step {} processing ...", req.session, req.step);
            let embedding = if let Some(layers) = container.local {
                log::info!("[ModelService] session {} forward step {} local {layers:?} layers", req.session, req.step);
                let embedding = TensorBuf::try_from(req.embedding).unwrap().to_tensor(&self.device).unwrap();
                let (embedding, _) = self.layers.forward(Session(req.session), req.step, (embedding, req.seq_len), req.index_pos).await.unwrap();
                log::info!("[ModelService] session {} forward step {} local {layers:?} layers done", req.session, req.step);
                TensorBuf::from(embedding).to_vec()
            } else {
                req.embedding
            };

            let res = if let Some((dest, remote_session)) = &container.remote {
                log::info!("[ModelService] session {} forward step {} remote {dest:?} with remote session {}", req.session, req.step, remote_session.0);
                let res = self
                    .rpc
                    .request(
                        dest.clone(),
                        "FORWARD",
                        ForwardReq {
                            session: remote_session.0,
                            embedding,
                            step: req.step,
                            seq_len: req.seq_len,
                            index_pos: req.index_pos,
                        },
                    )
                    .await
                    .unwrap_or(ForwardRes { success: false, ..Default::default() });
                log::info!("[ModelService] session {} forward step {} remote {dest:?} with remote session {} done", req.session, req.step, remote_session.0);
                res
            } else {
                ForwardRes { success: true, embedding }
            };
            log::info!("[ModelService] session {} forward step {} done", req.session, req.step);
            res
        } else {
            log::warn!("[ModelService] forward session {} but not found", req.session);
            ForwardRes { success: false, ..Default::default() }
        }
    }

    pub async fn end(&self, req: EndReq) -> EndRes {
        if let Some(container) = self.sessions.write().remove(&Session(req.session)) {
            log::warn!("[ModelService] session {} ending ...", req.session);
            if let Some(layers) = container.local {
                log::warn!("[ModelService] session {} end local {layers:?} layers", req.session);
                self.layers.finish(Session(req.session)).await;
            }

            if let Some((dest, remote_session)) = &container.remote {
                log::info!("[ModelService] session {} end remote {dest:?} with remote session {}", req.session, remote_session.0);
                let res = self.rpc.request(dest.clone(), "END", EndReq { session: remote_session.0 }).await.unwrap_or(EndRes { success: false });
                log::info!("[ModelService] session {} end remote {dest:?} with remote session {} done", req.session, remote_session.0);
                res
            } else {
                EndRes { success: true }
            }
        } else {
            log::warn!("[ModelService] end session {} but not found", req.session);
            EndRes { success: false }
        }
    }
}
