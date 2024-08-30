use candle_core::quantized::gguf_file;
use clap::Parser;
use models::{
    get_device,
    phi3::{self, Phi3LayersWorker},
};
use protocol::{ModelLayersRanger, Session};
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
    let mut model_file = std::fs::File::open(phi3::model_path().await).unwrap();
    let model = gguf_file::Content::read(&mut model_file).unwrap();
    let layers_worker = Phi3LayersWorker::new(false, ModelLayersRanger::new(args.layers_from, args.layers_to), &model, &mut model_file, &device).unwrap();

    let (mut worker, _virtual_model_layer) = WorkerRunner::new(&args.registry_server, device, layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to, 32).await;
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
