# Usage Examples

Comprehensive examples for using the RakNet library.

## Table of Contents

- [Basic Server](#basic-server)
- [Basic Client](#basic-client)
- [Echo Server](#echo-server)
- [Chat Server](#chat-server)
- [File Transfer](#file-transfer)
- [Custom Reliability](#custom-reliability)
- [Connection Management](#connection-management)
- [Error Handling](#error-handling)

## Basic Server

Minimal server that accepts connections:

```rust
use raknet::{RakNetListener, Reliability};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create listener
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);

    // Set server information (shown in pong)
    listener.set_pong_data(bytes::Bytes::from("My Server v1.0"));

    // Start accepting connections
    listener.clone().run();

    println!("Server listening on 0.0.0.0:19132");

    // Accept connections
    while let Ok(stream) = listener.accept().await {
        println!("New connection from {}", stream.remote_addr());

        tokio::spawn(async move {
            // Handle connection...
        });
    }

    Ok(())
}
```

## Basic Client

Minimal client that connects to a server:

```rust
use raknet::RakNetClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let mut client = RakNetClient::new().await?;

    // Connect to server
    let stream = client.connect("127.0.0.1:19132".parse()?).await?;

    println!("Connected! MTU: {}", stream.mtu());

    Ok(())
}
```

## Echo Server

Server that echoes received data back to clients:

```rust
use raknet::{RakNetListener, Reliability};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
    listener.clone().run();

    println!("Echo server started on port 19132");

    while let Ok(mut stream) = listener.accept().await {
        tokio::spawn(async move {
            let addr = stream.remote_addr();
            println!("[{}] Connected", addr);

            while let Some(data) = stream.recv().await {
                println!("[{}] Received {} bytes", addr, data.len());

                // Echo back
                if let Err(e) = stream.send(data, Reliability::ReliableOrdered).await {
                    eprintln!("[{}] Send error: {}", addr, e);
                    break;
                }
            }

            println!("[{}] Disconnected", addr);
        });
    }

    Ok(())
}
```

## Chat Server

Multi-client chat server with broadcast:

```rust
use raknet::{RakNetListener, Reliability};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use std::collections::HashMap;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
    listener.clone().run();

    // Broadcast channel for messages
    let (tx, _rx) = broadcast::channel(100);

    // Track connected clients
    let clients = Arc::new(RwLock::new(HashMap::new()));

    println!("Chat server started");

    while let Ok(mut stream) = listener.accept().await {
        let addr = stream.remote_addr();
        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let clients = clients.clone();

        // Add client
        clients.write().await.insert(addr, ());

        tokio::spawn(async move {
            println!("[{}] joined", addr);

            // Broadcast task
            let stream_clone = stream.clone();
            let broadcast_task = tokio::spawn(async move {
                while let Ok((sender, msg)) = rx.recv().await {
                    if sender != addr {
                        let _ = stream_clone.send(msg, Reliability::ReliableOrdered).await;
                    }
                }
            });

            // Receive task
            while let Some(data) = stream.recv().await {
                let message = String::from_utf8_lossy(&data);
                println!("[{}] {}", addr, message);

                // Broadcast to all
                let broadcast_msg = format!("[{}] {}", addr, message);
                let _ = tx.send((addr, Bytes::from(broadcast_msg)));
            }

            // Cleanup
            println!("[{}] left", addr);
            clients.write().await.remove(&addr);
            broadcast_task.abort();
        });
    }

    Ok(())
}
```

## File Transfer

Server that receives files:

```rust
use raknet::{RakNetListener, Reliability};
use bytes::Bytes;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
    listener.clone().run();

    while let Ok(mut stream) = listener.accept().await {
        tokio::spawn(async move {
            // First packet contains filename
            if let Some(filename_bytes) = stream.recv().await {
                let filename = String::from_utf8_lossy(&filename_bytes);
                println!("Receiving file: {}", filename);

                let mut file = File::create(filename.as_ref()).await?;
                let mut total_bytes = 0;

                // Receive file chunks
                while let Some(chunk) = stream.recv().await {
                    file.write_all(&chunk).await?;
                    total_bytes += chunk.len();

                    if chunk.is_empty() {
                        break; // End marker
                    }
                }

                file.flush().await?;
                println!("File received: {} bytes", total_bytes);

                // Send confirmation
                stream.send(
                    Bytes::from("OK"),
                    Reliability::ReliableOrdered
                ).await?;
            }

            Ok::<_, Box<dyn std::error::Error>>(())
        });
    }

    Ok(())
}
```

Client that sends file:

```rust
use raknet::{RakNetClient, Reliability};
use bytes::Bytes;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RakNetClient::new().await?;
    let stream = client.connect("127.0.0.1:19132".parse()?).await?;

    // Send filename
    stream.send(
        Bytes::from("test.dat"),
        Reliability::ReliableOrdered
    ).await?;

    // Open and send file
    let mut file = File::open("test.dat").await?;
    let mut buffer = vec![0u8; 8192];
    let mut total_sent = 0;

    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        stream.send(
            Bytes::copy_from_slice(&buffer[..n]),
            Reliability::ReliableOrdered
        ).await?;

        total_sent += n;
        println!("Sent {} bytes", total_sent);
    }

    // Send end marker
    stream.send(Bytes::new(), Reliability::ReliableOrdered).await?;

    // Wait for confirmation
    if let Some(response) = stream.recv().await {
        println!("Server response: {:?}", response);
    }

    Ok(())
}
```

## Custom Reliability

Using different reliability levels for different data types:

```rust
use raknet::{RakNetStream, Reliability};
use bytes::Bytes;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
enum GamePacket {
    PlayerPosition { x: f32, y: f32, z: f32 },
    ChatMessage { text: String },
    PlayerJoined { name: String },
    ItemPickup { item_id: u32 },
}

async fn send_game_packet(
    stream: &RakNetStream,
    packet: GamePacket
) -> Result<(), Box<dyn std::error::Error>> {
    let data = bincode::serialize(&packet)?;
    let bytes = Bytes::from(data);

    let reliability = match packet {
        // Position updates: latest only, can drop old
        GamePacket::PlayerPosition { .. } => Reliability::UnreliableSequenced,

        // Chat: must arrive, must be in order
        GamePacket::ChatMessage { .. } => Reliability::ReliableOrdered,

        // Important events: must arrive, order matters
        GamePacket::PlayerJoined { .. } => Reliability::ReliableOrdered,

        // Pickups: must arrive, order doesn't matter
        GamePacket::ItemPickup { .. } => Reliability::Reliable,
    };

    stream.send(bytes, reliability).await?;
    Ok(())
}
```

## Connection Management

Server with connection limits and tracking:

```rust
use raknet::{RakNetListener, ConnectionState};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

struct ConnectionInfo {
    connected_at: Instant,
    bytes_sent: u64,
    bytes_received: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = Arc::new(RakNetListener::bind("0.0.0.0:19132").await?);
    listener.clone().run();

    let connections = Arc::new(RwLock::new(HashMap::new()));
    let max_connections = 100;

    while let Ok(mut stream) = listener.accept().await {
        // Check connection limit
        if connections.read().await.len() >= max_connections {
            println!("Server full, rejecting connection");
            stream.close().await?;
            continue;
        }

        let addr = stream.remote_addr();
        let connections = connections.clone();

        // Track connection
        connections.write().await.insert(addr, ConnectionInfo {
            connected_at: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
        });

        tokio::spawn(async move {
            println!("[{}] Connected", addr);

            while let Some(data) = stream.recv().await {
                // Update stats
                if let Some(info) = connections.write().await.get_mut(&addr) {
                    info.bytes_received += data.len() as u64;
                }

                // Handle data...
            }

            // Remove on disconnect
            connections.write().await.remove(&addr);
            println!("[{}] Disconnected", addr);
        });
    }

    Ok(())
}
```

## Error Handling

Comprehensive error handling:

```rust
use raknet::{RakNetClient, Reliability, Error};
use bytes::Bytes;

#[tokio::main]
async fn main() {
    // Client creation
    let mut client = match RakNetClient::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            return;
        }
    };

    // Connection with retry
    let mut stream = loop {
        match client.connect("127.0.0.1:19132".parse().unwrap()).await {
            Ok(s) => break s,
            Err(Error::Timeout) => {
                eprintln!("Connection timeout, retrying...");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            },
            Err(Error::ConnectionRefused(reason)) => {
                eprintln!("Connection refused: {}", reason);
                return;
            },
            Err(e) => {
                eprintln!("Connection error: {}", e);
                return;
            }
        }
    };

    println!("Connected!");

    // Sending with error handling
    let message = Bytes::from("Hello, Server!");
    match stream.send(message, Reliability::ReliableOrdered).await {
        Ok(()) => println!("Message sent"),
        Err(Error::ConnectionClosed) => {
            eprintln!("Connection closed");
            return;
        },
        Err(Error::SendQueueFull) => {
            eprintln!("Send queue full, waiting...");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            // Retry send...
        },
        Err(e) => {
            eprintln!("Send error: {}", e);
            return;
        }
    }

    // Receiving with timeout
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        stream.recv()
    ).await {
        Ok(Some(data)) => {
            println!("Received: {:?}", data);
        },
        Ok(None) => {
            println!("Connection closed");
        },
        Err(_) => {
            eprintln!("Receive timeout");
        }
    }

    // Graceful shutdown
    if let Err(e) = stream.close().await {
        eprintln!("Error closing connection: {}", e);
    }
}
```

## Advanced: Custom Protocol

Building a simple RPC system on top of RakNet:

```rust
use raknet::{RakNetStream, Reliability};
use bytes::Bytes;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
enum Request {
    GetPlayerInfo { player_id: u32 },
    SetPlayerPosition { player_id: u32, x: f32, y: f32, z: f32 },
}

#[derive(Serialize, Deserialize)]
enum Response {
    PlayerInfo { name: String, level: u32 },
    Success,
    Error { message: String },
}

async fn rpc_call(
    stream: &RakNetStream,
    request: Request
) -> Result<Response, Box<dyn std::error::Error>> {
    // Send request
    let request_data = bincode::serialize(&request)?;
    stream.send(Bytes::from(request_data), Reliability::ReliableOrdered).await?;

    // Wait for response
    let response_data = stream.recv().await
        .ok_or("Connection closed")?;

    let response: Response = bincode::deserialize(&response_data)?;
    Ok(response)
}

// Usage
async fn example(stream: &RakNetStream) {
    match rpc_call(stream, Request::GetPlayerInfo { player_id: 123 }).await {
        Ok(Response::PlayerInfo { name, level }) => {
            println!("Player: {} (Level {})", name, level);
        },
        Ok(Response::Error { message }) => {
            eprintln!("RPC error: {}", message);
        },
        Err(e) => {
            eprintln!("Call failed: {}", e);
        },
        _ => {}
    }
}
```

---

For more examples, see the `examples/` directory in the repository.
