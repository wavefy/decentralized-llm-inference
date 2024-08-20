use network::{addr::NodeId, node::NetworkNode};
use protocol::{ModelLayersRanger, Session};
use registry::client::RegistryClient;

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
    pub async fn new(
        registry_endpoint: &str,
        model: &str,
        node_id: &str,
        from: u32,
        to: u32,
    ) -> Self {
        log::info!(
            "[WorkerRunner] start with node {node_id} with model {model}, layers [{from}-{to}]"
        );
        let node_id = NodeId(node_id.to_string());
        let mut registry_client =
            RegistryClient::new(registry_endpoint, model, node_id.clone()).await;
        registry_client.update_layer(ModelLayersRanger { from, to });

        Self {
            registry_client,
            network: NetworkNode::new(node_id),
        }
    }

    pub fn session_req(&mut self, session: Session, req: SessionReq) {
        todo!()
    }

    pub async fn recv(&mut self) -> Option<WorkerRunnerEvent> {
        loop {
            let incoming = self.registry_client.recv().await?;
        }
    }
}
