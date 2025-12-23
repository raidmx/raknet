/// Full RakNet server example demonstrating connection handling.
///
/// This example shows how to:
/// - Create a listener
/// - Accept incoming connections
/// - Send and receive data on established connections

use raknet::{RakNetListener, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<()> {
    // Create listener
    let mut listener = RakNetListener::bind("0.0.0.0:19132").await?;
    println!("RakNet server listening on {}", listener.local_addr()?);

    // Set custom pong data (MOTD)
    listener.set_pong_data(
        "MCPE;Rust RakNet Server;568;1.20.0;0;10;13253860892328930865;Bedrock;Survival;1;19132;19133;"
    );

    // Start listener in background
    Arc::new(listener).run();

    // Accept connections in a loop
    println!("Waiting for connections...");

    loop {
        // Accept new connection
        match timeout(Duration::from_secs(60), listener.accept()).await {
            Ok(Ok(mut stream)) => {
                println!(
                    "New connection from {} (MTU: {})",
                    stream.remote_addr(),
                    stream.mtu()
                );

                // Spawn task to handle this connection
                tokio::spawn(async move {
                    handle_connection(&mut stream).await;
                });
            }
            Ok(Err(e)) => {
                eprintln!("Error accepting connection: {}", e);
                break;
            }
            Err(_) => {
                println!("No connections in the last 60 seconds...");
                continue;
            }
        }
    }

    Ok(())
}

/// Handles a single connection.
async fn handle_connection(stream: &mut raknet::RakNetStream) {
    println!("Handling connection from {}", stream.remote_addr());

    // Receive packets from this connection
    while let Some(data) = stream.recv().await {
        println!(
            "Received {} bytes from {}: {:?}",
            data.len(),
            stream.remote_addr(),
            &data[..data.len().min(16)]
        );

        // Echo back
        if let Err(e) = stream
            .send(data, raknet::Reliability::ReliableOrdered)
            .await
        {
            eprintln!("Error sending: {}", e);
            break;
        }
    }

    println!("Connection from {} closed", stream.remote_addr());
}
