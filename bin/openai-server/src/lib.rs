use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use api_control::{p2p_start, p2p_status, p2p_stop, p2p_suggest_layers, ModelState, P2pState};
use clap::{Parser, Subcommand};
use contract::{
    aptos_sdk::{rest_client::AptosBaseUrl, types::LocalAccount},
    OnChainService,
};
use openai_http::ModelStore;
use poem::{
    listener::TcpListener,
    middleware::{Cors, Tracing},
    EndpointExt, Route, Server,
};
use tokio::sync::{mpsc::channel, Mutex};
use worker::run_model_worker;

mod api_control;
mod worker;

#[derive(Debug, Subcommand)]
pub enum ServerMode {
    Contributor(ContributorMode),
    Gateway(GatewayMode),
}

#[derive(Debug, Parser)]
pub struct ContributorMode {}

#[derive(Debug, Parser)]
pub struct GatewayMode {
    /// Private key for gateway mode
    #[arg(env, long)]
    private_key: String,

    /// Models: phi3, llama32-1b, llama32-3b, llama32-vision-11b
    #[arg(env, long)]
    models: Vec<String>,
}

pub async fn start_http_server(http_bind: SocketAddr, registry_server: &str, node_id: &str, stun_server: &str, mode: ServerMode) {
    let store = ModelStore::default();
    match mode {
        ServerMode::Contributor(_) => {
            let p2p_app = Route::new()
                .at("/v1/status", poem::get(p2p_status))
                .at("/v1/suggest_layers", poem::get(p2p_suggest_layers))
                .at("/v1/start", poem::post(p2p_start))
                .at("/v1/stop", poem::post(p2p_stop))
                .data(P2pState {
                    registry_server: registry_server.to_string(),
                    node_id: node_id.to_string(),
                    store: store.clone(),
                    stun_server: stun_server.to_string(),
                    models: Default::default(),
                });

            let (chat_tx, mut chat_rx) = channel(10);
            let openai_app = openai_http::create_route(chat_tx, store.clone());
            let app = Route::new().nest("/p2p/", p2p_app).nest("/", openai_app).with(Cors::new()).with(Tracing::default());

            tokio::spawn(async move {
                while let Some(req) = chat_rx.recv().await {
                    let _ = store.send_model(req).await;
                }
            });

            Server::new(TcpListener::bind(http_bind)).run(app).await.unwrap();
        }
        ServerMode::Gateway(gateway) => {
            let mut controls = HashMap::new();
            let model_ids = gateway.models;
            let models: Arc<Mutex<HashMap<String, ModelState>>> = Default::default();
            for model_id in model_ids {
                let range = 0..0;
                let (control_tx, control_rx) = channel(10);
                controls.insert(model_id.to_string(), control_tx.clone());
                let account = LocalAccount::from_private_key(&gateway.private_key, 0).expect("Invalid private key");
                let onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet, range.clone());
                onchain_service.init().await;
                let usage_service = Arc::new(onchain_service);
                let registry_server = registry_server.to_owned();
                let model = model_id.to_owned();
                let node_id = node_id.to_owned();
                let stun_server = stun_server.to_owned();
                let store = store.clone();
                let model_state = ModelState {
                    model: model_id.to_string(),
                    from_layer: range.start,
                    to_layer: range.end,
                    query_tx: control_tx.clone(),
                    wallet: usage_service.clone(),
                };
                models.lock().await.insert(model_id.to_string(), model_state);
                tokio::spawn(async move { run_model_worker(&registry_server, &model, &node_id, range, &stun_server, control_rx, usage_service, store.clone()).await });
            }

            let (chat_tx, mut chat_rx) = channel(10);
            let openai_app = openai_http::create_route(chat_tx, store.clone());
            let p2p_app = Route::new().at("/v1/status", poem::get(p2p_status)).data(P2pState {
                registry_server: registry_server.to_string(),
                node_id: node_id.to_string(),
                store: store.clone(),
                stun_server: stun_server.to_string(),
                models,
            });
            let app = Route::new().nest("/p2p", p2p_app).nest("/", openai_app).with(Cors::new()).with(Tracing::default());

            tokio::spawn(async move {
                while let Some(req) = chat_rx.recv().await {
                    let _ = store.send_model(req).await;
                }
            });

            Server::new(TcpListener::bind(http_bind)).run(app).await.unwrap();
        }
    }
}
