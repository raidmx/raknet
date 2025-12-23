/// Connection handling for established RakNet connections.
///
/// This module provides the RakNetStream which manages an active connection,
/// handling packet sending/receiving, reliability, ordering, and periodic maintenance.

use crate::error::{Error, Result};
use crate::protocol::*;
use crate::reliability::*;
use crate::state::SharedState;
use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::interval;

/// A RakNet connection stream.
///
/// This represents an established connection with a remote peer.
/// It handles all reliability layer logic, sending/receiving frames,
/// and provides a simple API for sending/receiving application data.
pub struct RakNetStream {
    /// The UDP socket for this connection (connected to remote).
    socket: Arc<UdpSocket>,

    /// Remote peer address.
    remote_addr: SocketAddr,

    /// Shared connection state.
    state: Arc<SharedState>,

    /// Channel for sending application data to the connection.
    send_tx: mpsc::UnboundedSender<Bytes>,

    /// Channel for receiving application data from the connection.
    recv_rx: mpsc::UnboundedReceiver<Bytes>,
}

impl RakNetStream {
    /// Creates a new RakNetStream with the given socket and MTU.
    ///
    /// This spawns the receive and tick tasks that handle the connection.
    pub fn new(socket: UdpSocket, remote_addr: SocketAddr, mtu: u16) -> Self {
        let socket = Arc::new(socket);
        let state = SharedState::new(mtu);

        // Channels for application data
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        let (recv_tx, recv_rx) = mpsc::unbounded_channel();

        // Spawn receive task
        tokio::spawn(receive_task(
            socket.clone(),
            state.clone(),
            recv_tx,
        ));

        // Spawn send task
        tokio::spawn(send_task(
            socket.clone(),
            state.clone(),
            send_rx,
        ));

        // Spawn tick task
        tokio::spawn(tick_task(socket.clone(), state.clone()));

        Self {
            socket,
            remote_addr,
            state,
            send_tx,
            recv_rx,
        }
    }

    /// Returns the local address of this connection.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Returns the remote address of this connection.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Returns the current MTU size.
    pub fn mtu(&self) -> u16 {
        self.state.mtu()
    }

    /// Checks if the connection is still active.
    pub fn is_connected(&self) -> bool {
        self.state.is_connected()
    }

    /// Sends application data to the remote peer with the specified reliability.
    ///
    /// Data will be automatically fragmented if it exceeds the MTU.
    pub async fn send(&self, data: Bytes, _reliability: Reliability) -> Result<()> {
        if !self.state.is_connected() {
            return Err(Error::ConnectionClosed);
        }

        // For now, send directly through the channel
        // The send task will handle fragmentation and reliability
        self.send_tx.send(data)
            .map_err(|_| Error::ConnectionClosed)?;

        Ok(())
    }

    /// Receives application data from the remote peer.
    ///
    /// Returns `None` if the connection is closed.
    pub async fn recv(&mut self) -> Option<Bytes> {
        self.recv_rx.recv().await
    }

    /// Marks the connection as fully connected (after handshake).
    pub(crate) fn mark_connected(&self) {
        self.state.mark_connected();
    }

    /// Gracefully closes the connection.
    pub async fn close(&self) -> Result<()> {
        if self.state.mark_disconnecting() {
            // Send disconnect notification
            let disconnect = encode_disconnect_notification();
            self.socket.send(&disconnect).await?;
        }
        self.state.mark_disconnected();
        Ok(())
    }
}

/// Receive task - processes incoming packets.
async fn receive_task(
    socket: Arc<UdpSocket>,
    state: Arc<SharedState>,
    app_data_tx: mpsc::UnboundedSender<Bytes>,
) {
    let mut buf = vec![0u8; 2048];

    loop {
        // Check if connection is closed
        if state.is_closed() {
            break;
        }

        match socket.recv(&mut buf).await {
            Ok(len) => {
                let packet = &buf[..len];
                if let Err(e) = handle_packet(packet, &state, &app_data_tx, &socket).await {
                    eprintln!("Error handling packet: {}", e);
                }
                state.metrics.record_recv(len);
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
                break;
            }
        }
    }

    state.mark_disconnected();
}

