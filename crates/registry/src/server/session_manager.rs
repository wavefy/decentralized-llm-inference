use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
};

use p2p_network::addr::NodeId;
use protocol::registry::to_worker::{NeighboursReply, UpdateReply};

#[derive(Default, Debug)]
struct NodeInfo {
    layers: Option<Range<u32>>,
}

#[derive(Default)]
pub struct SessionManager {
    nodes: HashMap<NodeId, NodeInfo>,
    events: VecDeque<(NodeId, protocol::registry::to_worker::Event)>,
}

impl SessionManager {
    pub fn on_start(&mut self, node: NodeId) {
        self.nodes.insert(node, Default::default());
    }

    pub fn on_event(&mut self, node: NodeId, event: protocol::registry::to_registry::Event) {
        match event {
            protocol::registry::to_registry::Event::Update(update) => {
                log::info!("[SessionManager] from node {} update layers [{}, {}]", node.0, update.from_layer, update.to_layer);
                let node_info = self.nodes.get_mut(&node).expect("Should have node");
                node_info.layers = Some(update.from_layer..update.to_layer);
                let neighbours = protocol::registry::to_worker::Event::Update(UpdateReply {
                    neighbours: self.nodes.keys().filter(|n| *n != &node).map(|n| n.0.clone()).collect::<Vec<_>>(),
                });
                self.events.push_back((node, neighbours));
            }
            protocol::registry::to_registry::Event::Neighbours(_) => {
                let neighbours = protocol::registry::to_worker::Event::Neighbours(NeighboursReply {
                    neighbours: self.nodes.keys().filter(|n| *n != &node).map(|n| n.0.clone()).collect::<Vec<_>>(),
                });
                self.events.push_back((node, neighbours));
            }
            protocol::registry::to_registry::Event::Relay(data) => {
                let dest = NodeId(data.dest);
                if self.nodes.contains_key(&dest) {
                    self.events.push_back((
                        dest,
                        protocol::registry::to_worker::Event::Relay(protocol::registry::to_worker::Relay { source: node.0, data: data.data }),
                    ))
                }
            }
        }
    }

    pub fn on_end(&mut self, node: NodeId) {
        self.nodes.remove(&node);
    }

    pub fn pop_out(&mut self) -> Option<(NodeId, protocol::registry::to_worker::Event)> {
        self.events.pop_front()
    }
}
