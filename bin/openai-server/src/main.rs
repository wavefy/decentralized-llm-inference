use std::{net::SocketAddr, sync::Arc};

use clap::Parser;
use contract::{
    aptos_sdk::{
        rest_client::{AptosBaseUrl, Client},
        types::LocalAccount,
    },
    client::OnChainClient,
};
use models::{get_device, llama, phi3, ChatCfg, ChatModel};
use openai_server::start_server;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    signal,
    sync::mpsc::channel,
};
use worker::WorkerRunner;

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

    /// Private key
    #[arg(env, long, default_value = "0x3bba41ade33b801bf3e42a080a699e73654eaf1775fb0afc5d65f5449e55d74b")]
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
    start_server(
        &args.registry_server,
        &args.model,
        &args.node_id,
        args.layers_from..args.layers_to,
        args.http_bind,
        args.stun_server,
        &args.private_key,
        &args.contract_address,
    )
    .await;
}
