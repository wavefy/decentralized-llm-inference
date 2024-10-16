use candle_core::{DType, Device, Tensor};
use clap::Parser;
use contract::{
    aptos_sdk::{rest_client::AptosBaseUrl, types::LocalAccount},
    OnChainService,
};
use models::{fake, get_device, llama, phi3, ModelLayersWorker};
use utils::random_node_id;
use std::{net::ToSocketAddrs, sync::Arc};
use tokio::signal;
use usage_service::WorkerUsageService;
use worker::WorkerRunner;

/// OpenAI Server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// registry server
    #[arg(env, long, default_value = "ws://127.0.0.1:3000/ws")]
    registry_server: String,

    /// stun server
    #[arg(env, long, default_value = "stun.l.google.com:19302")]
    stun_server: String,

    /// node id
    #[arg(env, long)]
    node_id: Option<String>,

    /// model id
    #[arg(env, long, default_value = "phi3")]
    model: String,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_from: u32,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_to: u32,

    /// Wallet private key
    #[arg(env, long, default_value = "0x69d91353993001d80ef74f7a27fcb15456d4d6298c755a5316a0a0d87b6b39b9")]
    private_key: String,

    /// Contract address
    #[arg(env, long, default_value = "0x9123e2561d81ba5f77473b8dc664fa75179c841061d12264508894610b9d0b7a")]
    contract_address: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    use std::env;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info,str0m=warn");
    }

    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let device = get_device(false).unwrap();

    let account = LocalAccount::from_private_key(&args.private_key, 0).expect("Invalid private key");
    let onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet, args.layers_from..args.layers_to);
    onchain_service.init().await;
    let usage_service = Arc::new(onchain_service);
    let node_id = args.node_id.unwrap_or_else(random_node_id);

    match args.model.as_str() {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, args.layers_from..args.layers_to, &device).await.unwrap();
            run::<_, 32>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &node_id,
                args.layers_from,
                args.layers_to,
                &args.stun_server,
                usage_service,
            )
            .await;
        }
        "llama32-1b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-1B-Instruct".to_string(),
                model: "model.safetensors".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, args.layers_from..args.layers_to).await;
            run::<_, 16>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &node_id,
                args.layers_from,
                args.layers_to,
                &args.stun_server,
                usage_service,
            )
            .await;
        }
        "llama32-3b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-3B-Instruct".to_string(),
                model: "model.safetensors".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, args.layers_from..args.layers_to).await;
            run::<_, 28>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &node_id,
                args.layers_from,
                args.layers_to,
                &args.stun_server,
                usage_service,
            )
            .await;
        }
        "llama32-vision-11b" => {
            let resource = llama::ModelResource {
                repo: "unsloth/Llama-3.2-11B-Vision-Instruct".to_string(),
                model: "model.safetensors.index.json".to_string(),
                config: "config.json".to_string(),
                tokenizer: "tokenizer.json".to_string(),
            };
            let layers_worker = llama::new_layers(&resource, DType::F16, device.clone(), false, args.layers_from..args.layers_to).await;
            run::<_, 40>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &node_id,
                args.layers_from,
                args.layers_to,
                &args.stun_server,
                usage_service,
            )
            .await;
        }
        "fake" => {
            let layers_worker = fake::FakeLayersWorker::new(args.layers_from..args.layers_to);
            run::<_, 16>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &node_id,
                args.layers_from,
                args.layers_to,
                &args.stun_server,
                usage_service,
            )
            .await;
        }
        _ => panic!("unsupported"),
    }
}

async fn run<LW: ModelLayersWorker<(Tensor, u32)> + Send + Sync + 'static, const MODEL_LAYERS: usize>(
    registry_server: &str,
    device: Device,
    layers_worker: LW,
    model: &str,
    node_id: &str,
    from: u32,
    to: u32,
    stun_server: &str,
    usage_service: Arc<dyn WorkerUsageService>,
) {
    let stun_servers = stun_server.to_socket_addrs().unwrap().collect();
    let (mut worker, _virtual_layers) = WorkerRunner::<MODEL_LAYERS>::new(registry_server, model, node_id, from..to, layers_worker, device, stun_servers, usage_service).await;

    loop {
        tokio::select! {
            e = worker.recv() => match e {
                Some(e) => {},
                None => break,
            },
            _ = signal::ctrl_c() => {
                worker.shutdown().await;
                break;
            },
        }
    }
}
