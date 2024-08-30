use model_router::RouteAction;
use network::addr::NodeId;
use protocol::{
    worker::event::{EndReq, EndRes, ForwardReq, ForwardRes, StartReq, StartRes},
    Session,
};

pub struct LlmSession {
    session: Session,
    pre: Option<NodeId>,
    action: RouteAction<NodeId>,
    inited: bool,
}

impl LlmSession {
    pub fn new(session: Session, pre: Option<NodeId>, action: RouteAction<NodeId>) -> Self {
        let inited = matches!(&action, RouteAction::LastProcess);
        Self { session, pre, action, inited }
    }

    pub fn is_first(&self) -> bool {
        self.pre.is_none()
    }

    pub fn is_local_process(&self) -> bool {
        !matches!(self.action, RouteAction::PassthroughTo { .. })
    }

    pub fn is_last(&self) -> bool {
        matches!(self.action, RouteAction::LastProcess)
    }

    pub fn set_inited(&mut self) {
        self.inited = true;
    }

    pub fn start_req_next(&self) -> (NodeId, protocol::worker::Event) {
        match &self.action {
            RouteAction::PassthroughTo { dest } => {
                log::info!("[LlmSession] passthrough start_req to next {dest:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartReq(StartReq { session_id: self.session.0, from: 0 })),
                    },
                )
            }
            RouteAction::ForwardTo { dest, from } => {
                log::info!("[LlmSession] forward start_req to next {dest:?}, from {from}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartReq(StartReq {
                            session_id: self.session.0,
                            from: *from,
                        })),
                    },
                )
            }
            RouteAction::LastProcess => {
                log::info!("[LlmSession] answer start_req to pre {:?}", self.pre);
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartRes(StartRes { session_id: self.session.0 })),
                    },
                )
            }
        }
    }

    pub fn start_res_next(&self) -> (NodeId, protocol::worker::Event) {
        log::info!("[LlmSession] backward start_res to pre {:?}", self.pre);
        (
            self.pre.clone().unwrap(), //TODO check in case pre is None
            protocol::worker::Event {
                event: Some(protocol::worker::event::Event::StartRes(StartRes { session_id: self.session.0 })),
            },
        )
    }

    pub fn forward_req_next(&self, step: u32, payload: Vec<u8>) -> (NodeId, protocol::worker::Event) {
        match &self.action {
            RouteAction::PassthroughTo { dest } => {
                log::info!("[LlmSession] passthrough forward_req to next {dest:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardReq(ForwardReq {
                            session_id: self.session.0,
                            step,
                            from: 0,
                            payload,
                        })),
                    },
                )
            }
            RouteAction::ForwardTo { dest, from } => {
                log::info!("[LlmSession] forward forward_req to next {dest:?}, from {from}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardReq(ForwardReq {
                            session_id: self.session.0,
                            step,
                            from: *from,
                            payload,
                        })),
                    },
                )
            }
            RouteAction::LastProcess => {
                log::info!("[LlmSession] answer forward_req to pre {:?}", self.pre);
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardRes(ForwardRes {
                            session_id: self.session.0,
                            step,
                            payload,
                        })),
                    },
                )
            }
        }
    }

    pub fn forward_res_next(&self, step: u32, payload: Vec<u8>) -> (NodeId, protocol::worker::Event) {
        log::info!("[LlmSession] backward forward_res to pre {:?}", self.pre);
        (
            self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
            protocol::worker::Event {
                event: Some(protocol::worker::event::Event::ForwardRes(ForwardRes {
                    session_id: self.session.0,
                    step,
                    payload,
                })),
            },
        )
    }

    pub fn end_req_next(&self) -> (NodeId, protocol::worker::Event) {
        match &self.action {
            RouteAction::PassthroughTo { dest } => {
                log::info!("[LlmSession] passthrough end_req to next {dest:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndReq(EndReq { session_id: self.session.0 })),
                    },
                )
            }
            RouteAction::ForwardTo { dest, from } => {
                log::info!("[LlmSession] forward end_req to next {dest:?}, from {from}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndReq(EndReq { session_id: self.session.0 })),
                    },
                )
            }
            RouteAction::LastProcess => {
                log::info!("[LlmSession] answer end_req to pre {:?}", self.pre);
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndRes(EndRes { session_id: self.session.0 })),
                    },
                )
            }
        }
    }

    pub fn end_res_next(&self) -> (NodeId, protocol::worker::Event) {
        log::info!("[LlmSession] backward end_res to pre {:?}", self.pre);
        (
            self.pre.clone().unwrap(), //TODO check in case pre is None
            protocol::worker::Event {
                event: Some(protocol::worker::event::Event::EndRes(EndRes { session_id: self.session.0 })),
            },
        )
    }
}
