use std::{ops::Range, sync::Arc};

use candle_core::{Device, Tensor};
use model_router::RouteTable;
use models::{remote::TensorBuf, ChatCfg, ModelLayersWorker};
use p2p_network::addr::NodeId;
use prost::Message;
use protocol::{
    llm::*,
    worker::event::{RpcReq, RpcRes},
    Session,
};
use spin::RwLock;
use tokio::sync::oneshot;
use usage_service::WorkerUsageService;
use utils::shared_map::SharedHashMap;

use crate::{rpc::RpcClientTx, ServiceHandler};

#[derive(Clone)]
struct SessionContainer {
    chat_id: u64,
    local: Option<Range<u32>>,
    remote: Option<(NodeId, Session)>,
}

pub enum WorkerEvent {
    Start(u64, Vec<String>),
    Forward(u64),
    End(u64, String),
}

pub struct WorkerEventWithResp {
    pub event: WorkerEvent,
    pub resp: Option<oneshot::Sender<bool>>,
}

pub struct ModelService<LW, const MODEL_LAYERS: usize> {
    device: Device,
    layers: LW,
    rpc: RpcClientTx,
    sessions: SharedHashMap<Session, SessionContainer>,
    router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>,
    usage_service: Arc<dyn WorkerUsageService>,
}

impl<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static, const MODEL_LAYERS: usize> ModelService<LW, MODEL_LAYERS> {
    pub fn new(layers: LW, device: Device, rpc: RpcClientTx, router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>, usage_service: Arc<dyn WorkerUsageService>) -> Self {
        Self {
            layers,
            device,
            rpc,
            router,
            sessions: Default::default(),
            usage_service,
        }
    }
}