/// Handles an incoming packet.
async fn handle_packet(
    packet: &[u8],
    state: &Arc<SharedState>,
    app_data_tx: &mpsc::UnboundedSender<Bytes>,
    socket: &UdpSocket,
) -> Result<()> {
    if packet.is_empty() {
        return Ok(());
    }

    let packet_id = packet[0];

    match packet_id {
        // Connected ping/pong
        ID_CONNECTED_PING => {
            handle_connected_ping(packet, state, &socket).await
        }

        ID_CONNECTED_PONG => {
            handle_connected_pong(packet, state).await
        }

        ID_DISCONNECT_NOTIFICATION => {
            decode_disconnect_notification(packet)?;
            state.mark_disconnected();
            Ok(())
        }

        // Datagram packet (0x80-0x8f)
        id if is_datagram(id) => {
            let (seq, frames_data) = decode_datagram(packet)?;
            handle_datagram(seq, frames_data, state, app_data_tx, socket).await
        }

        // ACK packet
        id if is_ack(id) => {
            let (_is_ack, ranges) = decode_ack_nack(packet)?;
            handle_ack(ranges, state).await
        }

        // NACK packet
        id if is_nack(id) => {
            let (_is_nack, ranges) = decode_ack_nack(packet)?;
            handle_nack(ranges, state).await
        }

        _ => {
            // Unknown packet type
            Ok(())
        }
    }
}

/// Handles a datagram packet containing frames.
async fn handle_datagram(
    seq: u24,
    frames_data: Bytes,
    state: &Arc<SharedState>,
    app_data_tx: &mpsc::UnboundedSender<Bytes>,
    socket: &UdpSocket,
) -> Result<()> {
    // Check for duplicate
    let is_new = {
        let mut window = state.recv_window.lock();
        match window.mark_received(seq.get()) {
            Some(true) => true,
            Some(false) => return Ok(()), // Duplicate
            None => return Ok(()), // Too old
        }
    };

    if !is_new {
        return Ok(());
    }

    // Add to ACK list
    {
        let mut acks = state.pending_acks.lock();
        acks.insert(seq);
    }

    // Decode frames
    let mut buf = &frames_data[..];
    while !buf.is_empty() {
        let frame = Frame::decode(&mut buf)?;
        handle_frame(frame, state, app_data_tx, socket).await?;
    }

    Ok(())
}

/// Handles a single frame.
async fn handle_frame(
    frame: Frame,
    state: &Arc<SharedState>,
    app_data_tx: &mpsc::UnboundedSender<Bytes>,
    socket: &UdpSocket,
) -> Result<()> {
    // Check if this is a protocol packet (not application data)
    if !frame.payload.is_empty() {
        let packet_id = frame.payload[0];
        match packet_id {
            ID_CONNECTION_REQUEST => {
                return handle_connection_request(&frame.payload, socket, state).await;
            }
            ID_NEW_INCOMING_CONNECTION => {
                return handle_new_incoming_connection(&frame.payload, state).await;
            }
            _ => {} // Not a protocol packet, continue to handle as application data
        }
    }

    // Handle based on reliability level
    match frame.reliability {
        Reliability::Unreliable | Reliability::UnreliableSequenced => {
            // Deliver immediately
            deliver_frame_payload(frame, state, app_data_tx);
        }

        Reliability::Reliable | Reliability::ReliableOrdered | Reliability::ReliableSequenced => {
            // For reliable packets, check ordering if needed
            if frame.reliability.is_ordered() {
                // Check if fragmented
                if frame.split.is_some() {
                    // Fragment reassembly first
                    deliver_frame_payload(frame, state, app_data_tx);
                } else {
                    // Use ordered channel
                    let channel = frame.order_channel as usize;
                    let mut ordered_chan = state.ordered_channels[channel].lock();

                    let payloads = ordered_chan.insert(
                        frame.order_index.unwrap_or(u24::new(0)).get(),
                        frame.payload
                    );

                    // Deliver all ready packets in order
                    for payload in payloads {
                        let _ = app_data_tx.send(payload);
                    }
                }
            } else {
                // Deliver immediately for non-ordered reliable
                deliver_frame_payload(frame, state, app_data_tx);
            }
        }
    }

    Ok(())
}

