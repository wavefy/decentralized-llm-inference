use std::{collections::HashMap, time::Duration};

use candle_core::{Device, Tensor};
use model_router::RouteTable;
use models::{remote::TensorBuf, ModelLayersWorker};
use network::{
    addr::NodeId,
    node::{NetworkNode, NodeEvent},
};
use protocol::{ModelLayersRanger, Session};
use registry::{
    client::{RegistryClient, RegistryClientEvent},
    AnswerError,
};
use session::LlmSession;
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        oneshot,
    },
    time::{Instant, Interval},
};

use crate::VirtualModelLayers;

mod session;

pub enum LlmReq {
    Start(Session),
    Forward(Session, u32, Vec<u8>, u32, u32),
    End(Session),
}

pub enum LlmRes {
    Start(Session),
    Backward(Session, u32, Vec<u8>, u32, u32),
    End(Session),
}

#[derive(Debug)]
pub enum SessionReq {
    Start,
    Forward(u32, Vec<u8>, u32, u32),
    Stop,
}

#[derive(Debug)]
pub enum SessionRes {
    Started(NodeId),
    Backward(u32, Vec<u8>, u32, u32),
    Stopped(NodeId),
}

pub struct WorkerRunner<const MODEL_LAYERS: usize> {
    registry_client: RegistryClient,
    router: RouteTable<NodeId, MODEL_LAYERS>,
    network: NetworkNode<protocol::worker::Event>,
    ticker: Interval,
    sessions: HashMap<Session, LlmSession>,
    llm_req_tx: Sender<LlmReq>,
    llm_res_rx: Receiver<LlmRes>,
    session_control_rx: Receiver<(Session, SessionReq, oneshot::Sender<SessionRes>)>,
}

impl<const MODEL_LAYERS: usize> WorkerRunner<MODEL_LAYERS> {
    // TODO make layers_worker generic
    pub async fn new<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static>(
        registry_endpoint: &str,
        device: Device,
        layers_worker: LW,
        model: &str,
        node_id: &str,
        from: u32,
        to: u32,
    ) -> (Self, VirtualModelLayers) {
        log::info!("[WorkerRunner] start with node {node_id} with model {model}, layers [{from}-{to}] / total {MODEL_LAYERS}");
        let node_id = NodeId(node_id.to_string());
        let mut registry_client = RegistryClient::new(registry_endpoint, model, node_id.clone()).await;
        registry_client.update_layer(ModelLayersRanger { from, to });

        let (session_control_tx, session_control_rx) = channel(10);
        let model_layers = VirtualModelLayers {
            device: device.clone(),
            session_control: session_control_tx,
        };

        let (llm_res_tx, llm_res_rx) = channel(10);
        let (llm_req_tx, mut llm_req_rx) = channel(10);

        // task for running all process
        tokio::spawn(async move {
            while let Some(req) = llm_req_rx.recv().await {
                // TODO implement real
                match req {
                    LlmReq::Start(session) => {
                        log::info!("[WorkerRunner] llm core processing {session:?} start");
                        layers_worker.start(session).await;
                        log::info!("[WorkerRunner] llm core processed {session:?} start");
                        llm_res_tx.send(LlmRes::Start(session)).await.unwrap();
                    }
                    LlmReq::Forward(session, step, payload, seq_len, index_pos) => {
                        let start_ms = Instant::now();
                        log::info!("[WorkerRunner] llm core processing {session:?} forward step {step}, payload {} bytes", payload.len());
                        let tensor = TensorBuf::try_from(payload).unwrap().to_tensor(&device).unwrap();
                        let (res_tensor, _) = layers_worker.forward(session, step, (tensor, seq_len), index_pos).await.unwrap();
                        let res_tensor_buf = TensorBuf::from(res_tensor.clone()).to_vec();
                        log::info!(
                            "[WorkerRunner] llm core processed {session:?} forward step {step}, res payload {} bytes, dims {:?} take {:?}",
                            res_tensor_buf.len(),
                            res_tensor.shape().dims(),
                            start_ms.elapsed()
                        );
                        llm_res_tx.send(LlmRes::Backward(session, step, res_tensor_buf, seq_len, index_pos)).await.unwrap();
                    }
                    LlmReq::End(session) => {
                        log::info!("[WorkerRunner] llm core processing {session:?} end");
                        layers_worker.finish(session).await;
                        log::info!("[WorkerRunner] llm core processed {session:?} end");
                        llm_res_tx.send(LlmRes::End(session)).await.unwrap();
                    }
                }
            }
        });

        (
            Self {
                registry_client,
                router: RouteTable::new(from..to),
                network: NetworkNode::new(node_id).await,
                ticker: tokio::time::interval(Duration::from_millis(1000)),
                sessions: HashMap::new(),
                llm_req_tx,
                llm_res_rx,
                session_control_rx,
            },
            model_layers,
        )
    }

