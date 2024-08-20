use clap::Parser;
use registry::server::RegistryServer;
use std::net::SocketAddr;

/// Registry server for decentralized LLM
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// address to bind websocket server
    #[arg(env, short, long, default_value = "0.0.0.0:3000")]
    ws_bind: SocketAddr,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    env_logger::builder().format_timestamp_millis().init();

    let mut registry = RegistryServer::new(args.ws_bind);
    while let Some(_) = registry.recv().await {}
}
