use network::{
    addr::NodeId,
    node::{NetworkNode, NodeEvent, OutgoingError},
};
use protocol::{ModelLayersRanger, Session};
use registry::{
    client::{RegistryClient, RegistryClientEvent},
    AnswerError,
};

pub enum SessionReq {
    Start,
    Forward,
    Stop,
}

pub enum SessionRes {
    Started,
    Backward,
    Stopped,
}

pub enum WorkerRunnerEvent {
    Session(Session, SessionRes),
}

pub struct WorkerRunner {
    registry_client: RegistryClient,
    network: NetworkNode,
}

impl WorkerRunner {
    pub async fn new(registry_endpoint: &str, model: &str, node_id: &str, from: u32, to: u32) -> Self {
        log::info!("[WorkerRunner] start with node {node_id} with model {model}, layers [{from}-{to}]");
        let node_id = NodeId(node_id.to_string());
        let mut registry_client = RegistryClient::new(registry_endpoint, model, node_id.clone()).await;
        registry_client.update_layer(ModelLayersRanger { from, to });

        Self {
            registry_client,
            network: NetworkNode::new(node_id).await,
        }
    }

    pub fn session_req(&mut self, session: Session, req: SessionReq) {
        todo!()
    }

    pub async fn shutdown(&mut self) {
        self.registry_client.shutdown().await;
        self.network.shutdown().await
    }

    pub async fn recv(&mut self) -> Option<WorkerRunnerEvent> {
        loop {
            tokio::select! {
                e = self.registry_client.recv() => match e?.ok()? {
                    RegistryClientEvent::Answer(from, conn, answer) => {
                        log::info!("[WorkerRunner] answer from {from:?}, {answer}");
                        if let Err(e) = self.network.on_answer(conn, from, answer).await {
                            log::error!("[WorkerRunner] process answer error {e:?}");
                        }
                    },
                    RegistryClientEvent::Offer(from, conn, offer) => {
                        log::info!("[WorkerRunner] offer from {from:?}, {offer}");
                        let answer = self.network.on_offer(conn, from.clone(), &offer).await.map_err(|e| format!("{e:?}"));
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
                e = self.network.recv() => match e? {
                    NodeEvent::NodeConnected(_, _) => {},
                    NodeEvent::NodeStats(_, _, _) => {},
                    NodeEvent::NodeMsg(_, _, _) => {},
                    NodeEvent::NodeDisconnected(_, _) => {},
                }
            }
        }
    }
}
