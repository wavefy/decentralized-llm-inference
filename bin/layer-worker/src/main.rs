use std::str::FromStr;

use candle_core::{DType, Device, Tensor};
use clap::Parser;
use contract::{
    aptos_sdk::{
        rest_client::{aptos_api_types::Address, AptosBaseUrl},
        types::LocalAccount,
    },
    OnChainService,
};
use models::{fake, get_device, llama, phi3, ModelLayersWorker};
use tokio::signal;
use worker::{WorkerEvent, WorkerEventWithResp, WorkerRunner};

/// OpenAI Server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// registry server
    #[arg(env, long, default_value = "ws://127.0.0.1:3000/ws")]
    registry_server: String,

    /// node id
    #[arg(env, long)]
    node_id: String,

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
    let account_address = account.address().to_string();
    let mut onchain_service = OnChainService::new(account, AptosBaseUrl::Testnet, &args.contract_address);
    onchain_service.init().await;

    match args.model.as_str() {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, args.layers_from..args.layers_to, &device).await.unwrap();
            run::<_, 32>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &args.node_id,
                args.layers_from,
                args.layers_to,
                &account_address,
                onchain_service,
            )
            .await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, args.layers_from..args.layers_to).await;
            run::<_, 16>(
                &args.registry_server,
                device,
                layers_worker,
                &args.model,
                &args.node_id,
                args.layers_from,
                args.layers_to,
                &account_address,
                onchain_service,
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
                &args.node_id,
                args.layers_from,
                args.layers_to,
                &account_address,
                onchain_service,
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
    address: &str,
    mut onchain_service: OnChainService,
) {
    let (mut worker, _virtual_layers, mut worker_event_rx) = WorkerRunner::<MODEL_LAYERS>::new(registry_server, model, node_id, from..to, layers_worker, device, address).await;

    tokio::spawn(async move {
        loop {
            match worker_event_rx.recv().await {
                Some(WorkerEventWithResp { event, resp }) => {
                    log::info!("[LayerWorker] WorkerEventWithResp {:?}", resp);
                    match event {
                        WorkerEvent::Start(chat_id, addresses) => {
                            log::info!("[LayerWorker] WorkerEvent::Start {:?}", addresses);
                            if let Some(resp) = resp {
                                resp.send(true);
                            }
                        }
                        WorkerEvent::Forward(chat_id) => {
                            onchain_service.increment_chat_token_count(chat_id, 1);
                        }
                        WorkerEvent::End(chat_id, client_address) => {
                            log::info!("[LayerWorker] WorkerEvent::End {:?}", client_address);
                            let res = onchain_service.claim_tokens(chat_id, Address::from_str(&client_address).unwrap()).await;
                            log::info!("[LayerWorker] claim_tokens {:?}", res);
                            if let Some(resp) = resp {
                                resp.send(res.is_ok());
                            }
                        }
                    };
                }
                None => break,
            }
        }
    });

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