/// Delivers a frame's payload to the application.
///
/// Handles fragment reassembly for split packets.
fn deliver_frame_payload(
    frame: Frame,
    state: &Arc<SharedState>,
    app_data_tx: &mpsc::UnboundedSender<Bytes>,
) {
    // Check if frame is split (fragmented)
    if let Some(split_info) = frame.split {
        // Fragment reassembly
        let mut fragment_queue = state.fragment_queue.lock();

        if let Some(reassembled) = fragment_queue.insert(
            split_info.id,
            split_info.index,
            split_info.count,
            frame.payload,
        ) {
            // All fragments received - deliver complete packet
            let _ = app_data_tx.send(reassembled);
        }
        // Otherwise wait for more fragments
    } else {
        // Not fragmented - deliver immediately
        let _ = app_data_tx.send(frame.payload);
    }
}

/// Handles ACK ranges.
async fn handle_ack(ranges: AckRangeList, state: &Arc<SharedState>) -> Result<()> {
    let mut queue = state.send_queue.lock();

    for range in ranges.ranges() {
        match range {
            AckRange::Single(seq) => {
                queue.acknowledge(seq.get());
            }
            AckRange::Range { start, end } => {
                queue.acknowledge_range(start.get(), end.get());
            }
        }
    }

    Ok(())
}

/// Handles NACK ranges.
async fn handle_nack(_ranges: AckRangeList, _state: &Arc<SharedState>) -> Result<()> {
    // TODO: Immediately retransmit NACKed packets
    // For now, the tick task will handle retransmission

    Ok(())
}

/// Handles a ConnectedPing packet and sends a pong response.
async fn handle_connected_ping(
    packet: &[u8],
    _state: &Arc<SharedState>,
    socket: &UdpSocket,
) -> Result<()> {
    let ping_timestamp = decode_connected_ping(packet)?;

    let pong_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let pong = encode_connected_pong(ping_timestamp, pong_timestamp);
    socket.send(&pong).await?;

    Ok(())
}

/// Handles a ConnectedPong packet and updates RTT metrics.
async fn handle_connected_pong(packet: &[u8], state: &Arc<SharedState>) -> Result<()> {
    let (ping_timestamp, _pong_timestamp) = decode_connected_pong(packet)?;

    // Look up when we sent the ping
    if let Some(send_time) = state.pending_pings.lock().remove(&ping_timestamp) {
        let rtt = send_time.elapsed();
        state.metrics.update_rtt(rtt);
    }

    Ok(())
}

/// Handles a ConnectionRequest packet from the client.
///
/// This is sent by the client after receiving OpenConnectionReply2.
/// Server responds with ConnectionRequestAccepted.
async fn handle_connection_request(
    packet: &[u8],
    socket: &UdpSocket,
    _state: &Arc<SharedState>,
) -> Result<()> {
    let (_client_guid, request_timestamp) = decode_connection_request(packet)?;

    // Get current timestamp for the accepted packet
    let accepted_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    // Get the remote address from the socket
    let remote_addr = socket.peer_addr()?;

    // Send ConnectionRequestAccepted
    let accepted = encode_connection_request_accepted(
        remote_addr,
        request_timestamp,
        accepted_timestamp,
    );

    socket.send(&accepted).await?;

    Ok(())
}

