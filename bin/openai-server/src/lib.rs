use std::{net::SocketAddr, ops::Range, str::FromStr, sync::Arc};

use api::{chat_completions, get_model, list_models};
use candle_core::DType;
use contract::{
    aptos_sdk::{rest_client::{aptos_api_types::Address, AptosBaseUrl}, types::LocalAccount},
    client::OnChainClient,
};
use models::{fake, get_device, llama, phi3, ChatModel};
use poem::{listener::TcpListener, middleware::Cors, EndpointExt, Route, Server};
use tokio::{
    signal,
    sync::mpsc::{channel, Receiver},
};
use worker::{WorkerEvent, WorkerRunner};

mod api;

pub async fn start_server(registry_server: &str, model: &str, node_id: &str, layers: Range<u32>, http_bind: SocketAddr, private_key: &str, contract_address: &str) {
    let device = get_device(false).unwrap();
    let account = LocalAccount::from_private_key(private_key, 0).expect("Invalid private key");
    let account_address = account.address().to_string();
    let onchain_client = OnChainClient::new(account, AptosBaseUrl::Testnet, contract_address);
    onchain_client.update_sequence_number().await.expect("Failed to update sequence number");
    log::info!("[OpenAIServer] onchain client initialized");
    let (worker_event_tx, mut worker_event_rx) = channel(10);
    let current_balance = onchain_client.get_current_balance().await.expect("Failed to get current balance");
    log::info!("[OpenAIServer] current balance: {current_balance}");
    match model {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, layers.clone(), &device).await.unwrap();
            let (mut worker, virtual_model_layers) = WorkerRunner::<32>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), &account_address, worker_event_tx).await;
            let model_exe = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, &mut worker_event_rx, &onchain_client).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), &account_address, worker_event_tx).await;
            let model_exe = llama::LlamaModel::new(device.clone(), DType::F16, virtual_model_layers, false).await;
            run(&mut worker, Arc::new(model_exe), http_bind, &mut worker_event_rx, &onchain_client).await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(layers.clone());
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), &account_address, worker_event_tx).await;
            let model_exe = fake::FakeModel::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, Arc::new(model_exe), http_bind, &mut worker_event_rx, &onchain_client).await;
        }
        _ => panic!("unsupported"),
    }
}

async fn run<const MODEL_LAYERS: usize>(
    worker: &mut WorkerRunner<MODEL_LAYERS>,
    model_exe: Arc<dyn ChatModel>,
    http_bind: SocketAddr,
    worker_event_rx: &mut Receiver<WorkerEvent>,
    onchain_client: &OnChainClient,
) {
    let app = Route::new()
        .at("/v1/chat/completions", poem::post(chat_completions).data(model_exe))
        .at("/v1/models", poem::get(list_models))
        .at("/v1/models/:model_id", poem::get(get_model))
        .with(Cors::new());

    tokio::spawn(async move { Server::new(TcpListener::bind(http_bind)).run(app).await });

    loop {
        tokio::select! {
            e = worker.recv() => match e {
                Some(e) => {},
                None => {
                    log::error!("[OpenAIServer] worker closed");
                    break;
                },
            },
            e = worker_event_rx.recv() => match e {
                Some(WorkerEvent::Start(success, chat_id, addresses)) => {
                    log::info!("[OpenAIServer] WorkerEvent::Start {addresses:?}");
                    // TODO: To handle max_tokens, the current token count is inaccurate so
                    // we are using a fixed value of 100 for now. Expiration is also fixed to 1000.
                    if !success {
                        log::error!("[OpenAIServer] WorkerEvent::Start failed");
                        continue;
                    }
                    let addresses = addresses.iter().map(|a| Address::from_str(a).unwrap()).collect();
                    let res = onchain_client.create_session(chat_id, 1000, 100, addresses).await;
                    if let Ok(res) = res {
                        log::info!("[OpenAIServer] create_session success: {res:?}");
                    } else {
                        log::error!("[OpenAIServer] create_session failed: {res:?}");
                    }

                },
                Some(WorkerEvent::End(success, chat_id, count, client_address)) => {
                    log::info!("[OpenAIServer] WorkerEvent::Stop token count: {count}, client_address: {client_address}");
                    if !success {
                        log::error!("[OpenAIServer] WorkerEvent::End failed");
                        continue;
                    }
                    let onchain_session_id = onchain_client.get_session_id(onchain_client.account.address().into(), chat_id).await;
                    if let Ok(onchain_session_id) = onchain_session_id {
                        let res = onchain_client.update_token_count(onchain_session_id, count.into()).await;
                        if let Ok(res) = res {
                            log::info!("[OpenAIServer] update_token_count success: {res:?}");
                        } else {
                            log::error!("[OpenAIServer] update_token_count failed: {res:?}");
                        }
                    } else {
                        log::error!("[OpenAIServer] get_session_id failed: {onchain_session_id:?}");
                    }
                },
                _ => {
                },
            },
            _ = signal::ctrl_c() => {
                worker.shutdown().await;
                break;
            },
        }
    }
}
