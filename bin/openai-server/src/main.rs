use std::net::SocketAddr;

use clap::Parser;
use openai_server::{start_control_server, start_server};
use utils::random_node_id;

/// OpenAI Server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// status bind addr
    #[arg(env, long, default_value = "127.0.0.1:18889")]
    control_bind: SocketAddr,

    /// http bind addr
    #[arg(env, long, default_value = "127.0.0.1:18888")]
    http_bind: SocketAddr,

    /// stun server
    #[arg(env, long, default_value = "stun.l.google.com:19302")]
    stun_server: String,

    /// registry server
    #[arg(env, long, default_value = "ws://127.0.0.1:3000/ws")]
    registry_server: String,

    /// node id
    #[arg(env, long)]
    node_id: Option<String>,

    /// model id
    #[arg(env, long)]
    model: Option<String>,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_from: Option<u32>,

    /// model layers, layer 0 is embeding work, from 1 is for matrix jobs
    #[arg(env, long)]
    layers_to: Option<u32>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    use std::env;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info,str0m=warn");
    }

    let node_id = args.node_id.unwrap_or_else(random_node_id);

    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    if let Some(model) = args.model {
        let layers_from = args.layers_from.unwrap_or(0);
        let layers_to = args.layers_to.unwrap_or(0);
        start_server(&args.registry_server, &model, &node_id, layers_from..layers_to, args.http_bind, &args.stun_server).await;
    } else {
        start_control_server(args.control_bind, &args.registry_server, &node_id, args.http_bind, &args.stun_server).await;
    }
}
