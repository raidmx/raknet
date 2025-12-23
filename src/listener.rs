use crate::connection::RakNetStream;
use crate::error::{Error, Result};
use crate::protocol::*;
use crate::socket;
use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};

/// RakNet server listener.
///
/// This listens for incoming RakNet connections and handles the handshake process.
/// Once a connection is established, it creates a session socket and spawns a connection task.
pub struct RakNetListener {
    /// The underlying UDP socket.
    socket: Arc<UdpSocket>,

    /// Local address the listener is bound to.
    local_addr: SocketAddr,

    /// Server GUID (randomly generated).
    server_guid: i64,

    /// Pong data sent in response to pings (server info, MOTD, etc.).
    pong_data: Bytes,

    /// Channel for accepting new connections.
    accept_rx: mpsc::UnboundedReceiver<RakNetStream>,

    /// Channel sender for new connections (cloned for handshake task).
    accept_tx: mpsc::UnboundedSender<RakNetStream>,

    /// Active connections (for tracking).
    connections: Arc<RwLock<HashMap<SocketAddr, ()>>>,
}

impl RakNetListener {
    /// Binds a new RakNet listener to the specified address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to bind to (e.g., "0.0.0.0:19132")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use raknet::RakNetListener;
    ///
    /// #[tokio::main]
    /// async fn main() -> raknet::Result<()> {
    ///     let listener = RakNetListener::bind("0.0.0.0:19132").await?;
    ///     println!("Listening on {}", listener.local_addr()?);
    ///     Ok(())
    /// }
    /// ```
    pub async fn bind<A: tokio::net::ToSocketAddrs>(addr: A) -> Result<Self> {
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(addr)
            .await?
            .collect();

        let addr = addrs.first().ok_or(Error::InvalidAddress)?;

        let socket = socket::create_tokio_listener(*addr).await?;
        let local_addr = socket.local_addr()?;

        // Generate random server GUID
        let server_guid = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Default pong data (Minecraft Bedrock Edition format)
        // Format: MCPE;server_name;protocol_version;game_version;online_players;max_players;server_guid;world_name;gamemode
        let pong_data = format!(
            "MCPE;Rust RakNet Server;{};1.20.0;0;10;{};RakNet;Survival",
            PROTOCOL_VERSION,
            server_guid
        );

        let (accept_tx, accept_rx) = mpsc::unbounded_channel();

        Ok(Self {
            socket: Arc::new(socket),
            local_addr,
            server_guid,
            pong_data: Bytes::from(pong_data),
            accept_rx,
            accept_tx,
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Returns the local address the listener is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.local_addr)
    }

    /// Sets the pong data sent in response to unconnected pings.
    ///
    /// This is typically used to set server information (MOTD, player count, etc.).
    pub fn set_pong_data(&mut self, data: impl Into<Bytes>) {
        self.pong_data = data.into();
    }

    /// Accepts a new incoming connection.
    ///
    /// This method blocks until a new connection is ready or an error occurs.
    /// Use this in a loop to handle multiple connections.
    pub async fn accept(&mut self) -> Result<RakNetStream> {
        self.accept_rx.recv().await.ok_or(Error::ListenerClosed)
    }

    /// Runs the listener loop, handling incoming unconnected packets.
    ///
    /// This spawns a background task that handles the handshake for new connections.
    /// Use `accept()` to retrieve established connections.
    pub fn run(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];

            loop {
                match self.socket.recv_from(&mut buf).await {
                    Ok((len, remote_addr)) => {
                        let packet = &buf[..len];
                        if packet.is_empty() {
                            continue;
                        }

                        let packet_id = packet[0];

                        match packet_id {
                            ID_UNCONNECTED_PING => {
                                let _ = self.handle_unconnected_ping(packet, remote_addr).await;
                            }
                            ID_OPEN_CONNECTION_REQUEST_1 => {
                                let _ = self.handle_open_connection_request_1(packet, remote_addr).await;
                            }
                            ID_OPEN_CONNECTION_REQUEST_2 => {
                                let listener = self.clone();
                                let packet = packet.to_vec();
                                // Spawn handshake task to avoid blocking listener
                                tokio::spawn(async move {
                                    let _ = listener.handle_open_connection_request_2(&packet, remote_addr).await;
                                });
                            }
                            _ => {
                                // Unknown or unhandled packet type
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Listener error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// Handles an unconnected ping packet.
    async fn handle_unconnected_ping(&self, packet: &[u8], remote_addr: SocketAddr) -> Result<()> {
        match decode_unconnected_ping(packet) {
            Ok((timestamp, _client_guid)) => {
                // Send pong response
                let pong = encode_unconnected_pong(
                    timestamp,
                    self.server_guid,
                    &self.pong_data,
                );

                self.socket.send_to(&pong, remote_addr).await?;
            }
            Err(e) => {
                // Log error but continue serving other clients
                eprintln!("Invalid unconnected ping from {}: {}", remote_addr, e);
            }
        }

        Ok(())
    }

    /// Handles an open connection request 1 packet (MTU discovery).
    async fn handle_open_connection_request_1(&self, packet: &[u8], remote_addr: SocketAddr) -> Result<()> {
        match decode_open_connection_request_1(packet) {
            Ok((protocol_version, mtu)) => {
                // Check protocol version
                if protocol_version != PROTOCOL_VERSION {
                    // Send incompatible protocol version packet
                    let mut response = vec![0u8; 1 + 16 + 1 + 8];
                    response[0] = ID_INCOMPATIBLE_PROTOCOL_VERSION;
                    write_magic(&mut response, 1);
                    response[17] = PROTOCOL_VERSION;
                    // Server GUID (8 bytes)
                    response[18..26].copy_from_slice(&self.server_guid.to_be_bytes());

                    self.socket.send_to(&response, remote_addr).await?;
                    return Ok(());
                }

                // MTU should be reasonable (between 576 and 1500)
                let mtu = mtu.clamp(576, 1500);

                // Send reply
                let reply = encode_open_connection_reply_1(self.server_guid, mtu);
                self.socket.send_to(&reply, remote_addr).await?;
            }
            Err(e) => {
                eprintln!("Invalid open connection request 1 from {}: {}", remote_addr, e);
            }
        }

        Ok(())
    }

    /// Handles an open connection request 2 packet and completes the handshake.
    ///
    /// This creates a new session socket with SO_REUSEPORT on the same port,
    /// establishes the connection, and passes the stream to the accept channel.
    async fn handle_open_connection_request_2(&self, packet: &[u8], remote_addr: SocketAddr) -> Result<()> {
        // Check if already connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(&remote_addr) {
                // Send already connected packet
                let response = Bytes::from_static(&[ID_ALREADY_CONNECTED]);
                self.socket.send_to(&response, remote_addr).await?;
                return Ok(());
            }
        }

        // Decode request
        let (_server_addr, mtu, _client_guid) = match decode_open_connection_request_2(packet) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Invalid open connection request 2 from {}: {}", remote_addr, e);
                return Ok(());
            }
        };

        // MTU should be reasonable
        let mtu = mtu.clamp(576, 1500);

        // Create session socket with SO_REUSEPORT on the same port
        let session_socket = match socket::create_tokio_session(self.local_addr, remote_addr).await {
            Ok(sock) => sock,
            Err(e) => {
                eprintln!("Failed to create session socket for {}: {}", remote_addr, e);
                return Ok(());
            }
        };

        // Send reply from session socket (this establishes the 4-tuple routing)
        let reply = encode_open_connection_reply_2(self.server_guid, remote_addr, mtu);
        session_socket.send(&reply).await?;

        // Create RakNetStream for this connection
        let stream = RakNetStream::new(session_socket, remote_addr, mtu);

        // Connection starts in Connecting state
        // Will be marked as Connected after receiving NewIncomingConnection packet
        // (part of the full RakNet connection handshake)

        // Add to active connections
        {
            let mut connections = self.connections.write().await;
            connections.insert(remote_addr, ());
        }

        // Send to accept channel
        if let Err(e) = self.accept_tx.send(stream) {
            eprintln!("Failed to send stream to accept channel: {}", e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_listener_bind() {
        let listener = RakNetListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn test_set_pong_data() {
        let mut listener = RakNetListener::bind("127.0.0.1:0").await.unwrap();
        listener.set_pong_data("Custom MOTD");
        assert_eq!(&listener.pong_data[..], b"Custom MOTD");
    }
}
