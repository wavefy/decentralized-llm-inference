use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
    ops::Range,
    sync::Arc,
};

use api::{chat_completions, get_model, list_models};
use candle_core::DType;
use contract::{
    aptos_sdk::{
        rest_client::{aptos_api_types::Address, AptosBaseUrl},
        types::LocalAccount,
    },
    client::{self, OnChainClient},
    storage::OnChainStorage,
    OnChainService,
};
use models::{fake, get_device, llama, phi3, ChatModel};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use tokio::{
    signal,
    sync::mpsc::{channel, Receiver},
};
use worker::{WorkerEvent, WorkerEventWithResp, WorkerRunner};

mod api;


pub async fn start_server(registry_server: &str, model: &str, node_id: &str, layers: Range<u32>, http_bind: SocketAddr, stun_server: &str , private_key: &str, contract_address: &str) {
    let stun_servers = stun_server.to_socket_addrs().unwrap().collect::<Vec<_>>();
    log::info!("[OpenAIServer] start with model {} and stun server {}", model, stun_server);
    let device = get_device(false).unwrap();
    let account = LocalAccount::from_private_key(private_key, 0).expect("Invalid private key");
    let account_address = account.address().to_string();

    let mut onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet, contract_address);
    onchain_service.init().await;

    match model {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, layers.clone(), &device).await.unwrap();
            let (mut worker, virtual_model_layers, worker_event_rx) = WorkerRunner::<32>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, &account_address).await;
            let model_exe = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, onchain_service, worker_event_rx).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers, worker_event_rx) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, &account_address).await;
            let model_exe = llama::LlamaModel::new(device.clone(), DType::F16, virtual_model_layers, false).await;
            run(&mut worker, Arc::new(model_exe), http_bind, onchain_service, worker_event_rx).await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(layers.clone());
            let (mut worker, virtual_model_layers, worker_event_rx) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_server, &account_address).await;
            let model_exe = fake::FakeModel::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, onchain_service, worker_event_rx).await;
        }
        _ => panic!("unsupported"),
    }
}

async fn run<const MODEL_LAYERS: usize>(
    worker: &mut WorkerRunner<MODEL_LAYERS>,
    model_exe: Arc<dyn ChatModel>,
    http_bind: SocketAddr,
    mut onchain_service: OnChainService,
    mut worker_event_rx: Receiver<WorkerEventWithResp>,
) {
    let app = Route::new()
        .at("/v1/chat/completions", poem::post(chat_completions).data(model_exe))
        .at("/v1/models", poem::get(list_models))
        .at("/v1/models/:model_id", poem::get(get_model))
        .with(Cors::new());

    tokio::spawn(async move { Server::new(TcpListener::bind(http_bind)).run(app).await });

    tokio::spawn(async move {
        loop {
            if let Some(WorkerEventWithResp { event, resp }) = worker_event_rx.recv().await {
                if (env::var("IGNORE_CONTRACT").is_ok()) {
                    if let Some(resp) = resp {
                        resp.send(true);
                    }
                } else {
                    match event {
                        WorkerEvent::Start(chat_id, addresses) => {
                            log::info!("[OpenAIServer] WorkerEvent::Start {addresses:?}");
                            let addresses = addresses.iter().map(|a| Address::from_str(a).unwrap()).collect();
                            let res = onchain_service.create_session(chat_id, 100, 100, addresses).await;
                            log::info!("[OpenAIServer] create_session {res:?}");
                            if let Some(resp) = resp {
                                resp.send(res.is_ok());
                            }
                        }
                        WorkerEvent::Forward(chat_id) => {
                            log::info!("[OpenAIServer] WorkerEvent::Forward {chat_id}");
                            onchain_service.increment_chat_token_count(chat_id, 1);
                        }
                        WorkerEvent::End(chat_id, client_address) => {
                            log::info!("[OpenAIServer] WorkerEvent::End {client_address}");
                            let res = onchain_service.commit_token_count(chat_id).await;
                            log::info!("[OpenAIServer] commit_token_count {res:?}");
                            if let Some(resp) = resp {
                                resp.send(res.is_ok());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

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
