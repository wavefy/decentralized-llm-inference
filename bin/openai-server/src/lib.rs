use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
    ops::Range,
    str::FromStr,
    sync::Arc,
};

use api_chat::{chat_completions, get_model, list_models};
use api_status::{p2p_start, p2p_status, p2p_stop, p2p_suggest_layers, P2pState};
use candle_core::DType;
use models::{fake, get_device, llama, phi3, ChatModel};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use tokio::{
    signal,
    sync::{mpsc::{channel, Receiver}, oneshot, Mutex},
};
use usage_service::WorkerUsageService;
use worker::{WorkerEvent, WorkerEventWithResp, WorkerRunner};

mod api_chat;
mod api_status;

pub async fn start_control_server(control_http_bind: SocketAddr, registry_server: &str, node_id: &str, openai_http_bind: SocketAddr, stun_server: &str) {
    let app = Route::new()
        .at("/v1/p2p/status", poem::get(p2p_status))
        .at("/v1/p2p/suggest_layers", poem::get(p2p_suggest_layers))
        .at("/v1/p2p/start", poem::post(p2p_start))
        .at("/v1/p2p/stop", poem::post(p2p_stop))
        .data(P2pState {
            registry_server: registry_server.to_string(),
            node_id: node_id.to_string(),
            http_bind: openai_http_bind,
            stun_server: stun_server.to_string(),
            model: Arc::new(Mutex::new(None)),
        })
        .with(Cors::new());

    Server::new(TcpListener::bind(control_http_bind)).run(app).await.unwrap();
}

pub async fn start_server(registry_server: &str, model: &str, node_id: &str, layers: Range<u32>, http_bind: SocketAddr, stun_server: &str, query_rx: Receiver<WorkerControl>, usage_service: Arc<dyn WorkerUsageService>) {
    let stun_servers = stun_server.to_socket_addrs().unwrap().collect::<Vec<_>>();
    log::info!("[OpenAIServer] start with model {} and stun server {}", model, stun_server);
    let device = get_device(false).unwrap();

    match model {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, layers.clone(), &device).await.unwrap();
            let (mut worker, virtual_model_layers) = WorkerRunner::<32>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, query_rx).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = llama::LlamaModel::new(device.clone(), DType::F16, virtual_model_layers, false).await;
            run(&mut worker, Arc::new(model_exe), http_bind, query_rx).await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(layers.clone());
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = fake::FakeModel::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, query_rx).await;
        }
        _ => panic!("unsupported"),
    }
}

#[derive(Debug)]
pub struct WorkerStatus {
    pub ready: bool,
    pub peers: Vec<String>,
    pub sessions: Vec<u64>,
}

pub enum WorkerControl {
    Status(oneshot::Sender<WorkerStatus>),
    Stop(oneshot::Sender<()>),
}

async fn run<const MODEL_LAYERS: usize>(worker: &mut WorkerRunner<MODEL_LAYERS>, model_exe: Arc<dyn ChatModel>, http_bind: SocketAddr, mut query_rx: Receiver<WorkerControl>) {
    let app = Route::new()
        .at("/v1/chat/completions", poem::post(chat_completions).data(model_exe))
        .at("/v1/models", poem::get(list_models))
        .at("/v1/models/:model_id", poem::get(get_model))
        .with(Cors::new());

    let http_server = tokio::spawn(async move { Server::new(TcpListener::bind(http_bind)).run(app).await });

    loop {
        tokio::select! {
            e = worker.recv() => match e {
                Some(_e) => {},
                None => {
                    log::error!("[OpenAIServer] worker closed");
                    break;
                },
            },
            e = query_rx.recv() => match e {
                Some(WorkerControl::Status(sender)) => {
                    sender.send(WorkerStatus {
                        ready: worker.ready(),
                        peers: worker.peers().iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        sessions: worker.sessions(),
                    }).unwrap();
                }
                Some(WorkerControl::Stop(sender)) => {
                    sender.send(()).unwrap();
                    http_server.abort();
                    break;
                }
                None => {
                    log::error!("[OpenAIServer] query rx closed");
                    break;
                }
            },
            // TODO: current enable this will cause cannot stop server
            // _ = signal::ctrl_c() => {
            //     worker.shutdown().await;
            //     break;
            // },
        }
    }
}
