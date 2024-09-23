use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::{Duration, Instant},
};

use local_ip_address::local_ip;
use remote_conn::RemoteConn;
use tokio::{net::UdpSocket, time::Interval};

use crate::{
    addr::NodeId,
    shared_port::SharedUdpPort,
    stun::{get_public_ip, process_stun_response, send_stun_request},
};

mod remote_conn;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ConnId(pub u64);

impl ConnId {
    pub fn rand() -> Self {
        Self(rand::random())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum OutgoingError {
    NoConnection,
    Timeout,
    RemoteError(String),
    AlreadyExist,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IncomingError {
    RemoteNotFound,
    SdpError,
    MaxConnection,
    AlreadyHas,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    NoNode,
    NoConnection,
    ConnectionNotReady,
    Timeout,
}

pub struct ConnectionStats {
    pub rtt: u16,
}

pub enum NodeEvent<MSG> {
    NodeConnected(ConnId, NodeId),
    NodeStats(ConnId, NodeId, ConnectionStats),
    NodeMsg(ConnId, NodeId, MSG),
    NodeDisconnected(ConnId, NodeId),
}

pub struct NetworkNode<MSG> {
    node: NodeId,
    udp: UdpSocket,
    udp_buf: Vec<u8>,
    conns: HashMap<ConnId, RemoteConn>,
    nodes: HashMap<NodeId, ConnId>,
    events: VecDeque<NodeEvent<MSG>>,
    shared_udp: SharedUdpPort<ConnId>,
    interval: Interval,
    stun_servers: Vec<SocketAddr>,
    public_addr: Option<SocketAddr>,
    last_sent_stun: Instant,
}

impl<MSG: prost::Message + Default> NetworkNode<MSG> {
    pub async fn new(node: NodeId, stun_servers: Vec<SocketAddr>) -> Self {
        let local_ip = local_ip().unwrap();
        let udp = UdpSocket::bind(SocketAddr::new(local_ip, 0)).await.expect("Should listen");
        let public_addr = get_public_ip(&udp, &stun_servers).await.ok();
        log::info!("[NetworkNode] {node:?} local {local_ip} public {public_addr:?}");

        Self {
            node,
            udp,
            udp_buf: vec![0; 1500],
            nodes: HashMap::new(),
            conns: HashMap::new(),
            events: VecDeque::new(),
            shared_udp: SharedUdpPort::default(),
            interval: tokio::time::interval(Duration::from_millis(1)),
            stun_servers,
            public_addr,
            last_sent_stun: Instant::now(),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        for (conn_id, conn) in self.conns.iter_mut() {
            if conn.on_tick(now) {
                Self::pop_conn(conn_id, conn, &self.udp, &mut self.events, &mut self.shared_udp);
            }
        }
    }

    pub fn send(&mut self, node: NodeId, data: &MSG) -> Result<usize, SendError> {
        let buf = data.encode_to_vec();
        let conn_id = self.nodes.get(&node).ok_or(SendError::NoNode)?;
        let conn = self.conns.get_mut(conn_id).ok_or(SendError::NoConnection)?;
        let appended = conn.send_data(&buf).ok_or(SendError::ConnectionNotReady)?;
        assert_eq!(buf.len(), appended, "Should send all buffer");

        Self::pop_conn(conn_id, conn, &self.udp, &mut self.events, &mut self.shared_udp);

        Ok(appended)
    }

    pub fn broadcast(&mut self, data: &MSG) -> Result<(), SendError> {
        let buf = data.encode_to_vec();
        for (conn_id, conn) in self.conns.iter_mut() {
            conn.send_data(&buf);
            Self::pop_conn(conn_id, conn, &self.udp, &mut self.events, &mut self.shared_udp);
        }

        Ok(())
    }

    pub fn connect(&mut self, dest: NodeId) -> Option<(ConnId, String)> {
        if !self.nodes.contains_key(&dest) {
            log::info!("[NetworkNode {:?}] connect to {dest:?}", self.node);
            let conn_id = ConnId::rand();
            self.nodes.insert(dest.clone(), conn_id);

            let (mut conn, ice_ufrag) = RemoteConn::new(dest, vec![(self.udp.local_addr().expect("Should have local"), self.public_addr)]);
            self.shared_udp.add_ufrag(ice_ufrag, conn_id);
            let offer = conn.create_offer();
            Self::pop_conn(&conn_id, &mut conn, &self.udp, &mut self.events, &mut self.shared_udp);
            self.conns.insert(conn_id, conn);

            Some((conn_id, offer))
        } else {
            None
        }
    }

    pub fn on_offer(&mut self, conn_id: ConnId, from: NodeId, offer: &str) -> Result<String, IncomingError> {
        if self.conns.contains_key(&conn_id) {
            Err(IncomingError::AlreadyHas)
        } else {
            let (mut conn, ice_ufrag) = RemoteConn::new(from.clone(), vec![(self.udp.local_addr().expect("Should have local"), self.public_addr)]);
            self.shared_udp.add_ufrag(ice_ufrag, conn_id);
            let answer = conn.accept_offer(offer).ok_or(IncomingError::SdpError)?;
            Self::pop_conn(&conn_id, &mut conn, &self.udp, &mut self.events, &mut self.shared_udp);
            self.conns.insert(conn_id, conn);
            self.nodes.insert(from, conn_id);
            Ok(answer)
        }
    }

    pub fn on_answer(&mut self, conn: ConnId, from: NodeId, answer: String) -> Result<(), IncomingError> {
        let remote = self.conns.get_mut(&conn).ok_or(IncomingError::RemoteNotFound)?;

        if remote.remote() == from {
            remote.on_answer(&answer).ok_or(IncomingError::SdpError)?;
            Self::pop_conn(&conn, remote, &self.udp, &mut self.events, &mut self.shared_udp);
            Ok(())
        } else {
            Err(IncomingError::RemoteNotFound)
        }
    }

    pub async fn shutdown(&mut self) {
        for (conn_id, conn) in self.conns.iter_mut() {
            conn.shutdown();
            Self::pop_conn(conn_id, conn, &self.udp, &mut self.events, &mut self.shared_udp);
        }
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    break;
                },
                e = self.recv() => {
                    if e.is_none() {
                        break;
                    }
                }
            }
        }
    }

    pub async fn recv(&mut self) -> Option<NodeEvent<MSG>> {
        loop {
            if let Some(out) = self.events.pop_front() {
                if let NodeEvent::NodeDisconnected(conn_id, node) = &out {
                    self.conns.remove(conn_id);
                    self.nodes.remove(node);
                }
                return Some(out);
            }

            tokio::select! {
                _ = self.interval.tick() => {
                    self.tick();
                    if self.last_sent_stun.elapsed() > Duration::from_secs(10) {
                        self.last_sent_stun = Instant::now();
                        send_stun_request(&self.udp, &self.stun_servers).await.expect("Should send stun request");
                    }
                },
                net_in = self.udp.recv_from(&mut self.udp_buf) => {
                    if let Ok((len, remote)) = net_in {
                        if self.stun_servers.contains(&remote) {
                            log::info!("[NetworkNode] recv stun response from {remote}");
                            match process_stun_response(&self.udp_buf[0..len]) {
                                Ok(public_ip) => {
                                    log::info!("[NetworkNode] public ip {public_ip}");
                                    self.public_addr = Some(public_ip);
                                }
                                Err(e) => {
                                    log::error!("[NetworkNode] process stun response error {e:?}");
                                }
                            }
                            continue;
                        }

                        log::debug!("[NetworkNode {:?}] recv {len} bytes from {remote}", self.node);
                        if let Some(conn_id) = self.shared_udp.map_remote(remote, &self.udp_buf[0..len]) {
                            if let Some(conn) = self.conns.get_mut(&conn_id) {
                                conn.on_data(Instant::now(), remote, self.udp.local_addr().unwrap(), &self.udp_buf[0..len]);
                                Self::pop_conn(&conn_id, conn, &self.udp, &mut self.events, &mut self.shared_udp);
                            } else {
                                log::info!("[NetworkNode] connection {conn_id:?} not found");
                            }
                        } else {
                            log::warn!("[NetworkNode] unknown dest for data from {remote}");
                        }
                    } else {
                        return None;
                    }
                },
            }
        }
    }

    fn pop_conn(conn_id: &ConnId, conn: &mut RemoteConn, udp: &UdpSocket, events: &mut VecDeque<NodeEvent<MSG>>, shared_udp: &mut SharedUdpPort<ConnId>) {
        while let Some(event) = conn.pop_outgoing() {
            match event {
                remote_conn::RemoteConnOut::Transmit(from, to, buf) => {
                    log::debug!("[NetworkNode] conn {conn_id:?} send {from} => {to} with len {}", buf.len());
                    if let Err(e) = udp.try_send_to(&buf, to) {
                        log::error!("[NetworkNode] send data to {to} error {e:?}");
                    }
                }
                remote_conn::RemoteConnOut::Connected => {
                    log::info!("[NetworkNode] conn {conn_id:?} connected");
                    events.push_back(NodeEvent::NodeConnected(*conn_id, conn.remote()));
                }
                remote_conn::RemoteConnOut::Message(data) => {
                    log::debug!("[NetworkNode] conn {conn_id:?} on data {}", data.len());
                    match MSG::decode(data.as_slice()) {
                        Ok(msg) => {
                            events.push_back(NodeEvent::NodeMsg(*conn_id, conn.remote(), msg));
                        }
                        Err(e) => {
                            log::error!("[NetworkNode] decode message error {e:?}");
                        }
                    }
                }
                remote_conn::RemoteConnOut::Disconnected => {
                    log::info!("[NetworkNode] conn {conn_id:?} disconnected");
                    shared_udp.remove_task(*conn_id);
                    events.push_back(NodeEvent::NodeDisconnected(*conn_id, conn.remote()));
                }
            }
        }
    }
}
