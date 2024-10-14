use std::io::ErrorKind;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

pub struct ProtobufStream {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl ProtobufStream {
    pub fn new(stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        Self { stream }
    }

    pub async fn write<M: prost::Message>(&mut self, m: &M) -> Result<(), std::io::Error> {
        let mut buf = Vec::with_capacity(m.encoded_len());
        m.encode(&mut buf).expect("Should convert to buf");
        self.stream.send(Message::Binary(buf)).await.map_err(|e| std::io::Error::new(ErrorKind::BrokenPipe, e))
    }

    pub async fn read<M: prost::Message + Default>(&mut self) -> Option<Result<M, std::io::Error>> {
        let msg = self.stream.next().await?;
        match msg {
            Ok(Message::Binary(buf)) => Some(M::decode(buf.as_slice()).map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))),
            Ok(_) => Some(Err(std::io::Error::new(ErrorKind::InvalidData, "OnlySupportBinary"))),
            Err(e) => Some(Err(std::io::Error::new(ErrorKind::BrokenPipe, e))),
        }
    }

    pub async fn shutdown(&mut self) {
        let _ = self.stream.close(None).await;
    }
}