/// Handles a NewIncomingConnection packet from the client.
///
/// This is the final step of the connection handshake.
/// Marks the connection as fully established.
async fn handle_new_incoming_connection(
    packet: &[u8],
    state: &Arc<SharedState>,
) -> Result<()> {
    let (_server_addr, _request_ts, _accepted_ts) = decode_new_incoming_connection(packet)?;

    // Mark connection as fully established
    state.mark_connected();

    Ok(())
}

/// Send task - handles outgoing application data.
async fn send_task(
    socket: Arc<UdpSocket>,
    state: Arc<SharedState>,
    mut app_data_rx: mpsc::UnboundedReceiver<Bytes>,
) {
    while let Some(data) = app_data_rx.recv().await {
        if state.is_closed() {
            break;
        }

        // Check if data needs fragmentation
        let mtu = state.mtu() as usize;
        let max_single_frame_size = mtu - 60; // Account for headers (datagram + frame + IP/UDP)

        if data.len() <= max_single_frame_size {
            // Send as single frame
            if let Err(e) = send_single_frame(data, &socket, &state).await {
                eprintln!("Error sending frame: {}", e);
                break;
            }
        } else {
            // Fragment and send multiple frames
            if let Err(e) = send_fragmented(data, &socket, &state).await {
                eprintln!("Error sending fragmented packet: {}", e);
                break;
            }
        }
    }
}

/// Sends a single unfragmented frame.
async fn send_single_frame(
    data: Bytes,
    socket: &UdpSocket,
    state: &Arc<SharedState>,
) -> Result<()> {
    let reliability = Reliability::ReliableOrdered;
    let message_index = state.next_message_index();
    let order_index = state.next_order_index(0);

    let frame = Frame::new(reliability, data)
        .with_message_index(message_index)
        .with_order(order_index, 0);

    // Encode frame
    let mut frame_buf = BytesMut::new();
    frame.encode(&mut frame_buf);
    let encoded_frame = frame_buf.freeze();

    // Create datagram
    let seq = state.next_send_seq();
    let datagram = encode_datagram(seq, &[encoded_frame]);

    // Send datagram
    socket.send(&datagram).await?;
    state.metrics.record_send(datagram.len());

    // Add to send queue for tracking
    let mut queue = state.send_queue.lock();
    queue.insert(seq.get(), datagram);

    Ok(())
}

/// Sends a fragmented packet as multiple frames.
async fn send_fragmented(
    data: Bytes,
    socket: &UdpSocket,
    state: &Arc<SharedState>,
) -> Result<()> {
    let split_id = state.next_split_id();
    let mtu = state.mtu() as usize;

    // Calculate chunk size (MTU - headers - split info overhead)
    let chunk_size = mtu - 80; // Conservative estimate for all headers

    // Split data into chunks
    let total_count = ((data.len() + chunk_size - 1) / chunk_size) as u32;

    for (index, chunk_start) in (0..data.len()).step_by(chunk_size).enumerate() {
        let chunk_end = (chunk_start + chunk_size).min(data.len());
        let chunk = data.slice(chunk_start..chunk_end);

        let split_info = SplitInfo {
            count: total_count,
            id: split_id,
            index: index as u32,
        };

        let reliability = Reliability::ReliableOrdered;
        let message_index = state.next_message_index();
        let order_index = state.next_order_index(0);

        let frame = Frame::new(reliability, chunk)
            .with_message_index(message_index)
            .with_order(order_index, 0)
            .with_split(split_info);

        // Encode frame
        let mut frame_buf = BytesMut::new();
        frame.encode(&mut frame_buf);
        let encoded_frame = frame_buf.freeze();

        // Create datagram
        let seq = state.next_send_seq();
        let datagram = encode_datagram(seq, &[encoded_frame]);

        // Send datagram
        socket.send(&datagram).await?;
        state.metrics.record_send(datagram.len());

        // Add to send queue for tracking
        {
            let mut queue = state.send_queue.lock();
            queue.insert(seq.get(), datagram.clone());
        } // Lock dropped here

        // Small delay between fragments to avoid overwhelming receiver
        tokio::time::sleep(Duration::from_micros(100)).await;
    }

    Ok(())
}

