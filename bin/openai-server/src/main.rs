use std::{net::SocketAddr, sync::Arc};

use candle_core::{DType, Device};
use clap::Parser;
use models::{get_device, llama, phi3, ChatCfg, ChatModel};
use protocol::{ModelLayersRanger, Session};
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
    #[arg(env, long, default_value = "127.0.0.1:5555")]
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
            let (mut worker, virtual_model_layers) = WorkerRunner::<32>::new(&args.registry_server, device.clone(), layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to).await;
            let model = phi3::Phi3Model::new(device.clone(), virtual_model_layers).await;
            run(&mut worker, device, model.into(), &args.model, &args.node_id, args.layers_from, args.layers_to, 32, args.http_bind).await;
        }
        "llama" => {
            let layers_worker = llama::new_layers(DType::F16, device.clone(), false, ModelLayersRanger::new(args.layers_from, args.layers_to)).await;
            let (mut worker, virtual_model_layers) = WorkerRunner::<16>::new(&args.registry_server, device.clone(), layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to).await;
            let model = llama::LlamaModel::new(device.clone(), DType::F16, virtual_model_layers, false).await;
            run(&mut worker, device, model.into(), &args.model, &args.node_id, args.layers_from, args.layers_to, 16, args.http_bind).await;
        }
        _ => panic!("unsupported"),
    }
}

async fn run<CM: ChatModel + Send + Sync + 'static, const MODEL_LAYERS: usize>(
    worker: &mut WorkerRunner<MODEL_LAYERS>,
    device: Device,
    model_exe: Arc<CM>,
    model: &str,
    node_id: &str,
    from: u32,
    to: u32,
    total: u32,
    http_bind: SocketAddr,
) {
    let tcp_listener = TcpListener::bind(http_bind).await.expect("Should open tcp port");

    loop {
        tokio::select! {
            e = tcp_listener.accept() => match e {
                Ok((stream, remote)) => {
                    spawn_session(stream, remote, model_exe.clone());
                },
                Err(err) => {
                    log::error!("[OpenAIServer] tcp listener error {err:?}");
                    break;
                }
            },
            e = worker.recv() => match e {
                Some(e) => {},
                None => {
                    log::error!("[OpenAIServer] worker closed");
                    break;
                },
            },
            _ = signal::ctrl_c() => {
                worker.shutdown().await;
                break;
            },
        }
    }
}

fn spawn_session<CM: ChatModel + Send + Sync + 'static>(mut stream: TcpStream, remote: SocketAddr, model_exe: Arc<CM>) {
    let session = Session::new();
    tokio::spawn(async move {
        log::info!("[OpenAIServer] session {session:?} connected with remote {remote:?}");
        let mut buf = [0; 4096];
        if let Ok(len) = stream.read(&mut buf).await {
            let prompt = String::from_utf8_lossy(&buf[0..len]).trim().to_string();
            let (tx, mut rx) = channel(1);
            tokio::spawn(async move { model_exe.chat(session, ChatCfg::default(), &prompt, tx).await });

            while let Some(out) = rx.recv().await {
                stream.write_all(out.as_bytes()).await.unwrap();
            }
        }
        log::info!("[OpenAIServer] end session {session:?} with remote {remote:?}");
    });
}
