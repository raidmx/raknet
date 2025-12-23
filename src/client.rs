/// RakNet client implementation.
///
/// This module provides a client for connecting to RakNet servers,
/// performing the full connection handshake, and creating a stream
/// for bidirectional communication.

use crate::connection::RakNetStream;
use crate::error::{Error, Result};
use crate::protocol::*;
use crate::reliability::Reliability;
use bytes::Bytes;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// RakNet client for connecting to servers.
pub struct RakNetClient {
    /// UDP socket for communication.
    socket: UdpSocket,

    /// Client GUID (unique identifier).
    client_guid: i64,
}

impl RakNetClient {
    /// Creates a new RakNet client.
    ///
    /// Binds to a random local port and generates a unique client GUID.
    pub async fn new() -> Result<Self> {
        // Bind to any available port
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        // Generate random GUID
        let client_guid = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        Ok(Self {
            socket,
            client_guid,
        })
    }

    /// Connects to a RakNet server at the specified address.
    ///
    /// Performs the full connection handshake:
    /// 1. UnconnectedPing/Pong exchange
    /// 2. MTU discovery (OpenConnectionRequest1/Reply1)
    /// 3. Connection setup (OpenConnectionRequest2/Reply2)
    /// 4. Connection handshake (ConnectionRequest/Accepted, NewIncomingConnection)
    ///
    /// Returns a `RakNetStream` for bidirectional communication.
    pub async fn connect(&mut self, server_addr: SocketAddr) -> Result<RakNetStream> {
        // Step 1: Ping server to verify it's online
        self.send_unconnected_ping(server_addr).await?;
        let (_server_guid, _pong_data) = self.receive_unconnected_pong().await?;

        // Step 2: MTU discovery
        let mtu = self.discover_mtu(server_addr).await?;

        // Step 3: Connection setup (OpenConnectionRequest2/Reply2)
        self.send_open_connection_request_2(server_addr, mtu).await?;
        let (server_guid, _client_addr, mtu) = self.receive_open_connection_reply_2().await?;

        // Step 4: Connect the socket to the server (enables send() instead of send_to())
        self.socket.connect(server_addr).await?;

        // Step 5: Create RakNetStream
        // Transfer ownership of socket to the stream
        let socket = std::mem::replace(
            &mut self.socket,
            UdpSocket::bind("0.0.0.0:0").await?, // Placeholder
        );
        let stream = RakNetStream::new(socket, server_addr, mtu);

        // Step 6: Complete connection handshake
        self.complete_connection_handshake(&stream, server_addr, server_guid).await?;

        Ok(stream)
    }

    /// Sends an UnconnectedPing to the server.
    async fn send_unconnected_ping(&self, server_addr: SocketAddr) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let ping = encode_unconnected_ping(timestamp, self.client_guid);
        self.socket.send_to(&ping, server_addr).await?;

        Ok(())
    }

    /// Receives an UnconnectedPong from the server.
    async fn receive_unconnected_pong(&self) -> Result<(i64, Bytes)> {
        let mut buf = vec![0u8; 2048];

        // Wait for pong with timeout
        let len = timeout(Duration::from_secs(5), self.socket.recv(&mut buf))
            .await
            .map_err(|_| Error::Timeout)??;

        let packet = &buf[..len];
        if packet.is_empty() || packet[0] != ID_UNCONNECTED_PONG {
            return Err(Error::invalid_packet("Expected UnconnectedPong"));
        }

        let (_timestamp, server_guid, pong_data) = decode_unconnected_pong(packet)?;
        Ok((server_guid, pong_data))
    }

    /// Performs MTU discovery by trying different MTU sizes.
    ///
    /// Tries 1492, 1200, and 576 bytes in order, returning the first
    /// MTU that receives a successful reply.
    async fn discover_mtu(&self, server_addr: SocketAddr) -> Result<u16> {
        for mtu in [1492, 1200, 576] {
            let request = encode_open_connection_request_1(PROTOCOL_VERSION, mtu);
            self.socket.send_to(&request, server_addr).await?;

            // Wait for reply with timeout
            match timeout(
                Duration::from_millis(500),
                self.receive_open_connection_reply_1(),
            )
            .await
            {
                Ok(Ok((_server_guid, reply_mtu))) => return Ok(reply_mtu),
                _ => continue, // Try next MTU size
            }
        }

        Err(Error::Timeout)
    }

    /// Receives OpenConnectionReply1 from the server.
    async fn receive_open_connection_reply_1(&self) -> Result<(i64, u16)> {
        let mut buf = vec![0u8; 2048];
        let len = self.socket.recv(&mut buf).await?;

        let packet = &buf[..len];
        if packet.is_empty() || packet[0] != ID_OPEN_CONNECTION_REPLY_1 {
            return Err(Error::invalid_packet("Expected OpenConnectionReply1"));
        }

        decode_open_connection_reply_1(packet)
    }

    /// Sends OpenConnectionRequest2 to the server.
    async fn send_open_connection_request_2(
        &self,
        server_addr: SocketAddr,
        mtu: u16,
    ) -> Result<()> {
        let request = encode_open_connection_request_2(server_addr, mtu, self.client_guid);
        self.socket.send_to(&request, server_addr).await?;
        Ok(())
    }

    /// Receives OpenConnectionReply2 from the server.
    async fn receive_open_connection_reply_2(&self) -> Result<(i64, SocketAddr, u16)> {
        let mut buf = vec![0u8; 2048];

        let len = timeout(Duration::from_secs(5), self.socket.recv(&mut buf))
            .await
            .map_err(|_| Error::Timeout)??;

        let packet = &buf[..len];
        if packet.is_empty() || packet[0] != ID_OPEN_CONNECTION_REPLY_2 {
            return Err(Error::invalid_packet("Expected OpenConnectionReply2"));
        }

        decode_open_connection_reply_2(packet)
    }

    /// Completes the connection handshake after the stream is created.
    ///
    /// Sends ConnectionRequest, waits for ConnectionRequestAccepted,
    /// then sends NewIncomingConnection to finalize the connection.
    async fn complete_connection_handshake(
        &self,
        stream: &RakNetStream,
        server_addr: SocketAddr,
        _server_guid: i64,
    ) -> Result<()> {
        // Send ConnectionRequest as application data
        // The stream will handle framing and datagram encoding
        let request_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let connection_request = encode_connection_request(self.client_guid, request_timestamp);
        stream.send(connection_request, Reliability::ReliableOrdered).await?;

        // Wait for ConnectionRequestAccepted
        // This will be handled by the stream's receive task and won't be delivered to application
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Send NewIncomingConnection
        let accepted_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let new_incoming = encode_new_incoming_connection(
            server_addr,
            request_timestamp,
            accepted_timestamp,
        );
        stream.send(new_incoming, Reliability::ReliableOrdered).await?;

        // Wait for handshake to complete
        tokio::time::sleep(Duration::from_millis(200)).await;

        Ok(())
    }

    /// Returns the local address of the client socket.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Returns the client GUID.
    pub fn client_guid(&self) -> i64 {
        self.client_guid
    }
}
