use std::{
    net::{SocketAddr, ToSocketAddrs},
    ops::Range,
    sync::Arc,
};

use api::{chat_completions, get_model, list_models};
use candle_core::DType;
use models::{fake, get_device, llama, phi3, ChatModel};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use tokio::signal;
use worker::WorkerRunner;

mod api;

pub async fn start_server(registry_server: &str, model: &str, node_id: &str, layers: Range<u32>, http_bind: SocketAddr, stun_server: &str) {
    let stun_servers = stun_server.to_socket_addrs().unwrap().collect::<Vec<_>>();
    log::info!("[OpenAIServer] start with model {} and stun server {}", model, stun_server);
    let device = get_device(false).unwrap();
    match model {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, layers.clone(), &device).await.unwrap();
            let (mut worker, virtual_model_layers) = WorkerRunner::<32>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers).await;
            let model_exe = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers).await;
            let model_exe = llama::LlamaModel::new(device.clone(), DType::F16, virtual_model_layers, false).await;
            run(&mut worker, Arc::new(model_exe), http_bind).await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(layers.clone());
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers).await;
            let model_exe = fake::FakeModel::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind).await;
        }
        _ => panic!("unsupported"),
    }
}

async fn run<const MODEL_LAYERS: usize>(worker: &mut WorkerRunner<MODEL_LAYERS>, model_exe: Arc<dyn ChatModel>, http_bind: SocketAddr) {
    let app = Route::new()
        .at("/v1/chat/completions", poem::post(chat_completions).data(model_exe))
        .at("/v1/models", poem::get(list_models))
        .at("/v1/models/:model_id", poem::get(get_model))
        .with(Cors::new());

    tokio::spawn(async move { Server::new(TcpListener::bind(http_bind)).run(app).await });

    loop {
        tokio::select! {
            e = worker.recv() => match e {
                Some(_e) => {},
                None => {
                    log::error!("[OpenAIServer] worker closed");
                    break;
                },
            },
            _ = signal::ctrl_c() => {
                worker.shutdown().await;
                break;
            },
        }
    }
}