#[async_trait::async_trait]
impl<LW: ModelLayersWorker<(Tensor, u32)>, const MODEL_LAYERS: usize> ServiceHandler<MODEL_LAYERS> for ModelService<LW, MODEL_LAYERS> {
    fn sessions(&self) -> Vec<u64> {
        self.sessions.keys_clone().into_iter().map(|s| *s).collect()
    }

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
        if let Ok(req) = self.usage_service.pre_start(req.clone()).await {
            log::info!("[ModelService] chat {} start session {} with from_layer {}", req.chat_id, req.session, req.from_layer);
            let chat_id = req.chat_id;
            let remote_session = Session::new();
            let route = if let Some(route) = self.router.read().select_next(req.from_layer) {
                route
            } else {
                log::warn!("[ModelService] chat {} start session {} with from_layer {} but no route", req.chat_id, req.session, req.from_layer);
                return StartRes { success: false, metadata: vec![] };
            };

            self.sessions.insert(
                Session(req.session),
                SessionContainer {
                    chat_id: req.chat_id,
                    local: route.local.clone(),
                    remote: route.remote.as_ref().map(|(d, ..)| (d.clone(), remote_session.clone())),
                },
            );

            if let Some(layers) = route.local {
                log::info!("[ModelService] start session {} with local layers {layers:?}", req.session);
                // TODO: Handle Chat Config properly
                self.layers.start(Session(req.session), ChatCfg::default()).await;
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
                            metadata: req.metadata.clone(),
                            chain_index: req.chain_index + 1,
                            max_tokens: req.max_tokens,
                        },
                    )
                    .await
                    .unwrap_or(StartRes {
                        success: false,
                        metadata: req.metadata.clone(),
                        ..Default::default()
                    });
                log::info!(
                    "[ModelService] start session {} with remote {dest:?} layers {layers:?}, remote session {} done",
                    req.session,
                    remote_session.0
                );
                res
            } else {
                StartRes {
                    success: true,
                    metadata: req.metadata.clone(),
                }
            };
            log::info!("[ModelService] chat {} start session {} with from_layer {} done", req.chat_id, req.session, req.from_layer);
            self.usage_service.post_start(req, res).await
        } else {
            log::warn!("[ModelService] chat {} start session {} failed to pre_start", req.chat_id, req.session.clone());
            return StartRes { success: false, metadata: vec![] };
        }
    }

    pub async fn forward(&self, req: ForwardReq) -> ForwardRes {
        let res = if let Some(container) = self.sessions.get_clone(&Session(req.session)) {
            if let Ok(req) = self.usage_service.pre_forward(container.chat_id, req.clone()).await {
                log::info!("[ModelService] session {} forward step {} processing ...", req.session, req.step);
                let embedding = if let Some(layers) = container.local {
                    log::info!("[ModelService] session {} forward step {} local {layers:?} layers ...", req.session, req.step);
                    let embedding = TensorBuf::try_from(req.embedding.clone()).unwrap().to_tensor(&self.device).unwrap();
                    let (embedding, _) = self.layers.forward(Session(req.session), req.step, (embedding, req.seq_len), req.index_pos).await.unwrap();
                    log::info!("[ModelService] session {} forward step {} local {layers:?} layers done", req.session, req.step);
                    TensorBuf::from(embedding).to_vec()
                } else {
                    req.embedding.clone()
                };

                let res = if let Some((dest, remote_session)) = &container.remote {
                    log::info!(
                        "[ModelService] session {} forward step {} remote {dest:?} with remote session {}, embedding {} bytes",
                        req.session,
                        req.step,
                        remote_session.0,
                        embedding.len(),
                    );
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
                                metadata: req.metadata.clone(),
                                chain_index: req.chain_index + 1,
                            },
                        )
                        .await
                        .unwrap_or(ForwardRes {
                            success: false,
                            metadata: req.metadata.clone(),
                            ..Default::default()
                        });
                    log::info!(
                        "[ModelService] session {} forward step {} remote {dest:?} with remote session {} done",
                        req.session,
                        req.step,
                        remote_session.0
                    );
                    res
                } else {
                    ForwardRes {
                        success: true,
                        embedding,
                        metadata: req.metadata.clone(),
                    }
                };
                log::info!("[ModelService] session {} forward step {} done", req.session, req.step);
                self.usage_service.post_forward(container.chat_id, req, res).await
            } else {
                log::warn!("[ModelService] session {} failed to pre_forward", req.session);
                ForwardRes { success: false, ..Default::default() }
            }
        } else {
            log::warn!("[ModelService] forward session {} but not found", req.session);
            ForwardRes { success: false, ..Default::default() }
        };
        res
    }

    pub async fn end(&self, req: EndReq) -> EndRes {
        if let Some(container) = self.sessions.remove(&Session(req.session)) {
            if let Ok(req) = self.usage_service.pre_end(container.chat_id, req.clone()).await {
                log::warn!("[ModelService] session {} ending ...", req.session);
                if let Some(layers) = container.local {
                    log::warn!("[ModelService] session {} end local {layers:?} layers", req.session);
                    self.layers.finish(Session(req.session)).await;
                }

                let res = if let Some((dest, remote_session)) = &container.remote {
                    log::info!("[ModelService] session {} end remote {dest:?} with remote session {}", req.session, remote_session.0);
                    let res = self
                        .rpc
                        .request(
                            dest.clone(),
                            "END",
                            EndReq {
                                session: remote_session.0,
                                metadata: req.metadata.clone(),
                                chain_index: req.chain_index + 1,
                            },
                        )
                        .await
                        .unwrap_or(EndRes {
                            success: false,
                            metadata: req.metadata.clone(),
                            ..Default::default()
                        });
                    log::info!("[ModelService] session {} end remote {dest:?} with remote session {} done", req.session, remote_session.0);
                    res
                } else {
                    EndRes {
                        success: true,
                        metadata: req.metadata.clone(),
                    }
                };
                self.usage_service.post_end(container.chat_id, req, res).await
            } else {
                log::warn!("[ModelService] session {} failed to pre_end", req.session);
                EndRes { success: false, metadata: vec![] }
            }
        } else {
            log::warn!("[ModelService] end session {} but not found", req.session);
            EndRes { success: false, metadata: vec![] }
        }
    }
}
