use candle_core::{DType, Device, Tensor};
use clap::Parser;
use models::{get_device, llama, phi3, ModelLayersWorker};
use protocol::ModelLayersRanger;
use tokio::signal;
use worker::WorkerRunner;

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
    match args.model.as_str() {
        "phi3" => {
            let layers_worker = phi3::Phi3LayersWorker::new(false, ModelLayersRanger::new(args.layers_from, args.layers_to), &device).await.unwrap();
            run::<_, 32>(&args.registry_server, device, layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, ModelLayersRanger::new(args.layers_from, args.layers_to)).await;
            run::<_, 16>(&args.registry_server, device, layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to).await;
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
) {
    let (mut worker, _virtual_model_layer) = WorkerRunner::<MODEL_LAYERS>::new(registry_server, device, layers_worker, model, node_id, from, to).await;
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
