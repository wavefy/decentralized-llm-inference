use models::remote::{RpcRequest, RpcResponse};
use protocol::Session;

pub enum SessionReq {
    Start,
    Forward,
    Stop,
}

pub enum SessionRes {
    Started,
    Backward,
    Stopped,
}

pub enum WorkerRunnerEvent {
    Session(Session, SessionRes),
}

pub struct WorkerRunner {}

impl WorkerRunner {
    pub fn session_req(&mut self, session: Session, req: SessionReq) {
        todo!()
    }

    pub async fn recv(&mut self) -> Option<WorkerRunnerEvent> {
        todo!()
    }
}
