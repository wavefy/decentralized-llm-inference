use network::addr::NodeId;
use protocol::ModelLayersRanger;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ModelId(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum OfferError {}

#[derive(Debug, PartialEq, Eq)]
pub enum AnswerError {
    Remote(String),
}
