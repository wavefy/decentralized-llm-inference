use std::{collections::HashMap, fmt::Debug, hash::Hash, ops::Range};

const ROUTE_TIMEOUT_MS: u64 = 5_000; // a route will be removed after 5 seconds no update

#[derive(Debug, PartialEq, Clone)]
pub struct RoutePath<Node> {
    pub local: Option<Range<u32>>,
    pub remote: Option<(Node, Range<u32>, u32, u64)>,
}

impl<Node> RoutePath<Node> {
    fn cost(&self) -> u32 {
        match &self.remote {
            Some((_, _, cost, _)) => *cost,
            None => 0,
        }
    }

    fn last_updated(&self) -> Option<u64> {
        match &self.remote {
            Some((_, _, _, last_updated)) => Some(*last_updated),
            None => None,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct LayerRemotePaths<Node> {
    remotes: HashMap<Node, LayerRemoteInfo>,
    next: Option<(Node, LayerRemoteInfo)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerRemoteInfo {
    pub cost: u32,
    pub last_updated: u64,
}

impl LayerRemoteInfo {
    fn new(cost: u32, last_updated: u64) -> Self {
        Self { cost, last_updated }
    }
}

#[derive(Debug, PartialEq)]
pub struct RouteSync {
    pub layers: Vec<Option<LayerRemoteInfo>>,
}

impl RouteSync {
    pub fn dump(&self) {
        log::info!("==========start dump RouteSync==========");
        for (i, layer) in self.layers.iter().enumerate() {
            log::info!("layer {i}: {:?}", layer);
        }
        log::info!("==========end dump RouteSync==========");
    }
}

pub struct RouteTable<Node, const MODEL_LAYERS: usize> {
    remote_layers: [LayerRemotePaths<Node>; MODEL_LAYERS],
    local_layers: Range<u32>,
}

impl<Node: Clone + Debug + Eq + Hash, const MODEL_LAYERS: usize> RouteTable<Node, MODEL_LAYERS> {
    pub fn new(local_layers: Range<u32>) -> Self {
        Self {
            remote_layers: std::array::from_fn(|_| LayerRemotePaths {
                remotes: Default::default(),
                next: None,
            }),
            local_layers,
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        for (layer, route) in self.remote_layers.iter_mut().enumerate() {
            let pre = route.remotes.len();
            route.remotes.retain(|_dest, info| info.last_updated + ROUTE_TIMEOUT_MS > now_ms);
            if route.remotes.len() != pre {
                log::info!("layer {layer} remove {} timeout remotes", pre - route.remotes.len());
                route.update_best();
            }
        }
    }

    pub fn on_disconnected(&mut self, node: Node) {
        for (_layer, route) in self.remote_layers.iter_mut().enumerate() {
            if route.remotes.remove(&node).is_some() {
                route.update_best();
            }
        }
    }

    pub fn create_sync(&self, now_ms: u64) -> RouteSync {
        // log::info!("create sync");
        let mut layers = vec![None; MODEL_LAYERS];
        for layer in 0..MODEL_LAYERS {
            layers[layer] = self.select_next(layer as u32).map(|n: RoutePath<Node>| LayerRemoteInfo {
                cost: n.cost(),
                last_updated: n.last_updated().unwrap_or(now_ms),
            });
        }
        RouteSync { layers }
    }

    /// When we received a sync message from other node => we find if local layers can contribute to it
    /// We only care about
    pub fn apply_sync(&mut self, from: Node, rtt: u32, sync: RouteSync) {
        // log::info!("apply sync from {from:?} with rtt {rtt}");
        for layer in 0..MODEL_LAYERS {
            if let Some(Some(info)) = sync.layers.get(layer) {
                let mut info = info.clone();
                info.cost += rtt;
                self.remote_layers[layer].remotes.insert(from.clone(), info);
            } else {
                self.remote_layers[layer].remotes.remove(&from);
            }
            self.remote_layers[layer].update_best();
        }
    }

    pub fn select_next(&self, next_layer: u32) -> Option<RoutePath<Node>> {
        if self.local_layers.contains(&next_layer) {
            // if we can process some in local
            if self.local_layers.end == MODEL_LAYERS as u32 {
                // if this is last
                Some(RoutePath {
                    local: Some(next_layer..MODEL_LAYERS as u32),
                    remote: None,
                })
            } else {
                // if we need the help from other node
                self.remote_layers[self.local_layers.end as usize].next.as_ref().map(|(dest, info)| RoutePath {
                    local: Some(next_layer..self.local_layers.end),
                    remote: Some((dest.clone(), self.local_layers.end..MODEL_LAYERS as u32, info.cost, info.last_updated)),
                })
            }
        } else {
            self.remote_layers[next_layer as usize].next.as_ref().map(|(dest, info)| RoutePath {
                local: None,
                remote: Some((dest.clone(), next_layer..MODEL_LAYERS as u32, info.cost, info.last_updated)),
            })
        }
    }

    fn dump(&self) {
        log::info!("==========start dump==========");
        log::info!("local layers: {:?}", self.local_layers);
        log::info!("start remote layers");
        for layer in 0..MODEL_LAYERS {
            println!("layer {layer}: {:?}", self.remote_layers[layer].next);
        }
        log::info!("end remote layers");
        log::info!("start select next");
        for layer in 0..MODEL_LAYERS {
            let next = self.select_next(layer as u32);
            println!("layer {layer}: {next:?}");
        }
        log::info!("end select next");
        log::info!("==========end dump==========");
    }
}

impl<Node: Clone> LayerRemotePaths<Node> {
    pub fn update_best(&mut self) {
        self.next = self.remotes.iter().min_by(|a, b| a.1.cost.cmp(&b.1.cost)).map(|(dest, info)| (dest.clone(), info.clone()));
    }
}

#[cfg(test)]
mod tests {
    use crate::{table::ROUTE_TIMEOUT_MS, LayerRemoteInfo, RouteSync};

    type RoutePath = super::RoutePath<u8>;
    type RouteTable = super::RouteTable<u8, 3>;

    #[test]
    fn full_table() {
        let table = RouteTable::new(0..3);
        assert_eq!(
            table.create_sync(100),
            RouteSync {
                layers: vec![Some(LayerRemoteInfo::new(0, 100)), Some(LayerRemoteInfo::new(0, 100)), Some(LayerRemoteInfo::new(0, 100))]
            }
        );

        assert_eq!(table.select_next(0), Some(RoutePath { local: Some(0..3), remote: None }));

        assert_eq!(table.select_next(1), Some(RoutePath { local: Some(1..3), remote: None }));

        assert_eq!(table.select_next(2), Some(RoutePath { local: Some(2..3), remote: None }));
    }

    #[test]
    fn imcomplete_right() {
        let table = RouteTable::new(1..3);
        assert_eq!(
            table.create_sync(100),
            RouteSync {
                layers: vec![None, Some(LayerRemoteInfo::new(0, 100)), Some(LayerRemoteInfo::new(0, 100))]
            }
        );

        assert_eq!(table.select_next(0), None);

        assert_eq!(table.select_next(1), Some(RoutePath { local: Some(1..3), remote: None }));

        assert_eq!(table.select_next(2), Some(RoutePath { local: Some(2..3), remote: None }));
    }

    #[test]
    fn imcomplete_left() {
        let table = RouteTable::new(0..2);
        assert_eq!(table.create_sync(100), RouteSync { layers: vec![None, None, None] });

        assert_eq!(table.select_next(0), None);
        assert_eq!(table.select_next(1), None);
        assert_eq!(table.select_next(2), None);
    }

    #[test]
    fn imcomplete_right_sync() {
        let mut table = RouteTable::new(1..3);

        const REMOTE_NODE: u8 = 2;
        const RTT: u32 = 10;

        table.apply_sync(
            REMOTE_NODE,
            RTT,
            RouteSync {
                layers: vec![Some(LayerRemoteInfo::new(10, 100)), Some(LayerRemoteInfo::new(10, 100)), Some(LayerRemoteInfo::new(10, 100))],
            },
        );

        assert_eq!(
            table.select_next(0),
            Some(RoutePath {
                local: None,
                remote: Some((REMOTE_NODE, 0..3, 20, 100))
            })
        );

        assert_eq!(table.select_next(1), Some(RoutePath { local: Some(1..3), remote: None }));

        assert_eq!(table.select_next(2), Some(RoutePath { local: Some(2..3), remote: None }));
    }

    #[test]
    fn imcomplete_left_sync() {
        let mut table = RouteTable::new(0..1);

        const REMOTE_NODE: u8 = 2;
        const RTT: u32 = 10;

        table.apply_sync(
            REMOTE_NODE,
            RTT,
            RouteSync {
                layers: vec![None, Some(LayerRemoteInfo::new(0, 100)), Some(LayerRemoteInfo::new(0, 100))],
            },
        );

        assert_eq!(
            table.select_next(0),
            Some(RoutePath {
                local: Some(0..1),
                remote: Some((REMOTE_NODE, 1..3, 10, 100))
            })
        );

        assert_eq!(
            table.select_next(1),
            Some(RoutePath {
                local: None,
                remote: Some((REMOTE_NODE, 1..3, 10, 100))
            })
        );

        assert_eq!(
            table.select_next(2),
            Some(RoutePath {
                local: None,
                remote: Some((REMOTE_NODE, 2..3, 10, 100))
            })
        );
    }

    #[test]
    fn remote_timeout() {
        let mut table = RouteTable::new(0..1);

        const REMOTE_NODE: u8 = 2;
        const RTT: u32 = 10;

        table.apply_sync(
            REMOTE_NODE,
            RTT,
            RouteSync {
                layers: vec![None, Some(LayerRemoteInfo::new(0, 100)), Some(LayerRemoteInfo::new(0, 100))],
            },
        );

        // before tick => have next
        assert!(table.select_next(0).is_some());
        table.on_tick(100 + ROUTE_TIMEOUT_MS);
        // after tick => haven't next
        assert!(table.select_next(0).is_none());
    }
}
