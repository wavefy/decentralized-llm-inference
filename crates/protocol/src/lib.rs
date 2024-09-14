use model_router::{LayerRemoteInfo, RouteSync};

pub mod registry {
    include!(concat!(env!("OUT_DIR"), "/registry.rs"));
}

pub mod worker {
    include!(concat!(env!("OUT_DIR"), "/worker.rs"));
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Session(pub u64);
impl Session {
    pub fn new() -> Self {
        Self(rand::random())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelLayersRanger {
    pub from: u32,
    pub to: u32,
}

impl ModelLayersRanger {
    pub fn new(from: u32, to: u32) -> Self {
        Self { from, to }
    }
    pub fn len(&self) -> usize {
        (self.to - self.from + 1) as usize
    }
}

impl From<RouteSync> for worker::event::SyncReq {
    fn from(value: RouteSync) -> Self {
        worker::event::SyncReq {
            layers: value
                .layers
                .into_iter()
                .map(|l| match l {
                    Some(l) => worker::event::sync_req::LayerRemoteInfo {
                        enable: true,
                        cost: l.cost,
                        last_updated: l.last_updated,
                    },
                    None => worker::event::sync_req::LayerRemoteInfo {
                        enable: false,
                        cost: 0,
                        last_updated: 0,
                    },
                })
                .collect::<Vec<_>>(),
        }
    }
}

impl From<worker::event::SyncReq> for RouteSync {
    fn from(value: worker::event::SyncReq) -> Self {
        RouteSync {
            layers: value
                .layers
                .into_iter()
                .map(|l| {
                    if l.enable {
                        Some(LayerRemoteInfo {
                            cost: l.cost,
                            last_updated: l.last_updated,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
        }
    }
}
