/// Clean debug server using internal RakNet connection handling
///
/// The listener internally handles:
/// 1. Unconnected Ping/Pong (server discovery)
/// 2. Open Connection Request 1 / Reply 1 (MTU negotiation)
/// 3. Open Connection Request 2 / Reply 2 (connection establishment)
///
/// We just call accept() to get established connections.

use raknet::{RakNetListener, Reliability};
use bytes::Bytes;
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:19132";

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║         RakNet Debug Server - Clean Implementation        ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Create listener
    let listener = RakNetListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    let server_guid = listener.server_guid();

    println!("✓ Server Configuration:");
    println!("  ├─ Address: {}", local_addr);
    println!("  ├─ GUID: {}", server_guid);

    // Set MOTD for Minecraft Bedrock Edition
    let motd = format!("MCPE;Dedicated Server;898;1.21.130;0;11;{};Bedrock level;Survival;1;19132;19132;0;", server_guid);
    println!("  └─ MOTD: {}\n", motd);
    listener.set_pong_data(Bytes::from(motd)).await;

    // Wrap in Arc and start listener background task
    let listener = Arc::new(listener);

    println!("🚀 Starting listener (handles all RakNet protocol internally)...\n");
    listener.clone().run();

    println!("Waiting for connections...\n");
    println!("════════════════════════════════════════════════════════════\n");

    let mut connection_count = 0u32;

    // Accept loop - listener handles all protocol internally
    loop {
        match listener.accept().await {
            Ok(mut stream) => {
                connection_count += 1;
                let conn_id = connection_count;
                let remote = stream.remote_addr();
                let mtu = stream.mtu();
                let start_time = Instant::now();

                println!("╔════════════════════════════════════════════════════════════╗");
                println!("║  ✅ CONNECTION ESTABLISHED #{:<2}                           ║", conn_id);
                println!("╚════════════════════════════════════════════════════════════╝");
                println!("  ├─ Remote: {}", remote);
                println!("  ├─ MTU: {} bytes", mtu);
                println!("  └─ Established in: {:?}\n", start_time.elapsed());

                // Spawn task to handle this connection
                tokio::spawn(async move {
                    println!("[Connection #{}] 📨 Receiving packets...\n", conn_id);

                    let mut packet_count = 0u32;

                    while let Some(data) = stream.recv().await {
                        packet_count += 1;

                        println!("┌─ [Connection #{}] Packet #{} ─────────────────────", conn_id, packet_count);
                        println!("│  Size: {} bytes", data.len());
                        println!("│  Time: {:?}", start_time.elapsed());

                        // Show first bytes
                        let preview_len = data.len().min(16);
                        print!("│  Data: ");
                        for byte in &data[..preview_len] {
                            print!("{:02x} ", byte);
                        }
                        if data.len() > 16 {
                            print!("... (+{} bytes)", data.len() - 16);
                        }
                        println!();

                        // Identify Minecraft packet if possible
                        if !data.is_empty() {
                            let packet_id = data[0];
                            let packet_name = match packet_id {
                                0x01 => "Login",
                                0x02 => "Play Status",
                                0x05 => "Server To Client Handshake",
                                0x06 => "Client To Server Handshake",
                                0x07 => "Disconnect",
                                _ => "Unknown",
                            };
                            println!("│  Minecraft: 0x{:02x} ({})", packet_id, packet_name);
                        }

                        // Echo back
                        match stream.send(data.clone(), Reliability::ReliableOrdered).await {
                            Ok(_) => println!("│  ✓ Echoed back"),
                            Err(e) => {
                                println!("│  ✗ Send error: {}", e);
                                break;
                            }
                        }
                        println!("└────────────────────────────────────────────────\n");
                    }

                    // Connection closed
                    println!("\n╔════════════════════════════════════════════════════════════╗");
                    println!("║  🔴 CONNECTION CLOSED #{:<2}                                ║", conn_id);
                    println!("╚════════════════════════════════════════════════════════════╝");
                    println!("  Packets received: {}", packet_count);
                    println!("  Duration: {:?}\n", start_time.elapsed());
                });
            }
            Err(e) => {
                eprintln!("❌ Accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