    pub async fn shutdown(&mut self) {
        self.registry_client.shutdown().await;
        self.network.shutdown().await
    }

    pub async fn recv(&mut self) -> Option<()> {
        loop {
            tokio::select! {
                _ = self.ticker.tick() => {
                    let now_ms = now_ms();
                    let sync_msg = self.router.create_sync(now_ms);
                    if let Err(e) = self.network.broadcast(&protocol::worker::Event { event: Some(protocol::worker::event::Event::SyncReq(sync_msg.into())) }) {
                        log::error!("[WorkerRunner] broadcast route sync error {e:?}");
                    }
                    self.router.on_tick(now_ms);
                },
                e = self.registry_client.recv() => match e.unwrap().unwrap() {
                    RegistryClientEvent::Answer(from, conn, answer) => {
                        log::info!("[WorkerRunner] answer from {from:?}, {answer}");
                        if let Err(e) = self.network.on_answer(conn, from, answer) {
                            log::error!("[WorkerRunner] process answer error {e:?}");
                        }
                    },
                    RegistryClientEvent::Offer(from, conn, offer) => {
                        log::info!("[WorkerRunner] offer from {from:?}, {offer}");
                        let answer = self.network.on_offer(conn, from.clone(), &offer).map_err(|e| format!("{e:?}"));
                        self.registry_client.answer(from, conn, answer.map_err(|e| AnswerError::Remote(e)))
                    },
                    RegistryClientEvent::Neighbours(nodes) => {
                        for node in nodes {
                            if let Some((conn, sdp)) = self.network.connect(node.clone()) {
                                let req_id = self.registry_client.offer(node, conn, &sdp);
                                log::info!("[WorkerRunner] creating outgoing conn {conn:?} {req_id:?}");
                            }
                        }
                    },
                },
                e = self.network.recv() => match e.unwrap() {
                    NodeEvent::NodeConnected(_, _) => {},
                    NodeEvent::NodeStats(_, _, _) => {},
                    NodeEvent::NodeMsg(_, remote, msg) => {
                        if let Some(out) = self.on_remote_msg(remote.clone(), msg) {
                            return Some(out);
                        }
                    },
                    NodeEvent::NodeDisconnected(_, _) => {},
                },
                res = self.llm_res_rx.recv() => match res.unwrap() {
                    LlmRes::Start(session) => {
                        let llm_session = self.sessions.get_mut(&session).unwrap();
                        let (next, event) = llm_session.start_req_next();
                        if let Err(e) = self.network.send(next, &event) {
                            log::error!("[WorkerRunner] start_req: next layer event error {e:?}");
                        }
                    },
                    LlmRes::Backward(session, step, payload, seq_len, index_pos) => {
                        let llm_session = self.sessions.get(&session).unwrap();
                        let (next, event) = llm_session.forward_req_next(step, payload, seq_len, index_pos);
                        if let Err(e) = self.network.send(next, &event) {
                            log::error!("[WorkerRunner] forward_req: next layer event error {e:?}");
                        }
                    },
                    LlmRes::End(session) => {
                        let llm_session = self.sessions.get(&session).unwrap();
                        let (next, event) = llm_session.end_req_next();
                        if let Err(e) = self.network.send(next, &event) {
                            log::error!("[WorkerRunner] end_req: next layer event error {e:?}");
                        }
                    },
                },
                e = self.session_control_rx.recv() => match e {
                    Some((session, req, tx)) => {
                        self.on_local_session_req(session, req, tx)?;
                    },
                    None => {
                        break None;
                    }
                }
            }
        }
    }
}

