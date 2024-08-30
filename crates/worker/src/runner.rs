use std::{collections::HashMap, time::Duration};

use model_router::ModelRouter;
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
    sync::mpsc::{channel, Receiver, Sender},
    time::{Instant, Interval},
};

mod session;

pub enum LlmReq {
    Start(Session),
    Forward(Session, u32, Vec<u8>),
    End(Session),
}

pub enum LlmRes {
    Start(Session),
    Backward(Session, u32, Vec<u8>),
    End(Session),
}

pub enum SessionReq {
    Start,
    Forward(u32, Vec<u8>),
    Stop,
}

pub enum SessionRes {
    Started(NodeId),
    Backward(u32, Vec<u8>),
    Stopped(NodeId),
}

pub enum WorkerRunnerEvent {
    Session(Session, SessionRes),
}

pub struct WorkerRunner {
    node: NodeId,
    registry_client: RegistryClient,
    router: ModelRouter<NodeId>,
    network: NetworkNode<protocol::worker::Event>,
    ticker: Interval,
    started: Instant,
    sessions: HashMap<Session, LlmSession>,
    llm_req_tx: Sender<LlmReq>,
    llm_res_rx: Receiver<LlmRes>,
}

impl WorkerRunner {
    pub async fn new(registry_endpoint: &str, model: &str, node_id: &str, from: u32, to: u32, total: u32) -> Self {
        log::info!("[WorkerRunner] start with node {node_id} with model {model}, layers [{from}-{to}] / total {total}");
        let node_id = NodeId(node_id.to_string());
        let mut registry_client = RegistryClient::new(registry_endpoint, model, node_id.clone()).await;
        registry_client.update_layer(ModelLayersRanger { from, to });

        let (llm_res_tx, llm_res_rx) = channel(10);
        let (llm_req_tx, mut llm_req_rx) = channel(10);
        // task for running all process
        tokio::spawn(async move {
            while let Some(req) = llm_req_rx.recv().await {
                // TODO implement real
                match req {
                    LlmReq::Start(session) => {
                        log::info!("[WorkerRunner] llm core processing {session:?} start");
                        llm_res_tx.send(LlmRes::Start(session)).await.unwrap();
                    }
                    LlmReq::Forward(session, step, mut payload) => {
                        log::info!("[WorkerRunner] llm core processing {session:?} forward step {step}");
                        payload.fill('a' as u8 + step as u8);
                        llm_res_tx.send(LlmRes::Backward(session, step, payload)).await.unwrap();
                    }
                    LlmReq::End(session) => {
                        log::info!("[WorkerRunner] llm core processing {session:?} end");
                        llm_res_tx.send(LlmRes::End(session)).await.unwrap();
                    }
                }
            }
        });

        Self {
            node: node_id.clone(),
            registry_client,
            router: ModelRouter::new(node_id.clone(), from, to, total),
            network: NetworkNode::new(node_id).await,
            ticker: tokio::time::interval(Duration::from_millis(1000)),
            started: Instant::now(),
            sessions: HashMap::new(),
            llm_req_tx,
            llm_res_rx,
        }
    }

    pub fn session_req(&mut self, session: Session, req: SessionReq) -> Option<()> {
        match req {
            SessionReq::Start => {
                let action = self.router.next_for(0)?;
                log::info!("[WorkerRunner] select action {action:?} for session {session:?} with next layer is 0");
                let llm_session = LlmSession::new(session, None, action);
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
            SessionReq::Forward(step, payload) => {
                let llm_session = self.sessions.get(&session)?;
                if llm_session.is_local_process() {
                    log::info!("[WorkerRunner] session {session:?} process forward_req local");
                    self.llm_req_tx.try_send(LlmReq::Forward(session, step, payload)).unwrap();
                } else {
                    log::info!("[WorkerRunner] session {session:?} don't process forward_req local");
                    let (next, event) = llm_session.forward_req_next(step, payload);
                    self.network.send(next, &event).ok()?;
                }
                Some(())
            }
            SessionReq::Stop => {
                let llm_session = self.sessions.get(&session)?;
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

    pub async fn shutdown(&mut self) {
        self.registry_client.shutdown().await;
        self.network.shutdown().await
    }

    pub async fn recv(&mut self) -> Option<WorkerRunnerEvent> {
        loop {
            tokio::select! {
                _ = self.ticker.tick() => {
                    self.router.on_tick(self.started.elapsed().as_millis() as u64);
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
                        let llm_session = self.sessions.get(&session).unwrap();
                        let (next, event) = llm_session.start_req_next();
                        if let Err(e) = self.network.send(next, &event) {
                            log::error!("[WorkerRunner] start_req: next layer event error {e:?}");
                        }
                    },
                    LlmRes::Backward(session, step, mut payload) => {
                        let llm_session = self.sessions.get(&session).unwrap();
                        if step == 10 && llm_session.is_last() {
                            log::warn!("[WorkerRunner] simulate end");
                            payload = vec![];
                        }
                        let (next, event) = llm_session.forward_req_next(step, payload);
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
                }
            }
        }
    }
}

impl WorkerRunner {
    fn on_remote_msg(&mut self, remote: NodeId, msg: protocol::worker::Event) -> Option<WorkerRunnerEvent> {
        match msg.event? {
            protocol::worker::event::Event::SyncReq(req) => todo!(),
            protocol::worker::event::Event::SyncRes(res) => todo!(),
            protocol::worker::event::Event::StartReq(req) => {
                let session = Session(req.session_id);
                let action = self.router.next_for(req.from)?;
                log::info!("[WorkerRunner] select action {action:?} for session {:?} with next layer is {}", req.session_id, req.from);
                let llm_session = LlmSession::new(session, Some(remote), action);
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
                if llm_session.is_first() {
                    Some(WorkerRunnerEvent::Session(session, SessionRes::Started(remote)))
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
                if llm_session.is_local_process() {
                    self.llm_req_tx.try_send(LlmReq::Forward(session, req.step, req.payload)).unwrap();
                } else {
                    let (next, event) = llm_session.forward_req_next(req.step, req.payload);
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] forward req to next {next:?} error {e:?}");
                    }
                }
                None
            }
            protocol::worker::event::Event::ForwardRes(res) => {
                let session = Session(res.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
                if llm_session.is_first() {
                    Some(WorkerRunnerEvent::Session(session, SessionRes::Backward(res.step, res.payload)))
                } else {
                    let (next, event) = llm_session.forward_res_next(res.step, res.payload);
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] forward res to pre {next:?} error {e:?}");
                    }
                    None
                }
            }
            protocol::worker::event::Event::EndReq(req) => {
                let session = Session(req.session_id);
                let llm_session = self.sessions.get_mut(&session)?;
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
                let out = if llm_session.is_first() {
                    Some(WorkerRunnerEvent::Session(session, SessionRes::Stopped(remote)))
                } else {
                    let (next, event) = llm_session.end_res_next();
                    if let Err(e) = self.network.send(next.clone(), &event) {
                        log::error!("[WorkerRunner] end res to pre {next:?} error {e:?}");
                    }
                    None
                };
                self.sessions.remove(&session);
                out
            }
        }
    }
}
