use model_router::RoutePath;
use network::addr::NodeId;
use protocol::{
    worker::event::{EndReq, EndRes, ForwardReq, ForwardRes, StartReq, StartRes},
    Session,
};
use tokio::sync::oneshot;

use super::SessionRes;

pub struct LlmSession {
    session: Session,
    pre: Option<NodeId>,
    path: RoutePath<NodeId>,
    res_tx: Option<oneshot::Sender<SessionRes>>,
}

impl LlmSession {
    pub fn new(session: Session, pre: Option<NodeId>, path: RoutePath<NodeId>) -> Self {
        Self { session, pre, path, res_tx: None }
    }

    pub fn set_res_tx(&mut self, tx: oneshot::Sender<SessionRes>) {
        assert_eq!(self.pre, None);
        self.res_tx = Some(tx);
    }

    pub fn take_res_tx(&mut self) -> Option<oneshot::Sender<SessionRes>> {
        self.res_tx.take()
    }

    pub fn is_local_process(&self) -> bool {
        self.path.local.is_some()
    }

    pub fn is_last(&self) -> bool {
        self.path.remote.is_none() //if don't have next remote then it is last
    }

    pub fn start_req_next(&self) -> (NodeId, protocol::worker::Event) {
        match (&self.path.local, &self.path.remote) {
            (None, Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] passthrough start_req to next {dest:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartReq(StartReq {
                            session_id: self.session.0,
                            from_layer: remote_range.0,
                        })),
                    },
                )
            }
            (None, None) => {
                todo!()
            }
            (Some(local_range), Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] forward start_req to next {dest:?}, from {remote_range:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartReq(StartReq {
                            session_id: self.session.0,
                            from_layer: remote_range.0,
                        })),
                    },
                )
            }
            (Some(local_range), None) => {
                log::info!("[LlmSession] answer start_req to pre {:?}", self.pre);
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::StartRes(StartRes {
                            session_id: self.session.0,
                            from_layer: local_range.0,
                        })),
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
                event: Some(protocol::worker::event::Event::StartRes(StartRes {
                    session_id: self.session.0,
                    from_layer: 0, //TODO
                })),
            },
        )
    }

    pub fn forward_req_next(&self, step: u32, payload: Vec<u8>, seq_len: u32, index_pos: u32) -> (NodeId, protocol::worker::Event) {
        match (&self.path.local, &self.path.remote) {
            (None, Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] passthrough forward_req to next {dest:?}, payload {} bytes", payload.len());
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardReq(ForwardReq {
                            session_id: self.session.0,
                            from_layer: remote_range.0,

                            step,
                            payload,
                            seq_len,
                            index_pos,
                        })),
                    },
                )
            }
            (None, None) => {
                todo!()
            }
            (Some(local_range), Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] forward forward_req to next {dest:?}, range {remote_range:?}, payload {} bytes", payload.len());
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardReq(ForwardReq {
                            session_id: self.session.0,
                            from_layer: remote_range.0,

                            step,
                            payload,
                            seq_len,
                            index_pos,
                        })),
                    },
                )
            }
            (Some(local_range), None) => {
                log::info!("[LlmSession] answer forward_req to pre {:?}, payload {} bytes", self.pre, payload.len());
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::ForwardRes(ForwardRes {
                            session_id: self.session.0,
                            from_layer: local_range.0,

                            step,
                            payload,
                            seq_len,
                            index_pos,
                        })),
                    },
                )
            }
        }
    }

    pub fn forward_res_next(&self, step: u32, payload: Vec<u8>, seq_len: u32, index_pos: u32) -> (NodeId, protocol::worker::Event) {
        log::info!("[LlmSession] backward forward_res to pre {:?}", self.pre);
        (
            self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
            protocol::worker::Event {
                event: Some(protocol::worker::event::Event::ForwardRes(ForwardRes {
                    session_id: self.session.0,
                    from_layer: 0, //TODO

                    step,
                    payload,
                    seq_len,
                    index_pos,
                })),
            },
        )
    }

    pub fn end_req_next(&self) -> (NodeId, protocol::worker::Event) {
        match (&self.path.local, &self.path.remote) {
            (None, Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] passthrough end_req to next {dest:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndReq(EndReq {
                            session_id: self.session.0,
                            from_layer: remote_range.0,
                        })),
                    },
                )
            }
            (None, None) => {
                todo!()
            }
            (Some(local_range), Some((dest, remote_range, _, _))) => {
                log::info!("[LlmSession] forward end_req to next {dest:?}, range {remote_range:?}");
                (
                    dest.clone(),
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndReq(EndReq {
                            session_id: self.session.0,
                            from_layer: local_range.0,
                        })),
                    },
                )
            }
            (Some(local_range), None) => {
                log::info!("[LlmSession] answer end_req to pre {:?}", self.pre);
                (
                    self.pre.clone().unwrap(), //TODO check if we dont have pre but in LastProcess (single node entire llm layers)
                    protocol::worker::Event {
                        event: Some(protocol::worker::event::Event::EndRes(EndRes {
                            session_id: self.session.0,
                            from_layer: local_range.0,
                        })),
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
                event: Some(protocol::worker::event::Event::EndRes(EndRes {
                    session_id: self.session.0,
                    from_layer: 0, //TODO
                })),
            },
        )
    }
}
