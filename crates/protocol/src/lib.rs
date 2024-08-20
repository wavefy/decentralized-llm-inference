pub mod registry {
    include!(concat!(env!("OUT_DIR"), "/registry.rs"));
}

pub mod worker {
    include!(concat!(env!("OUT_DIR"), "/worker.rs"));
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Session(u64);
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
