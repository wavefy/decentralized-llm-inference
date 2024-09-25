use std::{collections::VecDeque, ops::Range};

use p2p_network::{addr::NodeId, node::ConnId};
use protobuf_stream::ProtobufStream;
use protocol::registry::{
    relay_data,
    to_registry::{self, UpdateRequest},
    RelayData, ToRegistry, ToWorker,
};
use tokio_tungstenite::connect_async;

mod layers_selection;
mod protobuf_stream;

use crate::AnswerError;
pub use layers_selection::{get_layers_distribution, select_layers, LayerSelectionRes};

#[derive(Debug, PartialEq, Eq)]
pub enum RegistryClientEvent {
    Answer(NodeId, ConnId, String),
    Offer(NodeId, ConnId, String),
    Neighbours(Vec<NodeId>),
}

pub struct RegistryClient {
    stream: ProtobufStream,
    queue: VecDeque<ToRegistry>,
}

impl RegistryClient {
    pub async fn new(endpoint: &str, model: &str, node_id: NodeId) -> Self {
        let url = format!("{endpoint}/{model}/{}", node_id.0);
        log::info!("[RegistryClient] connecting to {url}");
        let (ws_stream, _) = connect_async(&url).await.expect("Should connect success");

        log::info!("[RegistryClient] connected as node {}", node_id.0);

        Self {
            stream: ProtobufStream::new(ws_stream),
            queue: VecDeque::new(),
        }
    }

    pub fn update_layer(&mut self, layers_range: Range<u32>) {
        self.queue.push_back(ToRegistry {
            event: Some(to_registry::Event::Update(UpdateRequest {
                from_layer: layers_range.start,
                to_layer: layers_range.end,
            })),
        });
    }

    pub fn find_neigbours(&mut self) {
        self.queue.push_back(ToRegistry {
            event: Some(to_registry::Event::Neighbours(to_registry::NeighboursRequest {})),
        });
    }

    pub fn offer(&mut self, dest: NodeId, conn_id: ConnId, offer: &str) {
        log::info!("[RegistryClient] offer to {dest:?}");
        self.queue.push_back(ToRegistry {
            event: Some(to_registry::Event::Relay(to_registry::Relay {
                dest: dest.0,
                data: Some(RelayData {
                    data: Some(relay_data::Data::Offer(relay_data::Offer {
                        conn_id: conn_id.0,
                        sdp: offer.to_owned(),
                    })),
                }),
            })),
        });
    }

    pub fn answer(&mut self, dest: NodeId, conn_id: ConnId, answer: Result<String, AnswerError>) {
        log::info!("[RegistryClient] answer to {dest:?}, {}", answer.as_ref().unwrap());
        self.queue.push_back(ToRegistry {
            event: Some(to_registry::Event::Relay(to_registry::Relay {
                dest: dest.0,
                data: Some(RelayData {
                    data: Some(relay_data::Data::Answer(relay_data::Answer {
                        conn_id: conn_id.0,
                        sdp: answer.unwrap(),
                    })),
                }),
            })),
        });
    }

    pub async fn shutdown(&mut self) {
        self.stream.shutdown().await;
    }

    pub async fn recv(&mut self) -> Option<Result<RegistryClientEvent, String>> {
        while let Some(req) = self.queue.pop_front() {
            self.stream.write(&req).await.ok()?;
        }

        loop {
            match self.stream.read::<ToWorker>().await? {
                Ok(event) => match event.event? {
                    protocol::registry::to_worker::Event::Update(res) => {
                        log::info!("[RegistryClient] will connect to {:?}", res.neighbours);
                        break Some(Ok(RegistryClientEvent::Neighbours(res.neighbours.into_iter().map(|n| NodeId(n)).collect::<Vec<_>>())));
                    }
                    protocol::registry::to_worker::Event::Neighbours(_) => todo!(),
                    protocol::registry::to_worker::Event::Relay(relay) => match relay.data {
                        Some(data) => match data.data {
                            Some(data) => match data {
                                relay_data::Data::Offer(offer) => break Some(Ok(RegistryClientEvent::Offer(NodeId(relay.source), ConnId(offer.conn_id), offer.sdp))),
                                relay_data::Data::Answer(answer) => break Some(Ok(RegistryClientEvent::Answer(NodeId(relay.source), ConnId(answer.conn_id), answer.sdp))),
                            },
                            None => {}
                        },
                        None => {}
                    },
                },
                Err(err) => break Some(Err(err.to_string())),
            }
        }
    }
}
