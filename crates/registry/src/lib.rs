use network::addr::NodeId;
use protocol::ModelLayersRanger;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ReqId(pub u64);

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ModelId(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum OfferError {}

#[derive(Debug, PartialEq, Eq)]
pub enum AnswerError {}

#[derive(Debug, PartialEq, Eq)]
pub struct NeighbourInfo {
    pub node_id: NodeId,
    pub range: ModelLayersRanger,
}
