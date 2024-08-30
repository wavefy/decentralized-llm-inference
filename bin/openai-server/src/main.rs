use std::{collections::HashMap, net::SocketAddr};

use clap::Parser;
use protocol::Session;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    signal,
    sync::mpsc::{channel, Sender},
};
use worker::{SessionReq, SessionRes, WorkerRunner};

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

    let tcp_listener = TcpListener::bind(args.http_bind).await.expect("Should open tcp port");
    let (control_tx, mut control_rx) = channel(10);
    let mut worker = WorkerRunner::new(&args.registry_server, &args.model, &args.node_id, args.layers_from, args.layers_to, 32).await;
    let mut sessions = HashMap::new();

    loop {
        tokio::select! {
            e = tcp_listener.accept() => match e {
                Ok((stream, remote)) => {
                    let (session, tx) = spawn_session(stream, remote, control_tx.clone());
                    sessions.insert(session, tx);
                },
                Err(err) => {
                    log::error!("[OpenAIServer] tcp listener error {err:?}");
                    break;
                }
            },
            e = control_rx.recv() => match e {
                Some((session, Some(req))) => {
                    worker.session_req(session, req);
                },
                Some((session, None)) => {
                    sessions.remove(&session);
                },
                None => {
                    log::error!("[OpenAIServer] control_rx closed");
                    break;
                },
            },
            e = worker.recv() => match e {
                Some(e) => match e {
                    worker::WorkerRunnerEvent::Session(session, res) => {
                        if let Some(tx) = sessions.get(&session) {
                            tx.send(res).await.unwrap();
                        }
                    },
                },
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

fn spawn_session(mut stream: TcpStream, remote: SocketAddr, control_tx: Sender<(Session, Option<SessionReq>)>) -> (Session, Sender<SessionRes>) {
    let session = Session::new();
    let (tx, mut rx) = channel(10);
    tokio::spawn(async move {
        log::info!("[OpenAIServer] session {session:?} connected with remote {remote:?}");
        control_tx.send((session, Some(SessionReq::Start))).await.unwrap();
        stream.write_all("Connecting\n".as_bytes()).await.unwrap();

        let mut buf = [0; 4096];
        let mut started = false;
        loop {
            tokio::select! {
                e = rx.recv() => match e {
                    Some(res) => match res {
                        SessionRes::Started(next) => {
                            log::info!("[OpenAiServer] session started {session:?}");
                            started = true;
                            stream.write_all("Connected\n".as_bytes()).await.unwrap();
                        }
                        SessionRes::Backward(step, payload) => {
                            if payload.is_empty() {
                                control_tx.send((session, Some(SessionReq::Stop))).await.unwrap();
                            } else {
                                stream.write_all(&payload).await.unwrap();
                                control_tx.send((session, Some(SessionReq::Forward(step + 1, payload)))).await.unwrap();
                            }
                        }
                        SessionRes::Stopped(next) => {
                            log::info!("[OpenAiServer] session stopped {session:?}");
                            break;
                        },
                    },
                    None => break,
                },
                e = stream.read(&mut buf) => match e {
                    Ok(0) => {
                        break;
                    },
                    Ok(len) => {
                        log::info!("[OpenAiServer] session received {len} bytes from client");
                        if !started {
                            stream.write_all("Connecting...\n".as_bytes()).await.unwrap();
                        } else {
                            control_tx.send((session, Some(SessionReq::Forward(0, buf[..len].to_vec())))).await.unwrap();
                        }
                    },
                    Err(e) => break,
                }
            }
        }
        log::info!("[OpenAIServer] end session {session:?} with remote {remote:?}");
        control_tx.send((session, None)).await.unwrap();
    });
    (session, tx)
}
