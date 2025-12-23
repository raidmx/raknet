/// Simple RakNet client to test our server implementation
///
/// This sends the standard RakNet handshake sequence to verify
/// the server correctly handles Open Connection Request packets.

use raknet::protocol::*;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "127.0.0.1:19132";

    println!("\n═══════════════════════════════════════════");
    println!("  RakNet Test Client");
    println!("═══════════════════════════════════════════\n");

    // Create UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let local_addr = socket.local_addr()?;
    println!("✓ Client socket: {}", local_addr);
    println!("✓ Server: {}\n", server_addr);

    // Step 1: Send Unconnected Ping
    println!("1️⃣  Sending Unconnected Ping...");
    let ping = encode_unconnected_ping(1000, 123456789);
    socket.send_to(&ping, server_addr).await?;

    // Receive Unconnected Pong
    let mut buf = vec![0u8; 2048];
    let (len, _) = socket.recv_from(&mut buf).await?;
    match decode_unconnected_pong(&buf[..len]) {
        Ok((timestamp, server_guid, pong_data)) => {
            println!("   ✓ Received Unconnected Pong");
            println!("     ├─ Server GUID: {}", server_guid);
            println!("     ├─ Timestamp: {}", timestamp);
            println!("     └─ Data: {}\n", String::from_utf8_lossy(&pong_data));
        }
        Err(e) => {
            println!("   ✗ Failed to decode pong: {}\n", e);
            return Ok(());
        }
    }

    // Step 2: Send Open Connection Request 1
    println!("2️⃣  Sending Open Connection Request 1...");
    let mtu = 1400;
    let req1 = encode_open_connection_request_1(PROTOCOL_VERSION, mtu);
    socket.send_to(&req1, server_addr).await?;
    println!("   ├─ Protocol version: {}", PROTOCOL_VERSION);
    println!("   └─ MTU: {} bytes", mtu);

    // Receive Open Connection Reply 1
    let (len, _) = socket.recv_from(&mut buf).await?;
    let packet_id = buf[0];

    if packet_id == ID_INCOMPATIBLE_PROTOCOL_VERSION {
        println!("   ✗ Server rejected: Incompatible protocol version\n");
        return Ok(());
    }

    match decode_open_connection_reply_1(&buf[..len]) {
        Ok((server_guid, server_mtu)) => {
            println!("   ✓ Received Open Connection Reply 1");
            println!("     ├─ Server GUID: {}", server_guid);
            println!("     └─ MTU: {} bytes\n", server_mtu);
        }
        Err(e) => {
            println!("   ✗ Failed to decode reply: {}\n", e);
            return Ok(());
        }
    }

    // Step 3: Send Open Connection Request 2
    println!("3️⃣  Sending Open Connection Request 2...");
    let client_guid = 987654321i64;
    let req2 = encode_open_connection_request_2(
        server_addr.parse()?,
        mtu,
        client_guid,
    );
    socket.send_to(&req2, server_addr).await?;
    println!("   ├─ Client GUID: {}", client_guid);
    println!("   └─ MTU: {} bytes", mtu);

    // Receive Open Connection Reply 2
    let (len, _) = socket.recv_from(&mut buf).await?;
    match decode_open_connection_reply_2(&buf[..len]) {
        Ok((server_guid, client_addr, server_mtu)) => {
            println!("   ✓ Received Open Connection Reply 2");
            println!("     ├─ Server GUID: {}", server_guid);
            println!("     ├─ Client address: {}", client_addr);
            println!("     └─ MTU: {} bytes\n", server_mtu);

            println!("═══════════════════════════════════════════");
            println!("  ✅ RakNet Handshake Successful!");
            println!("═══════════════════════════════════════════\n");
        }
        Err(e) => {
            println!("   ✗ Failed to decode reply: {}\n", e);
            return Ok(());
        }
    }

    Ok(())
}
