use std::{net::SocketAddr, sync::Arc};

use contract::{aptos_sdk::{rest_client::AptosBaseUrl, types::LocalAccount}, OnChainService};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Body, Response,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        mpsc::{channel, Sender},
        oneshot, Mutex,
    },
    task::JoinHandle,
};

use crate::{start_server, WorkerControl};

pub struct ModelState {
    model: String,
    from_layer: u32,
    to_layer: u32,
    runner: JoinHandle<()>,
    query_tx: Sender<WorkerControl>,
}

#[derive(Clone)]
pub struct P2pState {
    pub registry_server: String,
    pub node_id: String,
    pub http_bind: SocketAddr,
    pub stun_server: String,
    pub model: Arc<Mutex<Option<ModelState>>>,
}

#[derive(Debug, Serialize)]
struct ModelConfig {
    model: String,
    from_layer: u32,
    to_layer: u32,
}

#[derive(Debug, Serialize)]
struct P2pStatusRes {
    model: Option<ModelConfig>,
    spent: u64,
    earned: u64,
    balance: u64,
    peers: u32,
    sessions: u32,
    status: String,
}

#[handler]
pub async fn p2p_status(data: Data<&P2pState>) -> Response {
    let model = data.model.lock().await;
    let status = if let Some(model) = model.as_ref() {
        let (tx, rx) = oneshot::channel();
        model.query_tx.send(WorkerControl::Status(tx)).await.unwrap();
        let status = rx.await.unwrap();

        P2pStatusRes {
            status: if status.ready {
                "ready"
            } else {
                "incomplete"
            }
            .to_string(),
            model: Some(ModelConfig {
                model: model.model.clone(),
                from_layer: model.from_layer,
                to_layer: model.to_layer,
            }),
            spent: 0,
            earned: 0,
            balance: 0,
            peers: status.peers.len() as u32,
            sessions: status.sessions.len() as u32,
        }
    } else {
        P2pStatusRes {
            status: "stopped".to_string(),
            model: None,
            spent: 0,
            earned: 0,
            balance: 0,
            peers: 0,
            sessions: 0,
        }
    };

    Response::builder().header("Content-Type", "application/json").body(Body::from_json(&status).unwrap())
}

#[derive(Debug, Deserialize)]
struct P2pStart {
    model: String,
    from_layer: u32,
    to_layer: u32,
    private_key: String,
}

#[derive(Debug, Serialize)]
struct P2pStartRes {}

#[handler]
pub async fn p2p_start(body: Json<P2pStart>, data: Data<&P2pState>) -> Response {
    let mut current_model = data.model.lock().await;
    if current_model.is_some() {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from_string("Model already started".to_string()));
    }

    let registry_server = data.registry_server.clone();
    let node_id = data.node_id.clone();
    let http_bind = data.http_bind;
    let stun_server = data.stun_server.clone();
    let model = body.model.clone();
    let range = body.from_layer..body.to_layer;
    let (query_tx, query_rx) = channel(10);
    let account = LocalAccount::from_private_key(&body.private_key, 0).expect("Invalid private key");
    let onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet);
    onchain_service.init().await;

    let usage_service = Arc::new(onchain_service);
    let runner = tokio::spawn(async move {
        start_server(&registry_server, &model, &node_id, range, http_bind, &stun_server, query_rx, usage_service).await;
    });
    let model = ModelState {
        model: body.model.clone(),
        from_layer: body.from_layer,
        to_layer: body.to_layer,
        runner,
        query_tx,
    };
    *current_model = Some(model);
    Response::builder().status(StatusCode::OK).body(Body::from_json(&P2pStartRes {}).unwrap())
}
#[derive(Debug, Serialize)]
struct P2pStopRes {}

#[handler]
pub async fn p2p_stop(data: Data<&P2pState>) -> Response {
    let mut current_model = data.model.lock().await;
    if let Some(model) = current_model.as_ref() {
        model.runner.abort();
        *current_model = None;
        Response::builder().status(StatusCode::OK).body(Body::from_json(&P2pStopRes {}).unwrap())
    } else {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from_string("Model not started".to_string()));
    }
}
