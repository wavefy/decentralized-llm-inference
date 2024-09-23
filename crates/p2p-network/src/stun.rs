use std::net::SocketAddr;

use tokio::net::UdpSocket;

pub async fn send_stun_request(udp_client: &UdpSocket, stun_servers: &[SocketAddr]) -> Result<(), tokio::io::Error> {
    // Create a binding message
    let binding_msg = stun_coder::StunMessage::create_request()
        .add_attribute(stun_coder::StunAttribute::Software {
            description: String::from("rust-stun-coder"),
        }) // Add software attribute
        .add_message_integrity() // Add message integrity attribute
        .add_fingerprint(); // Add fingerprint attribute

    let integrity_pass = "STUN_CODER_PASS"; // Integrity password to use
                                            // Encode the binding_msg
    let bytes = binding_msg.encode(Some(integrity_pass)).expect("Should encode");

    // Send the message
    for stun_server in stun_servers.iter() {
        if stun_server.is_ipv4() == udp_client.local_addr()?.is_ipv4() {
            udp_client.send_to(&bytes, *stun_server).await?;
        }
    }
    Ok(())
}

pub fn process_stun_response(buf: &[u8]) -> Result<SocketAddr, String> {
    let stun_response = stun_coder::StunMessage::decode(buf, Some("STUN_CODER_PASS")).map_err(|e| e.to_string())?;
    for attr in stun_response.get_attributes() {
        if let stun_coder::StunAttribute::XorMappedAddress { socket_addr } = attr {
            return Ok(*socket_addr);
        }
    }

    Err("No XorMappedAddress found in STUN response".to_string())
}

pub async fn get_public_ip(udp_client: &UdpSocket, stun_servers: &[SocketAddr]) -> Result<SocketAddr, String> {
    send_stun_request(udp_client, stun_servers).await.map_err(|e| e.to_string())?;
    let mut buf = [0; 1024];
    let buf_len = udp_client.recv(&mut buf).await.map_err(|e| e.to_string())?;
    process_stun_response(&buf[..buf_len])
}
