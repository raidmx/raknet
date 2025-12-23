/// RakNet packet IDs for the protocol.
///
/// These are the first byte of each packet and determine how the packet should be processed.

// === Unconnected packets (before connection is established) ===

/// Unconnected ping packet.
/// Client sends this to discover servers and measure latency.
pub const ID_UNCONNECTED_PING: u8 = 0x01;

/// Unconnected pong packet.
/// Server responds to ping with this, includes server GUID and optional pong data.
pub const ID_UNCONNECTED_PONG: u8 = 0x1c;

/// Open connection request 1.
/// First step of connection handshake, includes protocol version and MTU padding.
pub const ID_OPEN_CONNECTION_REQUEST_1: u8 = 0x05;

/// Open connection reply 1.
/// Server response to request 1, confirms protocol version and MTU.
pub const ID_OPEN_CONNECTION_REPLY_1: u8 = 0x06;

/// Open connection request 2.
/// Second step of handshake, includes client GUID and security cookie.
pub const ID_OPEN_CONNECTION_REQUEST_2: u8 = 0x07;

/// Open connection reply 2.
/// Server response to request 2, confirms connection and provides server GUID.
pub const ID_OPEN_CONNECTION_REPLY_2: u8 = 0x08;

// === Connected packets (after connection is established) ===

/// Connection request packet.
/// Sent by client after handshake to request final connection.
pub const ID_CONNECTION_REQUEST: u8 = 0x09;

/// Connection request accepted packet.
/// Server accepts the connection request.
pub const ID_CONNECTION_REQUEST_ACCEPTED: u8 = 0x10;

/// New incoming connection packet.
/// Client confirms the accepted connection.
pub const ID_NEW_INCOMING_CONNECTION: u8 = 0x13;

/// Disconnect notification.
/// Gracefully closes the connection.
pub const ID_DISCONNECT_NOTIFICATION: u8 = 0x15;

/// Incompatible protocol version.
/// Server rejects connection due to protocol version mismatch.
pub const ID_INCOMPATIBLE_PROTOCOL_VERSION: u8 = 0x19;

/// Already connected.
/// Server rejects connection because client is already connected.
pub const ID_ALREADY_CONNECTED: u8 = 0x12;

/// No free incoming connections.
/// Server rejects connection because it's at capacity.
pub const ID_NO_FREE_INCOMING_CONNECTIONS: u8 = 0x14;

/// Connection banned.
/// Server rejects connection because the IP is banned.
pub const ID_CONNECTION_BANNED: u8 = 0x17;

/// Connected ping.
/// Keep-alive ping sent during an active connection.
pub const ID_CONNECTED_PING: u8 = 0x00;

/// Connected pong.
/// Response to connected ping.
pub const ID_CONNECTED_PONG: u8 = 0x03;

// === Datagram packets (reliability layer) ===

/// Frame set packet (datagram).
/// Contains one or more frames with reliability/ordering info.
/// Identified by flags: 0x80 | 0x04 (valid flag).
pub const ID_FRAME_SET: u8 = 0x80;

/// ACK packet.
/// Acknowledges received datagrams.
pub const ID_ACK: u8 = 0xc0;

/// NACK packet.
/// Negative acknowledgment for missing datagrams.
pub const ID_NACK: u8 = 0xa0;

// === Bitflags for datagram packets ===

/// Datagram packet flag (bit 7).
pub const BIT_FLAG_DATAGRAM: u8 = 0x80;

/// Valid datagram flag (bit 2).
/// Always set on datagram packets to distinguish from other packet types.
pub const BIT_FLAG_VALID: u8 = 0x04;

/// ACK packet flag (bit 6).
pub const BIT_FLAG_ACK: u8 = 0x40;

/// NACK packet flag (bit 5).
pub const BIT_FLAG_NACK: u8 = 0x20;

/// Checks if a packet is a datagram (frame set).
#[inline]
pub fn is_datagram(packet_id: u8) -> bool {
    (packet_id & BIT_FLAG_DATAGRAM) != 0
}

/// Checks if a packet is an ACK.
#[inline]
pub fn is_ack(packet_id: u8) -> bool {
    (packet_id & BIT_FLAG_ACK) != 0 && (packet_id & BIT_FLAG_DATAGRAM) == 0
}

/// Checks if a packet is a NACK.
#[inline]
pub fn is_nack(packet_id: u8) -> bool {
    (packet_id & BIT_FLAG_NACK) != 0 && (packet_id & BIT_FLAG_DATAGRAM) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_id_constants() {
        assert_eq!(ID_UNCONNECTED_PING, 0x01);
        assert_eq!(ID_UNCONNECTED_PONG, 0x1c);
        assert_eq!(ID_FRAME_SET, 0x80);
        assert_eq!(ID_ACK, 0xc0);
        assert_eq!(ID_NACK, 0xa0);
    }

    #[test]
    fn test_is_datagram() {
        assert!(is_datagram(0x84)); // 0x80 | 0x04
        assert!(is_datagram(0x80));
        assert!(!is_datagram(0x01));
        assert!(!is_datagram(0xc0)); // ACK
        assert!(!is_datagram(0xa0)); // NACK
    }

    #[test]
    fn test_is_ack() {
        assert!(is_ack(0xc0));
        assert!(!is_ack(0x80)); // Datagram
        assert!(!is_ack(0xa0)); // NACK
        assert!(!is_ack(0x01));
    }

    #[test]
    fn test_is_nack() {
        assert!(is_nack(0xa0));
        assert!(!is_nack(0x80)); // Datagram
        assert!(!is_nack(0xc0)); // ACK
        assert!(!is_nack(0x01));
    }
}