/// Tick task - periodic maintenance (ACK flush, retransmission, keepalive).
async fn tick_task(socket: Arc<UdpSocket>, state: Arc<SharedState>) {
    let mut interval = interval(Duration::from_millis(100));
    let mut ping_counter = 0u32;

    loop {
        interval.tick().await;

        if state.is_closed() {
            break;
        }

        ping_counter += 1;

        // Check for timeout
        if state.is_timed_out() {
            state.mark_disconnected();
            break;
        }

        // Flush pending ACKs
        if let Err(e) = flush_acks(&socket, &state).await {
            eprintln!("Error flushing ACKs: {}", e);
        }

        // Flush pending NACKs
        if let Err(e) = flush_nacks(&socket, &state).await {
            eprintln!("Error flushing NACKs: {}", e);
        }

        // Check for retransmissions (every 300ms = 3 ticks)
        if ping_counter % 3 == 0 {
            if let Err(e) = check_retransmissions(&socket, &state).await {
                eprintln!("Error checking retransmissions: {}", e);
            }
        }

        // Cleanup expired fragments (every 2 seconds = 20 ticks)
        if ping_counter % 20 == 0 {
            let cleaned = state.fragment_queue.lock().cleanup_expired();
            if cleaned > 0 {
                eprintln!("Cleaned up {} expired fragment entries", cleaned);
            }
        }

        // Send keepalive ping (every 5 seconds = 50 ticks)
        if ping_counter % 50 == 0 && state.is_connected() {
            if let Err(e) = send_keepalive_ping(&socket, &state).await {
                eprintln!("Error sending keepalive: {}", e);
            }
        }
    }
}

/// Flushes pending ACKs to the remote peer.
async fn flush_acks(socket: &UdpSocket, state: &Arc<SharedState>) -> Result<()> {
    let ack_list = {
        let mut acks = state.pending_acks.lock();
        if acks.ranges().is_empty() {
            return Ok(());
        }
        std::mem::replace(&mut *acks, AckRangeList::new())
    };

    let ack_packet = encode_ack(&ack_list);
    socket.send(&ack_packet).await?;
    state.metrics.record_send(ack_packet.len());

    Ok(())
}

/// Flushes pending NACKs to the remote peer.
async fn flush_nacks(socket: &UdpSocket, state: &Arc<SharedState>) -> Result<()> {
    let nack_list = {
        let mut nacks = state.pending_nacks.lock();
        if nacks.ranges().is_empty() {
            return Ok(());
        }
        std::mem::replace(&mut *nacks, AckRangeList::new())
    };

    let nack_packet = encode_nack(&nack_list);
    socket.send(&nack_packet).await?;
    state.metrics.record_send(nack_packet.len());

    Ok(())
}

/// Checks for packets that need retransmission.
async fn check_retransmissions(socket: &UdpSocket, state: &Arc<SharedState>) -> Result<()> {
    let rtt = state.metrics.rtt();
    let timeout = rtt * 3; // 3x RTT

    let packets_to_retransmit: Vec<Bytes> = {
        let mut queue = state.send_queue.lock();
        queue.get_expired(timeout, 3)
            .into_iter()
            .map(|(_seq, packet)| packet)
            .collect()
    };

    for packet in packets_to_retransmit {
        socket.send(&packet).await?;
        state.metrics.record_send(packet.len());
    }

    Ok(())
}

/// Sends a keepalive ping to the remote peer.
async fn send_keepalive_ping(socket: &UdpSocket, state: &Arc<SharedState>) -> Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let send_time = Instant::now();

    // Track ping for RTT calculation
    state.pending_pings.lock().insert(timestamp, send_time);

    let ping = encode_connected_ping(timestamp);
    socket.send(&ping).await?;
    state.metrics.record_send(ping.len());

    Ok(())
}

impl Drop for RakNetStream {
    fn drop(&mut self) {
        // Mark connection as closed when dropped
        self.state.mark_disconnected();
    }
}
