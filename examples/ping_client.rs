use bytes::Bytes;
use raknet::protocol::*;
use raknet::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logs
    tracing_subscriber::fmt::init();

    let server_addr = "127.0.0.1:19132";
    let client_guid = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    println!("Sending ping to {}", server_addr);

    // Create UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(server_addr).await?;

    // Send unconnected ping
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let ping = encode_unconnected_ping(timestamp, client_guid);
    socket.send(&ping).await?;

    println!("Ping sent, waiting for pong...");

    // Wait for pong
    let mut buf = vec![0u8; 2048];
    let len = socket.recv(&mut buf).await?;

    let (recv_timestamp, server_guid, pong_data) = decode_unconnected_pong(&buf[..len])?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let latency_ms = now - recv_timestamp;

    println!();
    println!("Received pong from server!");
    println!("  Server GUID: {}", server_guid);
    println!("  Latency: {} ms", latency_ms);
    println!("  Server info: {}", String::from_utf8_lossy(&pong_data));
    println!();

    Ok(())
}
