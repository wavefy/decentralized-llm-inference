use std::collections::VecDeque;

use network::addr::NodeId;
use protobuf_stream::ProtobufStream;
use protocol::{
    registry::{
        request::{self, UpdateRequest},
        Request, Response,
    },
    ModelLayersRanger,
};
use tokio_tungstenite::connect_async;

mod protobuf_stream;

use crate::{AnswerError, NeighbourInfo, OfferError, ReqId};

#[derive(Debug, PartialEq, Eq)]
pub enum RegistryClientEvent {
    Updated(ReqId),
    Answer(ReqId, Result<String, OfferError>),
    Offer(NodeId, ReqId, String),
    Neighbours(Vec<NeighbourInfo>),
}

pub struct RegistryClient {
    stream: ProtobufStream,
    req_id_seed: u64,
    queue: VecDeque<Request>,
}

impl RegistryClient {
    pub async fn new(endpoint: &str, model: &str, node_id: NodeId) -> Self {
        let url = format!("{endpoint}/{model}/{}", node_id.0);
        log::info!("[RegistryClient] connecting to {url}");
        let (ws_stream, _) = connect_async(&url).await.expect("Should connect success");

        log::info!("[RegistryClient] connected as node {}", node_id.0);

        Self {
            stream: ProtobufStream::new(ws_stream),
            req_id_seed: 0,
            queue: VecDeque::new(),
        }
    }

    pub fn update_layer(&mut self, layers_range: ModelLayersRanger) -> ReqId {
        let req_id = self.gen_req_id();
        self.queue.push_back(Request {
            req_id: req_id.0,
            req: Some(request::Req::Update(UpdateRequest {
                from_layer: layers_range.from,
                to_layer: layers_range.to,
            })),
        });
        req_id
    }

    pub fn find_neigbours(&mut self) -> ReqId {
        todo!()
    }

    pub fn offer(&mut self, dest: NodeId, offer: &str) -> ReqId {
        todo!()
    }

    pub fn answer(&mut self, dest: NodeId, req_id: ReqId, answer: Result<String, AnswerError>) {
        todo!()
    }

    pub async fn recv(&mut self) -> Option<Result<RegistryClientEvent, String>> {
        while let Some(req) = self.queue.pop_front() {
            self.stream.write(&req).await.ok()?;
        }

        loop {
            match self.stream.read::<Response>().await? {
                Ok(event) => match event.res? {
                    protocol::registry::response::Res::Update(res) => {
                        log::info!("[RegistryClient] will connect to {:?}", res.neighbours);
                    }
                    protocol::registry::response::Res::Neighbours(_) => todo!(),
                },
                Err(err) => break Some(Err(err.to_string())),
            }
        }
    }

    fn gen_req_id(&mut self) -> ReqId {
        self.req_id_seed += 1;
        ReqId(self.req_id_seed)
    }
}
