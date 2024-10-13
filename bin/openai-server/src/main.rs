use clap::Parser;
use openai_server::{start_http_server, ServerMode};
use std::net::SocketAddr;
use utils::random_node_id;

/// OpenAI Server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
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

    #[command(subcommand)]
    mode: ServerMode,
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
    start_http_server(args.http_bind, &args.registry_server, &node_id, &args.stun_server, args.mode).await;
}
