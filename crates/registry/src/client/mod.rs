use network::addr::NodeId;
use protocol::ModelLayersRanger;

use crate::{AnswerError, NeighbourInfo, OfferError, OfferId};

#[derive(Debug, PartialEq, Eq)]
pub enum RegistryClientEvent {
    Answer(OfferId, Result<String, OfferError>),
    Offer(NodeId, OfferId, String),
    Neighbours(Vec<NeighbourInfo>),
}

pub struct RegistryClient {}

impl RegistryClient {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, model: &str, layers_range: ModelLayersRanger) {
        todo!()
    }

    pub fn find_neigbours(&mut self) {}

    pub fn offer(&mut self, dest: NodeId, offer: &str) -> OfferId {
        todo!()
    }

    pub fn answer(&mut self, dest: NodeId, offer: OfferId, answer: Result<String, AnswerError>) {
        todo!()
    }

    pub async fn recv(&mut self) -> Option<RegistryClientEvent> {
        todo!()
    }
}
