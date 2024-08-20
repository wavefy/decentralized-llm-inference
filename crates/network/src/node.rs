use crate::addr::NodeId;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OutgoingId(u64);

#[derive(Debug, PartialEq, Eq)]
pub enum OutgoingError {
    NoConnection,
    Timeout,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IncomingError {
    MaxConnection,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MsgId(u64);

#[derive(Debug, PartialEq, Eq)]
pub enum MsgError {
    NoConnection,
    Timeout,
}

pub struct ConnectionStats {
    pub rtt: u16,
}

pub enum NodeEvent {
    NodeConnect(OutgoingId, NodeId, String),
    NodeConnected(NodeId),
    NodeStats(NodeId, ConnectionStats),
    NodeMsg(NodeId, Vec<u8>),
    NodeDisconnected(NodeId),
    // feedback for out connection
    ConnectAck(Result<(), MsgError>),
    // feedback for message sent success or not
    MsgAck(Result<(), MsgError>),
}

pub struct NetworkNode {
    node: NodeId,
}

impl NetworkNode {
    pub fn new(node: NodeId) -> Self {
        Self { node }
    }

    pub fn send(&mut self, dest: NodeId, data: Vec<u8>) -> Result<MsgId, MsgError> {
        todo!()
    }

    pub fn connect(&mut self, dest: NodeId) -> Result<OutgoingId, OutgoingError> {
        let offer = "TODO";
        todo!()
    }

    pub fn on_offer(&mut self, from: NodeId, offer: &str) -> Result<String, IncomingError> {
        todo!()
    }

    pub fn on_answer(&mut self, conn: OutgoingId, res: Result<String, OutgoingError>) {}

    pub async fn recv(&mut self) -> Option<NodeEvent> {
        todo!()
    }
}
