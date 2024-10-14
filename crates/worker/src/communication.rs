use std::{net::SocketAddr, ops::Range, sync::Arc, time::Duration};

use model_router::{RoutePath, RouteTable};
use p2p_network::{
    addr::NodeId,
    node::{NetworkNode, NodeEvent, SendError},
};
use protocol::worker::event::RpcRes;
use registry::{
    client::{RegistryClient, RegistryClientEvent},
    AnswerError,
};

use spin::RwLock;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::Interval;

use crate::{rpc::RpcClientRx, ServiceHandler};

pub struct WorkerCommunication<const MODEL_LAYERS: usize> {
    registry_client: RegistryClient,
    router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>,
    network: NetworkNode<protocol::worker::Event>,
    ticker: Interval,
    rpc_handler: Arc<dyn ServiceHandler<MODEL_LAYERS>>,
    rpc_rx: RpcClientRx,
    res_tx: Sender<(NodeId, RpcRes)>,
    res_rx: Receiver<(NodeId, RpcRes)>,
}

impl<const MODEL_LAYERS: usize> WorkerCommunication<MODEL_LAYERS> {
    // TODO make layers_worker generic
    pub async fn new(
        registry_endpoint: &str,
        model: &str,
        node_id: &str,
        range: Range<u32>,
        router: Arc<RwLock<RouteTable<NodeId, MODEL_LAYERS>>>,
        rpc_rx: RpcClientRx,
        rpc_handler: Arc<dyn ServiceHandler<MODEL_LAYERS>>,
        stun_servers: Vec<SocketAddr>,
    ) -> Self {
        log::info!("[WorkerComunication] start with node {node_id} with model {model}, layers [{range:?}] / total {MODEL_LAYERS}");
        let node_id = NodeId(node_id.to_string());
        let mut registry_client = RegistryClient::new(registry_endpoint, model, node_id.clone()).await;
        registry_client.update_layer(range.clone());
        let (res_tx, res_rx) = channel(10);

        Self {
            registry_client,
            router: router.clone(),
            network: NetworkNode::new(node_id, stun_servers).await,
            ticker: tokio::time::interval(Duration::from_millis(1000)),
            res_rx,
            res_tx,
            rpc_handler,
            rpc_rx,
        }
    }

    pub fn ready(&self) -> bool {
        self.router.read().ready()
    }

    pub fn peers(&self) -> Vec<NodeId> {
        self.network.peers()
    }

    pub fn sessions(&self) -> Vec<u64> {
        self.rpc_handler.sessions()
    }

    pub fn send_to(&mut self, dest: NodeId, msg: &protocol::worker::Event) -> Result<(), SendError> {
        self.network.send(dest, msg)?;
        Ok(())
    }

    pub fn route_for(&self, next_layer: u32) -> Option<RoutePath<NodeId>> {
        self.router.read().select_next(next_layer)
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
                    self.rpc_handler.tick();
                    let stats = self.rpc_handler.stats();
                    self.registry_client.update_stats(stats);
                    let sync_msg = self.router.read().create_sync(now_ms);
                    if let Err(e) = self.network.broadcast(&protocol::worker::Event { event: Some(protocol::worker::event::Event::SyncReq(sync_msg.into())) }) {
                        log::error!("[WorkerComunication] broadcast route sync error {e:?}");
                    }
                    self.router.write().on_tick(now_ms);
                },
                e = self.registry_client.recv() => match e.unwrap().unwrap() {
                    RegistryClientEvent::Answer(from, conn, answer) => {
                        log::info!("[WorkerComunication] answer from {from:?}, {answer}");
                        if let Err(e) = self.network.on_answer(conn, from, answer) {
                            log::error!("[WorkerComunication] process answer error {e:?}");
                        }
                    },
                    RegistryClientEvent::Offer(from, conn, offer) => {
                        log::info!("[WorkerComunication] offer from {from:?}, {offer}");
                        let answer = self.network.on_offer(conn, from.clone(), &offer).map_err(|e| format!("{e:?}"));
                        self.registry_client.answer(from, conn, answer.map_err(|e| AnswerError::Remote(e)))
                    },
                    RegistryClientEvent::Neighbours(nodes) => {
                        for node in nodes {
                            if let Some((conn, sdp)) = self.network.connect(node.clone()) {
                                let req_id = self.registry_client.offer(node, conn, &sdp);
                                log::info!("[WorkerComunication] creating outgoing conn {conn:?} {req_id:?}");
                            }
                        }
                    },
                },
                e = self.network.recv() => match e?{
                    NodeEvent::NodeConnected(_, _) => {},
                    NodeEvent::NodeStats(_, _, _) => {},
                    NodeEvent::NodeMsg(_conn, remote, msg) => match msg.event? {
                        protocol::worker::event::Event::SyncReq(req) => {
                            const FAKE_RTT: u32 = 50;
                            self.router.write().apply_sync(remote, FAKE_RTT, req.into());
                        },
                        protocol::worker::event::Event::SyncRes(_) => {},
                        protocol::worker::event::Event::RpcReq(req) => {
                            let handler = self.rpc_handler.clone();
                            let res_tx = self.res_tx.clone();
                            tokio::spawn(async move {
                                let res = handler.on_req(remote.clone(), req).await;
                                res_tx.send((remote, res)).await.expect("Should send to main");
                            });
                        },
                        protocol::worker::event::Event::RpcRes(res) => {
                            self.rpc_rx.on_res(res);
                        },
                    },
                    NodeEvent::NodeDisconnected(_, node) => {
                        self.router.write().on_disconnected(node);
                    },
                },
                e = self.rpc_rx.recv() => {
                    let (dest, req) = e?;
                    if let Err(e) = self.network.send(dest.clone(), &protocol::worker::Event { event: Some(protocol::worker::event::Event::RpcReq(req)) } ) {
                        log::error!("[WorkerCommunication] send rpc req to {dest:?} error {e:?}");
                    }
                },
                e = self.res_rx.recv() => {
                    let (dest, res) = e?;
                    if let Err(e) = self.network.send(dest.clone(), &protocol::worker::Event { event: Some(protocol::worker::event::Event::RpcRes(res)) } ) {
                        log::error!("[WorkerCommunication] send rpc res to {dest:?} error {e:?}");
                    }
                }
            }
        }
    }
}

/// Get current timestamp in ms
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis() as u64
}
