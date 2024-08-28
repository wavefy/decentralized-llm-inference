use std::{collections::VecDeque, net::SocketAddr, time::Instant};
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
    Disconnected,
}

pub struct RemoteConn {
    remote: NodeId,
    rtc: Rtc,
    pending: Option<SdpPendingOffer>,
    queue: VecDeque<RemoteConnOut>,
    channel: Option<ChannelId>,
    timeout: Option<Instant>,
}

impl RemoteConn {
    pub fn new(remote: NodeId, local_addrs: Vec<SocketAddr>) -> (Self, String) {
        let rtc_config = RtcConfig::new().set_local_ice_credentials(IceCreds::new());
        let ice_ufrag = rtc_config.local_ice_credentials().as_ref().expect("should have ice credentials").ufrag.clone();
        let mut rtc = rtc_config.build();

        for addr in local_addrs {
            rtc.add_local_candidate(Candidate::host(addr, Protocol::Udp).expect("Should create candidate"));
        }

        (
            Self {
                remote,
                rtc,
                pending: None,
                queue: VecDeque::new(),
                channel: None,
                timeout: None,
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
                                if self.channel.take().is_some() {
                                    self.queue.push_back(RemoteConnOut::Disconnected);
                                }
                            }
                        }
                        str0m::Event::ChannelOpen(channel, name) => {
                            log::info!("[RemoteConn {:?}] opened channel {name}", self.remote);
                            self.queue.push_back(RemoteConnOut::Connected);
                            self.channel = Some(channel);
                        }
                        str0m::Event::ChannelData(data) => {
                            log::info!("[RemoteConn {:?}] on channel data {}", self.remote, data.data.len());
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
}
