use p2p_network::addr::NodeId;
use poem::{
    get, handler,
    listener::TcpListener,
    web::{websocket::WebSocket, Data, Json, Path},
    EndpointExt, IntoResponse, Route, Server,
};
use protobuf_stream::ProtobufStream;
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod protobuf_stream;
mod session_manager;

use session_manager::{NodeInfo, SessionManager};

use crate::{get_model_info, ModelDistribution, ModelId, ModelInfo, SUPPORTED_MODELS};

enum StreamEvent {
    Start(ModelId, NodeId, Sender<protocol::registry::to_worker::Event>),
    Event(ModelId, NodeId, protocol::registry::to_registry::Event),
    End(ModelId, NodeId),
    Distribution(ModelId, Sender<ModelDistribution>),
    Health(Sender<Vec<ModelSwarmHealth>>),
}

pub struct RegistryServer {
    models: HashMap<ModelId, SessionManager>,
    stream_rx: Receiver<StreamEvent>,
    streams_tx: HashMap<(ModelId, NodeId), Sender<protocol::registry::to_worker::Event>>,
}

#[derive(Serialize)]
struct SwarmNode {
    id: String,
    info: NodeInfo,
}

#[derive(Serialize)]
struct ModelSwarmHealth {
    model: String,
    total_layers: usize,
    memory: usize,
    nodes: Vec<SwarmNode>,
}

impl RegistryServer {
    pub fn new(http_addr: SocketAddr) -> Self {
        let (stream_tx, stream_rx) = channel(10);
        tokio::spawn(async move {
            log::info!("[RegistryServer] listen on ws://{http_addr}");
            let app = Route::new()
                .at("/api/:model/distribution", get(distribution.data(stream_tx.clone())))
                .at("/ws/:model/:node", get(ws.data(stream_tx.clone())))
                .at("/api/health", get(health.data(stream_tx)))
                .at("/api/models", get(list_models));

            Server::new(TcpListener::bind(http_addr)).run(app).await
        });

        Self {
            models: Default::default(),
            stream_rx,
            streams_tx: HashMap::new(),
        }
    }

    pub async fn send(&mut self, model: ModelId, node: NodeId, data: protocol::registry::to_worker::Event) {
        if let Some(tx) = self.streams_tx.get(&(model, node)) {
            if let Err(e) = tx.send(data).await {
                log::error!("[RegistryServer] send event to stream error {e:?}");
            }
        }
    }

    pub async fn recv(&mut self) -> Option<()> {
        let event = self.stream_rx.recv().await?;
        match event {
            StreamEvent::Start(model, node, tx) => {
                self.streams_tx.insert((model.clone(), node.clone()), tx);
                let entry = self.models.entry(model).or_default();
                entry.on_start(node);
            }
            StreamEvent::Event(model, node, event) => {
                let entry = self.models.entry(model.clone()).or_default();
                entry.on_event(node, event);
                while let Some((dest, out)) = entry.pop_out() {
                    if let Some(tx) = self.streams_tx.get(&(model.clone(), dest)) {
                        if let Err(e) = tx.send(out).await {
                            log::error!("[RegistryServer] send event to stream error {e:?}");
                        }
                    }
                }
            }
            StreamEvent::End(model, node) => {
                self.streams_tx.remove(&(model.clone(), node.clone()));
                let entry = self.models.entry(model).or_default();
                entry.on_end(node);
            }
            StreamEvent::Distribution(model, tx) => {
                let res: ModelDistribution = self.models.get(&model).map(|m| m.get_distribution()).unwrap_or_default();
                if let Err(e) = tx.send(res).await {
                    log::error!("[RegistryServer] send distribution to stream error {e:?}");
                }
            }
            StreamEvent::Health(tx) => {
                let res: Vec<ModelSwarmHealth> = self
                    .models
                    .iter()
                    .map(|(model, session)| {
                        let nodes: Vec<SwarmNode> = session
                            .nodes()
                            .iter()
                            .map(|(node, info)| SwarmNode {
                                id: node.0.clone(),
                                info: info.clone(),
                            })
                            .collect();
                        let model_info = get_model_info(&model.0);
                        if model_info.is_none() {
                            log::warn!("[RegistryServer] Unexpected model, Mode {} not found", &model.0);
                        }
                        match model_info {
                            Some(ModelInfo { id: _, layers, memory }) => ModelSwarmHealth {
                                model: model.0.clone(),
                                total_layers: *layers,
                                memory: *memory,
                                nodes,
                            },
                            _ => ModelSwarmHealth {
                                model: model.0.clone(),
                                total_layers: 0,
                                memory: 0,
                                nodes,
                            },
                        }
                    })
                    .collect();
                if let Err(e) = tx.send(res).await {
                    log::error!("[RegistryServer] send health to stream error {e:?}");
                }
            }
        }
        Some(())
    }
}

#[handler]
async fn distribution(Path(model): Path<String>, stream_tx: Data<&Sender<StreamEvent>>) -> impl IntoResponse {
    let model_id = ModelId(model.clone());
    let (tx, mut rx) = channel(10);
    stream_tx.send(StreamEvent::Distribution(model_id.clone(), tx)).await.expect("Should send event main");
    let res = rx.recv().await.unwrap();
    Json(res).into_response()
}

#[handler]
async fn health(stream_tx: Data<&Sender<StreamEvent>>) -> impl IntoResponse {
    let (tx, mut rx) = channel(10);
    stream_tx.send(StreamEvent::Health(tx)).await.expect("Should send event health");
    let res = rx.recv().await.unwrap();
    Json(res).into_response()
}

#[handler]
async fn list_models() -> impl IntoResponse {
    Json(SUPPORTED_MODELS).into_response()
}

#[handler]
fn ws(Path((model, node)): Path<(String, String)>, ws: WebSocket, stream_tx: Data<&Sender<StreamEvent>>) -> impl IntoResponse {
    // TODO auth or
    let sender = stream_tx.clone();
    ws.on_upgrade(move |stream| async move {
        log::info!("[WebsocketServer] on connected from {node} with model {model}");
        let model_id = ModelId(model.clone());
        let node_id = NodeId(node.clone());
        let mut protobuf_stream = ProtobufStream::new(stream);
        let (tx, mut rx) = channel(10);
        sender.send(StreamEvent::Start(model_id.clone(), node_id.clone(), tx)).await.expect("Should send event main");

        loop {
            tokio::select! {
                msg = protobuf_stream.read::<protocol::registry::ToRegistry>() => {
                    if let Some(Ok(msg)) = msg {
                        if let Some(event) = msg.event {
                            sender.send(StreamEvent::Event(model_id.clone(), node_id.clone(), event)).await.expect("Should send req to main");
                        } else {
                            log::warn!("[WebsocketStream] request without body");
                        };
                    } else {
                        break;
                    }
                },
                out = rx.recv() => {
                    if let Some(out) = out {
                        if let Err(e) = protobuf_stream.write(&protocol::registry::ToWorker { event: Some(out) }).await {
                            log::error!("[WebsocketStream] write response error {e:?}");
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        sender.send(StreamEvent::End(model_id, node_id)).await.expect("Should send event main");
        log::info!("[WebsocketServer] on disconnected from {node} with model {model}");
    })
}
