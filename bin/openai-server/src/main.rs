use std::{net::SocketAddr, sync::Arc};

use candle_core::quantized::gguf_file;
use clap::Parser;
use models::{
    get_device,
    phi3::{self, Phi3LayersWorker, Phi3Model},
};
use protocol::{ModelLayersRanger, Session};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    signal,
    sync::mpsc::channel,
};
use worker::{VirtualModelLayers, WorkerRunner};

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
    let mut model_file = std::fs::File::open(phi3::model_path().await).unwrap();
    let model = gguf_file::Content::read(&mut model_file).unwrap();
    let layers_worker = Phi3LayersWorker::new(false, ModelLayersRanger::new(args.layers_from, args.layers_to), &model, &mut model_file, &device).unwrap();

    let (mut worker, virtual_model_layers) = WorkerRunner::new(&args.registry_server, device.clone(), layers_worker, &args.model, &args.node_id, args.layers_from, args.layers_to, 32).await;

    let tcp_listener = TcpListener::bind(args.http_bind).await.expect("Should open tcp port");
    let phi3 = Arc::new(Phi3Model::new(device, virtual_model_layers).await);

    loop {
        tokio::select! {
            e = tcp_listener.accept() => match e {
                Ok((stream, remote)) => {
                    spawn_session(stream, remote, phi3.clone());
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

fn spawn_session(mut stream: TcpStream, remote: SocketAddr, phi3: Arc<Phi3Model<VirtualModelLayers>>) {
    let session = Session::new();
    tokio::spawn(async move {
        log::info!("[OpenAIServer] session {session:?} connected with remote {remote:?}");
        let mut buf = [0; 4096];
        if let Ok(len) = stream.read(&mut buf).await {
            let prompt = String::from_utf8_lossy(&buf[0..len]).to_string();
            let (tx, mut rx) = channel(1);
            tokio::spawn(async move { phi3.chat(session, 0, 1024, &prompt, tx).await });

            while let Some(out) = rx.recv().await {
                stream.write_all(out.as_bytes()).await.unwrap();
            }
        }
        log::info!("[OpenAIServer] end session {session:?} with remote {remote:?}");
    });
}
