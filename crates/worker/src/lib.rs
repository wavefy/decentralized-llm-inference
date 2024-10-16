mod communication;
mod model_service;
mod rpc;
mod virtual_model_layers;

use std::{net::SocketAddr, ops::Range, sync::Arc};

use candle_core::{Device, Tensor};
pub use communication::*;
use model_router::RouteTable;
use model_service::ModelService;
pub use model_service::{WorkerEvent, WorkerEventWithResp};
use models::ModelLayersWorker;
use p2p_network::addr::NodeId;
use protocol::{
    registry::to_registry::Stats,
    worker::event::{RpcReq, RpcRes},
};
use rpc::create_rpc;
use spin::RwLock;
use usage_service::WorkerUsageService;
pub use virtual_model_layers::*;

#[async_trait::async_trait]
pub trait ServiceHandler<const MODEL_LAYERS: usize>: Send + Sync + 'static {
    fn tick(&self);
    fn sessions(&self) -> Vec<u64>;
    fn stats(&self) -> Stats;
    async fn on_req(&self, from: NodeId, req: RpcReq) -> RpcRes;
}

pub struct WorkerRunner<const MODEL_LAYERS: usize> {
    communication: WorkerCommunication<MODEL_LAYERS>,
}

impl<const MODEL_LAYERS: usize> WorkerRunner<MODEL_LAYERS> {
    pub async fn new<LW: ModelLayersWorker<(Tensor, u32)>>(
        registry_endpoint: &str,
        model: &str,
        node_id: &str,
        range: Range<u32>,
        layers: LW,
        device: Device,
        stun_servers: Vec<SocketAddr>,
        usage_service: Arc<dyn WorkerUsageService>,
    ) -> (Self, VirtualModelLayers<LW, MODEL_LAYERS>) {
        let router = Arc::new(RwLock::new(RouteTable::new(range.clone())));
        let (rpc_client, rpc_rx) = create_rpc();
        let model_service = Arc::new(ModelService::new(layers, device.clone(), rpc_client.clone(), router.clone(), usage_service));
        let communication = WorkerCommunication::new(registry_endpoint, model, node_id, range, router, rpc_rx, model_service.clone(), stun_servers).await;
        (Self { communication }, VirtualModelLayers { device, model_service })
    }

    pub fn ready(&self) -> bool {
        self.communication.ready()
    }

    pub fn peers(&self) -> Vec<NodeId> {
        self.communication.peers()
    }

    pub fn sessions(&self) -> Vec<u64> {
        self.communication.sessions()
    }

    pub async fn shutdown(&mut self) {
        self.communication.shutdown().await
    }

    pub async fn recv(&mut self) -> Option<()> {
        self.communication.recv().await
    }
}
