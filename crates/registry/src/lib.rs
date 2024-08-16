use network::addr::NodeId;
use protocol::ModelLayersRanger;

mod client;
mod server;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct OfferId(u64);

#[derive(Debug, PartialEq, Eq)]
pub enum OfferError {}

#[derive(Debug, PartialEq, Eq)]
pub enum AnswerError {}

#[derive(Debug, PartialEq, Eq)]
pub struct NeighbourInfo {
    pub node_id: NodeId,
    pub range: ModelLayersRanger,
}
