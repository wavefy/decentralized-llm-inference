use std::collections::HashMap;

use p2p_network::addr::NodeId;
use protocol::worker::event::{RpcReq, RpcRes};
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

#[derive(Clone)]
pub struct RpcClientTx {
    tx: Sender<(NodeId, String, Vec<u8>, oneshot::Sender<RpcRes>)>,
}

impl RpcClientTx {
    pub async fn request<REQ: prost::Message, RES: prost::Message + Default>(&self, dest: NodeId, cmd: &str, req: REQ) -> Result<RES, String> {
        let (tx, rx) = oneshot::channel();
        let mut buf = Vec::new();
        req.encode(&mut buf).expect("Should encode to buffer");
        self.tx.send((dest, cmd.to_owned(), buf, tx)).await.expect("Should send to main");
        match rx.await {
            Ok(res) => {
                if res.success {
                    match RES::decode(res.payload.as_slice()) {
                        Ok(res) => Ok(res),
                        Err(_) => Err("DECODE_ERROR".to_string()),
                    }
                } else {
                    Err("RPC_ERROR".to_string())
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct RpcClientRx {
    seq_seed: u32,
    waits: HashMap<u32, oneshot::Sender<RpcRes>>,
    rx: Receiver<(NodeId, String, Vec<u8>, oneshot::Sender<RpcRes>)>,
}

impl RpcClientRx {
    pub fn on_res(&mut self, res: RpcRes) {
        if let Some(tx) = self.waits.remove(&res.seq) {
            tx.send(res).expect("Should send res to waits");
        }
    }

    pub async fn recv(&mut self) -> Option<(NodeId, RpcReq)> {
        let (dest, cmd, payload, tx) = self.rx.recv().await?;
        let seq = self.seq_seed + 1;
        self.seq_seed += 1;
        self.waits.insert(seq, tx);
        Some((dest, RpcReq { seq, cmd, payload }))
    }
}

pub fn create_rpc() -> (RpcClientTx, RpcClientRx) {
    let (tx, rx) = channel(10);
    (
        RpcClientTx { tx },
        RpcClientRx {
            rx,
            seq_seed: 0,
            waits: HashMap::new(),
        },
    )
}
