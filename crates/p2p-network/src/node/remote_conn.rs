use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    net::SocketAddr,
    time::Instant,
};
use str0m::{
    change::{SdpAnswer, SdpOffer, SdpPendingOffer},
    channel::ChannelId,
    net::{Protocol, Receive},
    Candidate, IceCreds, Input, Rtc, RtcConfig,
};

use crate::addr::NodeId;

pub enum RemoteConnOut {
    Transmit(SocketAddr, SocketAddr, Vec<u8>),
    Connected,
    Message(Vec<u8>),
    Disconnected,
}

pub struct RemoteConn {
    remote: NodeId,
    rtc: Rtc,
    pending: Option<SdpPendingOffer>,
    queue: VecDeque<RemoteConnOut>,
    channel: Option<ChannelId>,
    timeout: Option<Instant>,
    buffer: ConnectionBuffer<4096, 150_000>,
}

impl RemoteConn {
    pub fn new(remote: NodeId, local_addrs: Vec<(SocketAddr, Option<SocketAddr>)>) -> (Self, String) {
        let rtc_config = RtcConfig::new().set_local_ice_credentials(IceCreds::new());
        let ice_ufrag = rtc_config.local_ice_credentials().as_ref().expect("should have ice credentials").ufrag.clone();
        let mut rtc = rtc_config.build();

        for (addr, mapped_addr) in local_addrs {
            rtc.add_local_candidate(Candidate::host(addr, Protocol::Udp).expect("Should create candidate"));
            if let Some(mapped_addr) = mapped_addr {
                rtc.add_local_candidate(Candidate::server_reflexive(mapped_addr, addr, Protocol::Udp).expect("Should create candidate"));
            }
        }

        (
            Self {
                remote,
                rtc,
                pending: None,
                queue: VecDeque::new(),
                channel: None,
                timeout: None,
                buffer: Default::default(),
            },
            ice_ufrag,
        )
    }

    pub fn remote(&self) -> NodeId {
        self.remote.clone()
    }

    pub fn create_offer(&mut self) -> String {
        let mut api = self.rtc.sdp_api();
        api.add_channel("data".to_string());
        let (offer, pending) = api.apply().expect("Should create offer");
        self.pending = Some(pending);
        offer.to_sdp_string()
    }

    pub fn accept_offer(&mut self, offer: &str) -> Option<String> {
        let offer = SdpOffer::from_sdp_string(offer).ok()?;
        let answer = self.rtc.sdp_api().accept_offer(offer).ok()?;
        Some(answer.to_sdp_string())
    }

    pub fn on_answer(&mut self, answer: &str) -> Option<()> {
        let answer = SdpAnswer::from_sdp_string(answer).ok()?;
        let pending = self.pending.take()?;
        self.rtc.sdp_api().accept_answer(pending, answer).ok()?;
        Some(())
    }

    pub fn on_tick(&mut self, now: Instant) -> bool {
        if let Some(timeout) = self.timeout {
            if timeout <= now {
                self.timeout = None;
                log::debug!("[RemoteConn {:?}] tick", self.remote);
                return self.rtc.handle_input(Input::Timeout(now)).is_ok();
            }
        }
        false
    }

    pub fn send_data(&mut self, buf: &[u8]) -> Option<usize> {
        self.buffer.push_frame(buf);
        self.pop_outgoing_buffer();
        Some(buf.len())
    }

    pub fn on_data(&mut self, now: Instant, from: SocketAddr, to: SocketAddr, buf: &[u8]) -> Option<()> {
        log::debug!("[RemoteConn {:?}] recv {} bytes from {}", self.remote, buf.len(), from);
        self.rtc.handle_input(Input::Receive(now, Receive::new(Protocol::Udp, from, to, buf).ok()?)).ok()
    }

    pub fn shutdown(&mut self) {
        if let Some(channel) = self.channel.take() {
            self.rtc.direct_api().close_data_channel(channel)
        }
    }

