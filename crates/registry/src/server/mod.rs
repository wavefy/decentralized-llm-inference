use network::addr::NodeId;
use poem::{
    get, handler,
    listener::TcpListener,
    web::{websocket::WebSocket, Data, Path},
    EndpointExt, IntoResponse, Route, Server,
};
use protobuf_stream::ProtobufStream;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod protobuf_stream;
mod session_manager;

use session_manager::SessionManager;

use crate::ModelId;

enum StreamEvent {
    Start(ModelId, NodeId),
    Req(
        ModelId,
        NodeId,
        protocol::registry::request::Req,
        tokio::sync::oneshot::Sender<protocol::registry::response::Res>,
    ),
    End(ModelId, NodeId),
}

pub struct RegistryServer {
    models: HashMap<ModelId, SessionManager>,
    stream_rx: Receiver<StreamEvent>,
}

impl RegistryServer {
    pub fn new(http_addr: SocketAddr) -> Self {
        let (stream_tx, stream_rx) = channel(10);
        tokio::spawn(async move {
            log::info!("[RegistryServer] listen on ws://{http_addr}");
            let app = Route::new().at("/ws/:model/:node", get(ws.data(stream_tx)));

            Server::new(TcpListener::bind(http_addr)).run(app).await
        });

        Self {
            models: Default::default(),
            stream_rx,
        }
    }

    pub async fn recv(&mut self) -> Option<()> {
        let event = self.stream_rx.recv().await?;
        match event {
            StreamEvent::Start(model, node) => {
                let entry = self.models.entry(model).or_default();
                entry.on_start(node);
            }
            StreamEvent::Req(model, node, req, tx) => {
                let entry = self.models.entry(model).or_default();
                let res = entry.on_req(node, req);
                tx.send(res).expect("Should send to main task");
            }
            StreamEvent::End(model, node) => {
                let entry = self.models.entry(model).or_default();
                entry.on_end(node);
            }
        }
        Some(())
    }
}

#[handler]
fn ws(
    Path((model, node)): Path<(String, String)>,
    ws: WebSocket,
    stream_tx: Data<&Sender<StreamEvent>>,
) -> impl IntoResponse {
    // TODO auth or
    let sender = stream_tx.clone();
    ws.on_upgrade(move |stream| async move {
        log::info!("[WebsocketServer] on connected from {node} with model {model}");
        let model_id = ModelId(model.clone());
        let node_id = NodeId(node.clone());
        let mut protobuf_stream = ProtobufStream::new(stream);
        sender
            .send(StreamEvent::Start(model_id.clone(), node_id.clone()))
            .await
            .expect("Should send event main");

        while let Some(Ok(req)) = protobuf_stream.read::<protocol::registry::Request>().await {
            if let Some(req_inner) = req.req {
                let (tx, mut rx) = tokio::sync::oneshot::channel();
                sender
                    .send(StreamEvent::Req(
                        model_id.clone(),
                        node_id.clone(),
                        req_inner,
                        tx,
                    ))
                    .await
                    .expect("Should send req to main");
                if let Ok(res) = rx.await {
                    if let Err(e) = protobuf_stream
                        .write(&protocol::registry::Response {
                            req_id: req.req_id,
                            res: Some(res),
                        })
                        .await
                    {
                        log::error!("[WebsocketStream] write response error {e:?}");
                    }
                }
            } else {
                log::warn!("[WebsocketStream] request without body");
                if let Err(e) = protobuf_stream
                    .write(&protocol::registry::Response {
                        req_id: req.req_id,
                        res: None,
                    })
                    .await
                {
                    log::error!("[WebsocketStream] write response error {e:?}");
                }
            };
        }

        sender
            .send(StreamEvent::End(model_id, node_id))
            .await
            .expect("Should send event main");
        log::info!("[WebsocketServer] on disconnected from {node} with model {model}");
    })
}
