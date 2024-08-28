use std::{collections::HashMap, hash::Hash};

const REMOTE_TIMEOUT: u64 = 10000;

pub struct ModelRange {
    pub from: u32,
    pub to: u32,
}

enum NextLayer<Node> {
    Local { cost: u32 },
    Remote { next: Node, cost: u32, last_updated: u64 },
}

struct RemoteContainer<Node> {
    layers: Vec<Option<(Node, u32)>>,
    cost: u32,
    last_updated: u64,
}

pub struct ModelRouterSync<Node> {
    layers: Vec<Option<(Node, u32)>>,
}

pub struct ModelRouter<Node> {
    node: Node,
    layers_local: ModelRange,
    layers_total: u32,
    layers: Vec<Option<NextLayer<Node>>>,
    remotes: HashMap<Node, RemoteContainer<Node>>,
}

impl<Node: Hash + Eq + PartialEq + Copy + Clone + 'static> ModelRouter<Node> {
    pub fn set_local(&mut self, range: ModelRange, total: u32) {
        self.layers_local = range;
        self.layers_total = total;
        while self.layers.len() < total as usize {
            self.layers.push(None);
        }

        while self.layers.len() > total as usize {
            self.layers.pop();
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        let mut timeout_nodes = vec![];
        for (remote, container) in self.remotes.iter_mut() {
            if container.last_updated + REMOTE_TIMEOUT <= now_ms {
                timeout_nodes.push(*remote);
            }
        }

        for (index, layer) in self.layers.iter_mut().enumerate() {
            //TODO: calc most
        }
    }

    pub fn on_remote_sync(&mut self, now_ms: u64, from: Node, cost: u32, sync: ModelRouterSync<Node>) {
        let entry = self.remotes.entry(from).or_insert_with(|| RemoteContainer {
            layers: vec![],
            last_updated: now_ms,
            cost,
        });
        entry.layers = sync.layers;
        entry.cost = cost;
        entry.last_updated = now_ms;
    }

    pub fn on_remote_shutdown(&mut self, _now_ms: u64, from: Node) {
        self.remotes.remove(&from);
    }

    pub fn build_sync(&mut self, _now_ms: u64, _dest: Node) -> ModelRouterSync<Node> {
        let layers = self
            .layers
            .iter()
            .map(|l| {
                l.as_ref().map(|l| match l {
                    NextLayer::Local { cost } => (self.node, *cost),
                    NextLayer::Remote { next, cost, .. } => (*next, *cost),
                })
            })
            .collect::<Vec<_>>();
        ModelRouterSync { layers }
    }
}
