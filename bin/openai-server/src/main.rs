use clap::Parser;
use std::net::SocketAddr;
use worker::WorkerRunner;

/// OpenAI Server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// address to bind websocket server
    #[arg(env, long, default_value = "0.0.0.0:4000")]
    http_bind: SocketAddr,

    /// registry server
    #[arg(env, long, default_value = "ws://127.0.0.1:3000/ws")]
    registry_server: String,

    /// node id
    #[arg(env, long)]
    node_id: String,

    /// model id
    #[arg(env, long, default_value = "phi3")]
    model: String,

    /// model layers
    #[arg(env, long)]
    layers_from: u32,

    /// model layers
    #[arg(env, long)]
    layers_to: u32,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    env_logger::builder().format_timestamp_millis().init();

    let mut worker = WorkerRunner::new(
        &args.registry_server,
        &args.model,
        &args.node_id,
        args.layers_from,
        args.layers_to,
    )
    .await;
    while let Some(e) = worker.recv().await {}
}
