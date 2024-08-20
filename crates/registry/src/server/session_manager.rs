use std::collections::HashMap;

use network::addr::NodeId;
use protocol::{
    registry::response::{NeighboursReply, UpdateReply},
    ModelLayersRanger,
};

#[derive(Default, Debug)]
struct NodeInfo {
    layers: Option<ModelLayersRanger>,
}

#[derive(Default)]
pub struct SessionManager {
    nodes: HashMap<NodeId, NodeInfo>,
}

impl SessionManager {
    pub fn on_start(&mut self, node: NodeId) {
        self.nodes.insert(node, Default::default());
    }

    pub fn on_req(
        &mut self,
        node: NodeId,
        req: protocol::registry::request::Req,
    ) -> protocol::registry::response::Res {
        match req {
            protocol::registry::request::Req::Update(update) => {
                log::info!(
                    "[SessionManager] from node {} update layers [{}, {}]",
                    node.0,
                    update.from_layer,
                    update.to_layer
                );
                let node_info = self.nodes.get_mut(&node).expect("Should have node");
                node_info.layers = Some(ModelLayersRanger {
                    from: update.from_layer,
                    to: update.to_layer,
                });
                protocol::registry::response::Res::Update(UpdateReply {
                    neighbours: self
                        .nodes
                        .keys()
                        .filter(|n| *n != &node)
                        .map(|n| n.0.clone())
                        .collect::<Vec<_>>(),
                })
            }
            protocol::registry::request::Req::Neighbours(_) => {
                protocol::registry::response::Res::Neighbours(NeighboursReply {
                    neighbours: self
                        .nodes
                        .keys()
                        .filter(|n| *n != &node)
                        .map(|n| n.0.clone())
                        .collect::<Vec<_>>(),
                })
            }
        }
    }

    pub fn on_end(&mut self, node: NodeId) {
        self.nodes.remove(&node);
    }
}
