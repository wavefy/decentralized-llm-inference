mod communication;
mod model_service;
mod rpc;
mod virtual_model_layers;

use std::{ops::Range, sync::Arc};

use candle_core::{Device, Tensor};
pub use communication::*;
use model_router::RouteTable;
use model_service::ModelService;
use models::ModelLayersWorker;
use p2p_network::addr::NodeId;
use protocol::worker::event::{RpcReq, RpcRes};
use rpc::create_rpc;
use spin::RwLock;
pub use virtual_model_layers::*;

#[async_trait::async_trait]
pub trait ServiceHandler<const MODEL_LAYERS: usize>: Send + Sync + 'static {
    async fn on_req(&self, from: NodeId, req: RpcReq) -> RpcRes;
}

pub struct WorkerRunner<const MODEL_LAYERS: usize> {
    communication: WorkerComunication<MODEL_LAYERS>,
}

impl<const MODEL_LAYERS: usize> WorkerRunner<MODEL_LAYERS> {
    pub async fn new<LW: ModelLayersWorker<(Tensor, u32)>>(
        registry_endpoint: &str,
        model: &str,
        node_id: &str,
        range: Range<u32>,
        layers: LW,
        device: Device,
    ) -> (Self, VirtualModelLayers<LW, MODEL_LAYERS>) {
        let router = Arc::new(RwLock::new(RouteTable::new(range.clone())));
        let (rpc_client, rpc_rx) = create_rpc();
        let model_service = Arc::new(ModelService::new(layers, device.clone(), rpc_client.clone(), router.clone()));

        let communication = WorkerComunication::new(registry_endpoint, model, node_id, range, router, rpc_rx, model_service.clone()).await;

        (Self { communication }, VirtualModelLayers { device, model_service })
    }

    pub async fn shutdown(&mut self) {
        self.communication.shutdown().await
    }

    pub async fn recv(&mut self) -> Option<()> {
        self.communication.recv().await
    }
}
