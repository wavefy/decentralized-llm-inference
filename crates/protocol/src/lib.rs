use model_router::{LayerRemoteInfo, RouteSync};
use std::fmt::Display;
use std::ops::Deref;

mod model;
mod openai;

pub use model::*;
pub use openai::*;

pub mod registry {
    include!(concat!(env!("OUT_DIR"), "/registry.rs"));
}

pub mod worker {
    include!(concat!(env!("OUT_DIR"), "/worker.rs"));
}

pub mod llm {
    include!(concat!(env!("OUT_DIR"), "/llm.rs"));
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Session(pub u64);
impl Session {
    pub fn new() -> Self {
        Self(rand::random())
    }
}

impl Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session({})", self.0)
    }
}

impl Deref for Session {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
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