    pub fn pop_outgoing(&mut self) -> Option<RemoteConnOut> {
        loop {
            log::debug!("[RemoteConn {:?}] try pop", self.remote());
            if let Some(out) = self.queue.pop_front() {
                return Some(out);
            }
            match self.rtc.poll_output().ok()? {
                str0m::Output::Timeout(timeout) => {
                    let after = timeout - Instant::now();
                    if after.is_zero() {
                        self.rtc.handle_input(Input::Timeout(Instant::now())).ok()?;
                        continue;
                    } else {
                        log::debug!("[RemoteConn {:?}] set timeout after {:?}", self.remote(), timeout - Instant::now());
                        self.timeout = Some(timeout);
                    }
                }
                str0m::Output::Transmit(out) => {
                    log::debug!("[RemoteConn {:?}] transmit {} bytes to {}", self.remote, out.contents.len(), out.destination);
                    self.queue.push_back(RemoteConnOut::Transmit(out.source, out.destination, out.contents.to_vec()))
                }
                str0m::Output::Event(event) => {
                    match event {
                        str0m::Event::IceConnectionStateChange(state) => {
                            log::info!("[RemoteConn {:?}] state changed to {state:?}", self.remote);
                            if let str0m::IceConnectionState::Disconnected = state {
                                self.queue.push_back(RemoteConnOut::Disconnected);
                            }
                        }
                        str0m::Event::ChannelOpen(channel, name) => {
                            log::info!("[RemoteConn {:?}] opened channel {name}", self.remote);
                            self.queue.push_back(RemoteConnOut::Connected);
                            self.channel = Some(channel);
                        }
                        str0m::Event::ChannelData(data) => {
                            log::debug!("[RemoteConn {:?}] on channel data {}", self.remote, data.data.len());
                            if self.buffer.on_received(&data.data).is_none() {
                                log::warn!("[RemoteConn {:?}] on channel data {} process failure", self.remote, data.data.len());
                            }
                            while let Some(frame) = self.buffer.pop_recv() {
                                self.queue.push_back(RemoteConnOut::Message(frame));
                            }
                            self.pop_outgoing_buffer();
                        }
                        str0m::Event::ChannelClose(_) => {
                            log::info!("[RemoteConn {:?}] closed channel", self.remote);
                            if self.channel.take().is_some() {
                                self.queue.push_back(RemoteConnOut::Disconnected);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
            }
            if self.queue.is_empty() {
                return None;
            }
        }
    }

    fn pop_outgoing_buffer(&mut self) -> Option<()> {
        let channel = self.channel?;
        while let Some(chunk) = self.buffer.pop_send() {
            let appended = self.rtc.channel(channel)?.write(true, &chunk).expect("should write to channel");
            assert_eq!(appended, chunk.len());
        }

        Some(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
enum ConnectionChunk {
    Chunk { frame_id: u32, chunk_id: u16, chunk_count: u16, data: Vec<u8> },
    ChunkAck { frame_id: u32, chunk_id: u16, chunk_count: u16 },
}

impl ConnectionChunk {
    fn in_air_size(&self) -> usize {
        if matches!(self, ConnectionChunk::Chunk { .. }) {
            bincode::serialized_size(self).expect("should calc binary size") as usize
        } else {
            0
        }
    }
}

struct IncomingFrame {
    chunk_count: u16,
    chunks: BTreeMap<u16, Vec<u8>>,
}

struct OutgoingFrame {
    chunks: BTreeMap<u16, usize>,
}

#[derive(Default)]
struct ConnectionBuffer<const CHUNK_SIZE: usize, const AIR_LIMIT: usize> {
    frame_id_seed: u32,
    high_priority: VecDeque<ConnectionChunk>,
    low_priority: VecDeque<ConnectionChunk>,
    incomings: HashMap<u32, IncomingFrame>,
    outgoings: HashMap<u32, OutgoingFrame>,
    outs: VecDeque<Vec<u8>>,
    in_air_size: usize,
}

impl<const CHUNK_SIZE: usize, const AIR_LIMIT: usize> ConnectionBuffer<CHUNK_SIZE, AIR_LIMIT> {
    pub fn push_frame(&mut self, data: &[u8]) {
        log::debug!("[ConnectionBuffer] push frame {} bytes, in_air {}/{}", data.len(), self.in_air_size, AIR_LIMIT);
        let frame_id = self.frame_id_seed;
        let mut frame = OutgoingFrame { chunks: Default::default() };
        self.frame_id_seed += 1;
        if data.len() <= CHUNK_SIZE {
            log::debug!("[ConnectionBuffer] push frame {frame_id}, single chunk to queue");
            let chunk = ConnectionChunk::Chunk {
                frame_id,
                chunk_id: 0,
                chunk_count: 1,
                data: data.to_vec(),
            };
            frame.chunks.insert(0, chunk.in_air_size());
            self.outgoings.insert(frame_id, frame);
            self.high_priority.push_back(chunk);
        } else {
            let chunks = data.chunks(CHUNK_SIZE);
            let chunk_count = chunks.len() as u16;
            for (chunk_id, chunk) in chunks.enumerate() {
                log::debug!("[ConnectionBuffer] push frame {frame_id}, chunk {chunk_id} to queue");
                let chunk = ConnectionChunk::Chunk {
                    frame_id,
                    chunk_id: chunk_id as u16,
                    chunk_count,
                    data: chunk.to_vec(),
                };
                let chunk_size = chunk.in_air_size();
                frame.chunks.insert(chunk_id as u16, chunk_size);
                self.low_priority.push_back(chunk);
            }
            self.outgoings.insert(frame_id, frame);
        }
    }

    pub fn on_received(&mut self, data: &[u8]) -> Option<()> {
        let ob: ConnectionChunk = bincode::deserialize(data).ok()?;
        match ob {
            ConnectionChunk::Chunk {
                frame_id,
                chunk_id,
                chunk_count,
                data,
            } => {
                log::debug!("[ConnectionBuffer] received frame {frame_id} chunk {chunk_id} / {chunk_count}");
                self.high_priority.push_back(ConnectionChunk::ChunkAck {
                    frame_id,
                    chunk_id: chunk_id,
                    chunk_count: chunk_count,
                });
                if chunk_count == 1 {
                    self.outs.push_back(data);
                } else {
                    let frame = self.incomings.entry(frame_id).or_insert_with(|| IncomingFrame {
                        chunk_count: chunk_count,
                        chunks: BTreeMap::new(),
                    });
                    frame.chunks.insert(chunk_id, data);

                    if frame.chunks.len() == frame.chunk_count as usize {
                        let frame = self.incomings.remove(&frame_id).expect("should have frame");
                        let frame_data = frame.chunks.into_iter().map(|(_k, v)| v).collect::<Vec<_>>().concat();
                        self.outs.push_back(frame_data);
                    }
                }
            }
            ConnectionChunk::ChunkAck { frame_id, chunk_id, chunk_count } => {
                if let Some(frame) = self.outgoings.get_mut(&frame_id) {
                    if let Some(size) = frame.chunks.remove(&chunk_id) {
                        log::debug!("[ConnectionBuffer] received ack for frame {frame_id} chunk {chunk_id} / {chunk_count}");
                        self.in_air_size -= size;
                        if frame.chunks.is_empty() {
                            log::debug!("[ConnectionBuffer] outgoing frame {frame_id} finished");
                            self.outgoings.remove(&frame_id);
                        }
                    } else {
                        log::warn!("[ConnectionBuffer] frame {frame_id} chunk {chunk_id} not found");
                    }
                } else {
                    log::warn!("[ConnectionBuffer] frame {frame_id} not found");
                }
            }
        }
        Some(())
    }

    pub fn pop_send(&mut self) -> Option<Vec<u8>> {
        let first = self.high_priority.front().or_else(|| self.low_priority.front())?;
        let first_size = first.in_air_size();
        if first_size + self.in_air_size <= AIR_LIMIT {
            let out = self.high_priority.pop_front().or_else(|| self.low_priority.pop_front()).expect("should have some");
            assert_eq!(out.in_air_size(), first_size);
            self.in_air_size += first_size;
            match &out {
                ConnectionChunk::Chunk { frame_id, chunk_id, chunk_count, .. } => {
                    log::debug!("[ConnectionBuffer] sending frame {frame_id} chunk {chunk_id}/{chunk_count} size {first_size}");
                }
                ConnectionChunk::ChunkAck { frame_id, chunk_id, chunk_count } => {
                    log::debug!("[ConnectionBuffer] sending ack fro frame {frame_id} chunk {chunk_id}/{chunk_count}");
                }
            }
            Some(bincode::serialize(&out).expect("should serialize"))
        } else {
            None
        }
    }

    pub fn pop_recv(&mut self) -> Option<Vec<u8>> {
        self.outs.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHUNK_SIZE: usize = 1024;
    const AIR_LIMIT: usize = 4096;

    #[test]
    fn buffer_small() {
        let mut buffer = ConnectionBuffer::<CHUNK_SIZE, AIR_LIMIT>::default();
        let small_data = vec![1, 2, 3, 4, 5];

        buffer.push_frame(&small_data);

        // Check if the data is sent immediately
        let sent = buffer.pop_send().unwrap();
        let chunk: ConnectionChunk = bincode::deserialize(&sent).unwrap();

        let expected_chunk = ConnectionChunk::Chunk {
            frame_id: 0,
            chunk_id: 0,
            chunk_count: 1,
            data: small_data.clone(),
        };
        assert_eq!(chunk, expected_chunk);

        // Simulate receiving an ACK
        let ack = ConnectionChunk::ChunkAck {
            frame_id: 0,
            chunk_id: 0,
            chunk_count: 1,
        };
        let ack_data = bincode::serialize(&ack).unwrap();
        buffer.on_received(&ack_data);

        // Check if the outgoing frame is removed
        assert!(buffer.outgoings.is_empty());
        assert_eq!(buffer.in_air_size, 0);
    }

    #[test]
    fn buffer_big() {
        let mut buffer = ConnectionBuffer::<CHUNK_SIZE, AIR_LIMIT>::default();
        let big_data = vec![42; CHUNK_SIZE * 2 + 100]; // 2 full chunks + 1 partial

        buffer.push_frame(&big_data);

        // Check if the data is sent in chunks
        for i in 0..3 {
            let sent = buffer.pop_send().unwrap();
            let chunk: ConnectionChunk = bincode::deserialize(&sent).unwrap();

            let expected_chunk = ConnectionChunk::Chunk {
                frame_id: 0,
                chunk_id: i as u16,
                chunk_count: 3,
                data: big_data[i * CHUNK_SIZE..std::cmp::min((i + 1) * CHUNK_SIZE, big_data.len())].to_vec(),
            };
            assert_eq!(chunk, expected_chunk);
        }
        assert_eq!(buffer.pop_send(), None);

        // Simulate receiving ACKs
        for i in 0..3 {
            let ack = ConnectionChunk::ChunkAck {
                frame_id: 0,
                chunk_id: i,
                chunk_count: 3,
            };
            let ack_data = bincode::serialize(&ack).unwrap();
            buffer.on_received(&ack_data);
        }

        // Check if the outgoing frame is removed after all ACKs
        assert!(buffer.outgoings.is_empty());
        assert_eq!(buffer.in_air_size, 0);
    }

    #[test]
    fn buffer_hybrid() {
        let mut buffer = ConnectionBuffer::<CHUNK_SIZE, AIR_LIMIT>::default();
        let big_data = vec![42; CHUNK_SIZE * 2];
        let small_data = vec![1, 2, 3, 4, 5];

        buffer.push_frame(&big_data);
        buffer.push_frame(&small_data);

        // Check if small data is sent first
        let sent = buffer.pop_send().unwrap();
        let chunk: ConnectionChunk = bincode::deserialize(&sent).unwrap();

        let expected_chunk = ConnectionChunk::Chunk {
            frame_id: 1, // Second frame (small data) should be sent first
            chunk_id: 0,
            chunk_count: 1,
            data: small_data.clone(),
        };
        assert_eq!(chunk, expected_chunk);

        // Now check if big data chunks are sent
        for i in 0..2 {
            let sent = buffer.pop_send().unwrap();
            let chunk: ConnectionChunk = bincode::deserialize(&sent).unwrap();

            let expected_chunk = ConnectionChunk::Chunk {
                frame_id: 0, // First frame (big data)
                chunk_id: i as u16,
                chunk_count: 2,
                data: big_data[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE].to_vec(),
            };
            assert_eq!(chunk, expected_chunk);
        }
    }

    #[test]
    fn wait_on_air() {
        let mut buffer = ConnectionBuffer::<CHUNK_SIZE, AIR_LIMIT>::default();
        let data = vec![42; AIR_LIMIT + 1]; // Slightly more than AIR_LIMIT

        buffer.push_frame(&data);

        // Send chunks until AIR_LIMIT is reached
        let mut sent_count = 0;
        while let Some(sent) = buffer.pop_send() {
            sent_count += 1;
            let chunk: ConnectionChunk = bincode::deserialize(&sent).unwrap();
            if let ConnectionChunk::Chunk { frame_id, chunk_id, chunk_count, .. } = chunk {
                // Simulate receiving an ACK for all but the last chunk
                if chunk_id < chunk_count - 1 {
                    let ack = ConnectionChunk::ChunkAck {
                        frame_id,
                        chunk_id: chunk_id,
                        chunk_count: chunk_count,
                    };
                    let ack_data = bincode::serialize(&ack).unwrap();
                    buffer.on_received(&ack_data);
                }
            }
        }

        // Check that we can't send more until we receive the last ACK
        assert!(buffer.pop_send().is_none());

        // Send the last ACK
        let last_ack = ConnectionChunk::ChunkAck {
            frame_id: 0,
            chunk_id: sent_count as u16 - 1,
            chunk_count: sent_count as u16,
        };
        let last_ack_data = bincode::serialize(&last_ack).unwrap();
        buffer.on_received(&last_ack_data);

        // Now we should be able to send again if there's more data
        buffer.push_frame(&[1, 2, 3]);
        assert!(buffer.pop_send().is_some());
    }

    #[test]
    fn test_receiving_chunks() {
        let mut buffer = ConnectionBuffer::<CHUNK_SIZE, AIR_LIMIT>::default();
        let data = vec![42; CHUNK_SIZE * 2 + 100]; // 2 full chunks + 1 partial

        // Simulate receiving chunks
        for i in 0..3 {
            let chunk_len = if i < 2 {
                CHUNK_SIZE
            } else {
                100
            };
            let chunk = ConnectionChunk::Chunk {
                frame_id: 0,
                chunk_id: i,
                chunk_count: 3,
                data: data[0..chunk_len].to_vec(),
            };
            let chunk_data = bincode::serialize(&chunk).unwrap();
            buffer.on_received(&chunk_data);
        }

        // Check if the full data is reconstructed
        let received = buffer.pop_recv().unwrap();
        assert_eq!(received, data);

        // Check if ACKs were queued
        for i in 0..3 {
            let ack = buffer.pop_send().unwrap();
            let chunk: ConnectionChunk = bincode::deserialize(&ack).unwrap();
            let expected_ack = ConnectionChunk::ChunkAck {
                frame_id: 0,
                chunk_id: i,
                chunk_count: 3,
            };
            assert_eq!(chunk, expected_ack);
        }
    }
}
