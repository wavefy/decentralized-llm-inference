use std::{collections::HashMap, sync::Arc};

use contract::{
    aptos_sdk::{rest_client::AptosBaseUrl, types::LocalAccount},
    OnChainService,
};
use openai_http::ModelStore;
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Query},
    Body, Response,
};
use registry::client::{get_layers_distribution, select_layers, LayerSelectionRes};
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{channel, Sender},
    oneshot, Mutex,
};

use crate::worker::{run_model_worker, WorkerControl};

pub struct ModelState {
    pub model: String,
    pub from_layer: u32,
    pub to_layer: u32,
    pub query_tx: Sender<WorkerControl>,
    pub wallet: Arc<OnChainService>,
}

#[derive(Clone)]
pub struct P2pState {
    pub registry_server: String,
    pub node_id: String,
    pub stun_server: String,
    pub store: ModelStore,
    pub models: Arc<Mutex<HashMap<String, ModelState>>>,
}

#[derive(Debug, Serialize)]
struct ModelStatus {
    status: String,
    model: String,
    from_layer: u32,
    to_layer: u32,
    peers: Vec<String>,
    sessions: u32,
    wallet: WalletStatus,
}

#[derive(Debug, Serialize)]
struct WalletStatus {
    spending: u64,
    earning: u64,
    balance: Option<u64>,
    topup_balance: Option<u64>,
    address: String,
}

#[derive(Debug, Serialize)]
struct P2pStatusRes {
    models: Vec<ModelStatus>,
}

#[handler]
pub async fn p2p_status(data: Data<&P2pState>) -> Response {
    let models = data.models.lock().await;
    let mut list_models = vec![];

    for model in models.values() {
        let (tx, rx) = oneshot::channel();
        model.query_tx.send(WorkerControl::Status(tx)).await.unwrap();
        let status = rx.await.unwrap();

        let status = ModelStatus {
            status: if status.ready {
                "ready"
            } else {
                "incomplete"
            }
            .to_string(),
            peers: status.peers,
            sessions: status.sessions.len() as u32,
            model: model.model.clone(),
            from_layer: model.from_layer,
            to_layer: model.to_layer,
            wallet: WalletStatus {
                spending: model.wallet.spending_token_count(),
                earning: model.wallet.earning_token_count(),
                balance: model.wallet.current_balance().await,
                topup_balance: model.wallet.topup_balance().await,
                address: model.wallet.address().to_string(),
            },
        };
        list_models.push(status);
    }

    let status = P2pStatusRes { models: list_models };
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
    let mut models = data.models.lock().await;
    if models.contains_key(&body.model) {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from_string("Model already started".to_string()));
    }

    let registry_server = data.registry_server.clone();
    let node_id = data.node_id.clone();
    let stun_server = data.stun_server.clone();
    let model = body.model.clone();
    let range = body.from_layer..body.to_layer;
    let (query_tx, query_rx) = channel(10);
    let account = LocalAccount::from_private_key(&body.private_key, 0).expect("Invalid private key");
    let onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet, range.clone());
    onchain_service.init().await;

    let usage_service = Arc::new(onchain_service);
    let wallet = usage_service.clone();
    let store = data.store.clone();
    tokio::spawn(async move {
        run_model_worker(&registry_server, &model, &node_id, range, &stun_server, query_rx, usage_service, store).await;
    });
    let model_state = ModelState {
        model: body.model.clone(),
        from_layer: body.from_layer,
        to_layer: body.to_layer,
        query_tx,
        wallet,
    };
    models.insert(body.model.clone(), model_state);
    Response::builder().status(StatusCode::OK).body(Body::from_json(&P2pStartRes {}).unwrap())
}

#[derive(Debug, Deserialize)]
struct P2pStop {
    model: String,
}

#[derive(Debug, Serialize)]
struct P2pStopRes {}

#[handler]
pub async fn p2p_stop(body: Json<P2pStop>, data: Data<&P2pState>) -> Response {
    let mut models = data.models.lock().await;
    if let Some(model) = models.get_mut(&body.model) {
        let (tx, rx) = oneshot::channel();
        log::info!("p2p_stop: sending stop signal");
        model.query_tx.send(WorkerControl::Stop(tx)).await.unwrap();
        rx.await.unwrap();
        log::info!("p2p_stop: stop ack signal received");
        models.remove(&body.model);
        Response::builder().status(StatusCode::OK).body(Body::from_json(&P2pStopRes {}).unwrap())
    } else {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::from_string("Model not started".to_string()));
    }
}

#[derive(Debug, Serialize)]
struct P2pSuggestLayersRes {
    distribution: Vec<usize>,
    min_layers: Option<u32>,
    from_layer: Option<u32>,
    to_layer: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct P2pSuggestLayers {
    model: String,
    layers: u32,
    max_layers: u32,
}

#[handler]
pub async fn p2p_suggest_layers(query: Query<P2pSuggestLayers>, data: Data<&P2pState>) -> Response {
    log::info!("[OpenAIServer] p2p_suggest_layers model: {}, layers: {}, max_layers: {}", query.model, query.layers, query.max_layers);
    match get_layers_distribution(&data.registry_server, &query.model).await {
        Ok(mut distrbution) => {
            while distrbution.layers.len() < query.max_layers as usize {
                distrbution.layers.push(0);
            }
            log::info!("distribution: {} {} {:?}", distrbution.layers.len(), query.max_layers, distrbution.layers);
            let suggested_layers = select_layers(&distrbution.layers, query.layers);
            log::info!("selected_layers: {suggested_layers:?}");

            Response::builder().status(StatusCode::OK).body(match suggested_layers {
                LayerSelectionRes::EnoughLayers { ranges } => Body::from_json(&P2pSuggestLayersRes {
                    distribution: distrbution.layers,
                    min_layers: None,
                    from_layer: Some(ranges.start),
                    to_layer: Some(ranges.end),
                })
                .unwrap(),
                LayerSelectionRes::NotEnoughLayers { min_layers } => Body::from_json(&P2pSuggestLayersRes {
                    distribution: distrbution.layers,
                    min_layers: Some(min_layers),
                    from_layer: None,
                    to_layer: None,
                })
                .unwrap(),
            })
        }
        Err(err) => {
            log::error!("Failed to get layers distribution: {}", err);
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from_string(err))
        }
    }
}