impl<const MODEL_LAYERS: usize> WorkerRunner<MODEL_LAYERS> {
    fn on_local_session_req(&mut self, session: Session, req: SessionReq, tx: oneshot::Sender<SessionRes>) -> Option<()> {
        match req {
            SessionReq::Start => {
                let action = self.router.select_next(0)?;
                log::info!("[WorkerRunner] select action {action:?} for session {session:?} with next layer is 0");
                let mut llm_session = LlmSession::new(session, None, action);
                llm_session.set_res_tx(tx);
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    log::info!("[WorkerRunner] session {session:?} process start_req local");
                    self.llm_req_tx.try_send(LlmReq::Start(session)).unwrap();
                } else {
                    log::info!("[WorkerRunner] session {session:?} don't process start_req local");
                    let (next, event) = llm_session.start_req_next();
                    self.network.send(next, &event).ok()?;
                }
                self.sessions.insert(session, llm_session);
                Some(())
            }
            SessionReq::Forward(step, payload, seq_len, index_pos) => {
                let llm_session = self.sessions.get_mut(&session)?;
                llm_session.set_res_tx(tx);
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    log::info!("[WorkerRunner] session {session:?} process forward_req local");
                    self.llm_req_tx.try_send(LlmReq::Forward(session, step, payload, seq_len, index_pos)).unwrap();
                } else {
                    log::info!("[WorkerRunner] session {session:?} don't process forward_req local");
                    let (next, event) = llm_session.forward_req_next(step, payload, seq_len, index_pos);
                    self.network.send(next, &event).ok()?;
                }
                Some(())
            }
            SessionReq::Stop => {
                let llm_session = self.sessions.get_mut(&session)?;
                llm_session.set_res_tx(tx);
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    log::info!("[WorkerRunner] session {session:?} process end_req local");
                    self.llm_req_tx.try_send(LlmReq::End(session)).unwrap();
                } else {
                    log::info!("[WorkerRunner] session {session:?} don't process end_req local");
                    let (next, event) = llm_session.end_req_next();
                    self.network.send(next, &event).ok()?;
                }
                Some(())
            }
        }
    }

    fn on_remote_msg(&mut self, remote: NodeId, msg: protocol::worker::Event) -> Option<()> {
        match msg.event? {
            protocol::worker::event::Event::SyncReq(req) => {
                log::info!("[WorkerRunner] on SyncReq from {remote:?}");
                const FAKE_RTT_MS: u32 = 100; //TODO get real rtt
                self.router.apply_sync(remote.clone(), FAKE_RTT_MS, req.into());
                if let Err(e) = self.network.send(
                    remote.clone(),
                    &protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::SyncRes(protocol::worker::event::SyncRes {})),
                    },
                ) {
                    log::error!("[WorkerRunner] forward res to pre {remote:?} error {e:?}");
                }
                Some(())
            }
            protocol::worker::event::Event::SyncRes(res) => {
                log::info!("[WorkerRunner] on SyncRes from {remote:?}");
                None
            }
            protocol::worker::event::Event::StartReq(req) => {
                let session = Session(req.session_id);
                let action = self.router.select_next(req.from_layer)?;
                log::info!("[WorkerRunner] select action {action:?} for session {:?} with next layer is {}", req.session_id, req.from_layer);
                let llm_session = LlmSession::new(session, Some(remote), action);
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    self.llm_req_tx.try_send(LlmReq::Start(session)).unwrap();
                } else {
                    let (next, event) = llm_session.start_req_next();
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] forward res to pre {next:?} error {e:?}");
                    }
                }
                self.sessions.insert(session, llm_session);
                None
            }
            protocol::worker::event::Event::StartRes(res) => {
                let session = Session(res.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
                // if it is local event then we should have res_tx
                if let Some(tx) = llm_session.take_res_tx() {
                    tx.send(SessionRes::Started(remote)).unwrap();
                    None
                } else {
                    let (next, event) = llm_session.start_res_next();
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] start res to pre {next:?} error {e:?}");
                    }
                    None
                }
            }
            protocol::worker::event::Event::ForwardReq(req) => {
                let session = Session(req.session_id);
                let llm_session = self.sessions.get(&session)?;
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    self.llm_req_tx.try_send(LlmReq::Forward(session, req.step, req.payload, req.seq_len, req.index_pos)).unwrap();
                } else {
                    let (next, event) = llm_session.forward_req_next(req.step, req.payload, req.seq_len, req.index_pos);
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] forward req to next {next:?} error {e:?}");
                    }
                }
                None
            }
            protocol::worker::event::Event::ForwardRes(res) => {
                let session = Session(res.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
                // if it is local req, then we should have res_tx
                if let Some(tx) = llm_session.take_res_tx() {
                    tx.send(SessionRes::Backward(res.step, res.payload, res.seq_len, res.index_pos)).unwrap();
                    None
                } else {
                    let (next, event) = llm_session.forward_res_next(res.step, res.payload, res.seq_len, res.index_pos);
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] forward res to pre {next:?} error {e:?}");
                    }
                    None
                }
            }
            protocol::worker::event::Event::EndReq(req) => {
                let session = Session(req.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
                // if need process local, then first wake it up, then wait it finish
                if llm_session.is_local_process() {
                    self.llm_req_tx.try_send(LlmReq::End(session)).unwrap();
                } else {
                    let (next, event) = llm_session.end_req_next();
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] end req to next {next:?} error {e:?}");
                    }
                }
                None
            }
            protocol::worker::event::Event::EndRes(res) => {
                let session = Session(res.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
                // if it is local then we should have res_tx
                if let Some(tx) = llm_session.take_res_tx() {
                    tx.send(SessionRes::Stopped(remote)).unwrap();
                } else {
                    let (next, event) = llm_session.end_res_next();
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] end res to pre {next:?} error {e:?}");
                    }
                };
                self.sessions.remove(&session);
                None
            }
        }
    }
}

/// Get current timestamp in ms
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
}
