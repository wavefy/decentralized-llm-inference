use std::{net::ToSocketAddrs, ops::Range, sync::Arc};

use candle_core::DType;

use models::{fake, get_device, llama, phi3, ChatModel};
use openai_http::ModelStore;

use protocol::Model;
use tokio::sync::{mpsc::Receiver, oneshot};
use usage_service::WorkerUsageService;
use worker::WorkerRunner;

pub async fn run_model_worker(
    registry_server: &str,
    model: &str,
    node_id: &str,
    layers: Range<u32>,
    stun_server: &str,
    control_rx: Receiver<WorkerControl>,
    usage_service: Arc<dyn WorkerUsageService>,
    store: ModelStore,
) {
    let stun_servers = stun_server.to_socket_addrs().unwrap().collect::<Vec<_>>();
    log::info!("[OpenAIServer] start with model {} and stun server {}", model, stun_server);
    let device = get_device(false).unwrap();

    match model {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, layers.clone(), &device).await.unwrap();
            let (mut worker, virtual_model_layers) = WorkerRunner::<32>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            let model = Model {
                id: "phi3".to_owned(),
                object: "model".to_owned(),
                created: 0,
                owned_by: "Microsoft".to_owned(),
            };
            run_model_worker_internal(&mut worker, model, Arc::new(model_exe), store, control_rx).await;
        }
        "llama32-1b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-1B-Instruct".to_string(),
                model: "model.safetensors".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = llama::LlamaModel::new(&resource, device.clone(), DType::F16, virtual_model_layers, false).await;
            let model = Model {
                id: "llama32-1b".to_owned(),
                object: "model".to_owned(),
                created: 0,
                owned_by: "unsloth".to_owned(),
            };
            run_model_worker_internal(&mut worker, model, Arc::new(model_exe), store, control_rx).await;
        }
        "llama32-3b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-3B-Instruct".to_string(),
                model: "model.safetensors".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<28>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = llama::LlamaModel::new(&resource, device.clone(), DType::F16, virtual_model_layers, false).await;
            let model = Model {
                id: "llama32-3b".to_owned(),
                object: "model".to_owned(),
                created: 0,
                owned_by: "unsloth".to_owned(),
            };
            run_model_worker_internal(&mut worker, model, Arc::new(model_exe), store, control_rx).await;
        }
        "llama32-vision-11b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-11B-Vision-Instruct".to_string(),
                model: "model.safetensors.index.json".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, layers.clone()).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<40>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = llama::LlamaModel::new(&resource, device.clone(), DType::F16, virtual_model_layers, false).await;
            let model = Model {
                id: "llama32-11b".to_owned(),
                object: "model".to_owned(),
                created: 0,
                owned_by: "unsloth".to_owned(),
            };
            run_model_worker_internal(&mut worker, model, Arc::new(model_exe), store, control_rx).await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(layers.clone());
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(registry_server, model, node_id, layers.clone(), layers_worker, device.clone(), stun_servers, usage_service).await;
            let model_exe = fake::FakeModel::new(device.clone(), virtual_model_layers).await;
            let model = Model {
                id: "fake".to_owned(),
                object: "model".to_owned(),
                created: 0,
                owned_by: "fake".to_owned(),
            };
            run_model_worker_internal(&mut worker, model, Arc::new(model_exe), store, control_rx).await;
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

async fn run_model_worker_internal<const MODEL_LAYERS: usize>(
    worker: &mut WorkerRunner<MODEL_LAYERS>,
    model: Model,
    model_exe: Arc<dyn ChatModel>,
    store: ModelStore,
    mut control_rx: Receiver<WorkerControl>,
) {
    let mut chat_rx = store.add_model(model.clone());
    loop {
        tokio::select! {
            e = worker.recv() => match e {
                Some(_e) => {},
                None => {
                    log::error!("[OpenAIServer] worker closed");
                    break;
                },
            },
            e = control_rx.recv() => match e {
                Some(WorkerControl::Status(sender)) => {
                    sender.send(WorkerStatus {
                        ready: worker.ready(),
                        peers: worker.peers().iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        sessions: worker.sessions(),
                    }).unwrap();
                }
                Some(WorkerControl::Stop(sender)) => {
                    log::info!("[OpenAIServer] p2p_stop: sending stop ack signal");
                    sender.send(()).unwrap();
                    log::info!("[OpenAIServer] p2p_stop: stop ack signal received");
                    break;
                }
                None => {
                    log::error!("[OpenAIServer] query rx closed");
                    break;
                }
            },
            c = chat_rx.recv() => match c {
                Some(req) => {
                    let prompt = model_exe.build_prompt(&req.req);
                    let model_exe = model_exe.clone();
                    tokio::spawn(async move {
                        if let Err(e) = model_exe.chat(req.session, req.cfg, &prompt, req.answer_tx).await {
                            log::error!("[OpenAIServer] run session error {e:?}");
                        }
                    });
                },
                None => {
                    log::error!("[OpenAIServer] chat_rx closed");
                    break;
                },
            }
        }
    }
    store.remove_model(&model.id);
}
