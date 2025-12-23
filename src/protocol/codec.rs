use crate::error::{Error, Result};
use crate::protocol::{check_magic, MAGIC};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::net::SocketAddr;

/// Encodes an unconnected ping packet.
///
/// Format:
/// - u8: Packet ID (0x01)
/// - i64: Client timestamp
/// - [u8; 16]: Magic
/// - i64: Client GUID
pub fn encode_unconnected_ping(timestamp: i64, client_guid: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 8 + 16 + 8);
    buf.put_u8(crate::protocol::ID_UNCONNECTED_PING);
    buf.put_i64(timestamp);
    buf.put_slice(&MAGIC);
    buf.put_i64(client_guid);
    buf.freeze()
}

/// Decodes an unconnected ping packet.
///
/// Returns (timestamp, client_guid).
pub fn decode_unconnected_ping(data: &[u8]) -> Result<(i64, i64)> {
    if data.len() < 1 + 8 + 16 + 8 {
        return Err(Error::invalid_packet("UnconnectedPing too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_UNCONNECTED_PING {
        return Err(Error::invalid_packet("Not an UnconnectedPing packet"));
    }

    let timestamp = buf.get_i64();

    if !check_magic(&data, 9) {
        return Err(Error::InvalidMagic);
    }
    buf.advance(16);

    let client_guid = buf.get_i64();

    Ok((timestamp, client_guid))
}

/// Encodes an unconnected pong packet.
///
/// Format:
/// - u8: Packet ID (0x1c)
/// - i64: Client timestamp (echoed back)
/// - i64: Server GUID
/// - [u8; 16]: Magic
/// - u16: Pong data length
/// - [u8]: Pong data (server info, MOTD, etc.)
pub fn encode_unconnected_pong(
    timestamp: i64,
    server_guid: i64,
    pong_data: &[u8],
) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 8 + 8 + 16 + 2 + pong_data.len());
    buf.put_u8(crate::protocol::ID_UNCONNECTED_PONG);
    buf.put_i64(timestamp);
    buf.put_i64(server_guid);
    buf.put_slice(&MAGIC);
    buf.put_u16(pong_data.len() as u16);
    buf.put_slice(pong_data);
    buf.freeze()
}

/// Decodes an unconnected pong packet.
///
/// Returns (timestamp, server_guid, pong_data).
pub fn decode_unconnected_pong(data: &[u8]) -> Result<(i64, i64, Bytes)> {
    if data.len() < 1 + 8 + 8 + 16 + 2 {
        return Err(Error::invalid_packet("UnconnectedPong too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_UNCONNECTED_PONG {
        return Err(Error::invalid_packet("Not an UnconnectedPong packet"));
    }

    let timestamp = buf.get_i64();
    let server_guid = buf.get_i64();

    if !check_magic(&data, 17) {
        return Err(Error::InvalidMagic);
    }
    buf.advance(16);

    let pong_data_len = buf.get_u16() as usize;
    if buf.remaining() < pong_data_len {
        return Err(Error::invalid_packet("UnconnectedPong pong data truncated"));
    }

    let pong_data = Bytes::copy_from_slice(&buf[..pong_data_len]);

    Ok((timestamp, server_guid, pong_data))
}

/// Encodes an open connection request 1 packet.
///
/// Format:
/// - u8: Packet ID (0x05)
/// - [u8; 16]: Magic
/// - u8: Protocol version
/// - [u8]: MTU padding (filled with 0x00 up to desired MTU)
pub fn encode_open_connection_request_1(protocol_version: u8, mtu: u16) -> Bytes {
    let total_size = mtu as usize;
    let mut buf = BytesMut::with_capacity(total_size);
    buf.put_u8(crate::protocol::ID_OPEN_CONNECTION_REQUEST_1);
    buf.put_slice(&MAGIC);
    buf.put_u8(protocol_version);

    // Fill rest with zeros to pad to MTU
    let padding_size = total_size.saturating_sub(buf.len());
    buf.put_bytes(0, padding_size);

    buf.freeze()
}

/// Decodes an open connection request 1 packet.
///
/// Returns (protocol_version, mtu_size).
pub fn decode_open_connection_request_1(data: &[u8]) -> Result<(u8, u16)> {
    if data.len() < 1 + 16 + 1 {
        return Err(Error::invalid_packet("OpenConnectionRequest1 too short"));
    }

    let packet_id = data[0];
    if packet_id != crate::protocol::ID_OPEN_CONNECTION_REQUEST_1 {
        return Err(Error::invalid_packet("Not an OpenConnectionRequest1 packet"));
    }

    if !check_magic(data, 1) {
        return Err(Error::InvalidMagic);
    }

    let protocol_version = data[17];

    // MTU is derived from the total packet size + IP/UDP headers
    // UDP header = 8 bytes, IP header = 20 bytes (28 total)
    let mtu = (data.len() + 28) as u16;

    Ok((protocol_version, mtu))
}

/// Encodes an open connection reply 1 packet.
///
/// Format:
/// - u8: Packet ID (0x06)
/// - [u8; 16]: Magic
/// - i64: Server GUID
/// - u8: Use encryption (0 = no)
/// - u16: MTU size
pub fn encode_open_connection_reply_1(server_guid: i64, mtu: u16) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 16 + 8 + 1 + 2);
    buf.put_u8(crate::protocol::ID_OPEN_CONNECTION_REPLY_1);
    buf.put_slice(&MAGIC);
    buf.put_i64(server_guid);
    buf.put_u8(0); // No encryption
    buf.put_u16(mtu);
    buf.freeze()
}

/// Decodes an open connection reply 1 packet.
///
/// Returns (server_guid, mtu).
pub fn decode_open_connection_reply_1(data: &[u8]) -> Result<(i64, u16)> {
    if data.len() < 1 + 16 + 8 + 1 + 2 {
        return Err(Error::invalid_packet("OpenConnectionReply1 too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_OPEN_CONNECTION_REPLY_1 {
        return Err(Error::invalid_packet("Not an OpenConnectionReply1 packet"));
    }

    if !check_magic(data, 1) {
        return Err(Error::InvalidMagic);
    }
    buf.advance(16);

    let server_guid = buf.get_i64();
    let _use_encryption = buf.get_u8();
    let mtu = buf.get_u16();

    Ok((server_guid, mtu))
}

/// Reads a SocketAddr from a buffer (RakNet address format).
///
/// Format:
/// - u8: IP version (4 = IPv4, 6 = IPv6)
/// - For IPv4: 4 bytes IP (inverted), u16 port
/// - For IPv6: 16 bytes IP, u16 port
pub fn read_address(buf: &mut &[u8]) -> Result<SocketAddr> {
    if buf.is_empty() {
        return Err(Error::invalid_packet("Address buffer empty"));
    }

    let ip_version = buf.get_u8();

    match ip_version {
        4 => {
            if buf.remaining() < 4 + 2 {
                return Err(Error::invalid_packet("IPv4 address truncated"));
            }
            // IP bytes are inverted in RakNet format
            let ip = [!buf.get_u8(), !buf.get_u8(), !buf.get_u8(), !buf.get_u8()];
            let port = buf.get_u16();
            Ok(SocketAddr::from((ip, port)))
        }
        6 => {
            if buf.remaining() < 16 + 2 {
                return Err(Error::invalid_packet("IPv6 address truncated"));
            }
            let mut ip = [0u8; 16];
            buf.copy_to_slice(&mut ip);
            let port = buf.get_u16();
            Ok(SocketAddr::from((ip, port)))
        }
        _ => Err(Error::invalid_packet(format!("Unknown IP version: {}", ip_version))),
    }
}

/// Writes a SocketAddr to a buffer (RakNet address format).
pub fn write_address(buf: &mut BytesMut, addr: &SocketAddr) {
    match addr {
        SocketAddr::V4(addr) => {
            buf.put_u8(4);
            // IP bytes are inverted in RakNet format
            for byte in addr.ip().octets() {
                buf.put_u8(!byte);
            }
            buf.put_u16(addr.port());
        }
        SocketAddr::V6(addr) => {
            buf.put_u8(6);
            buf.put_slice(&addr.ip().octets());
            buf.put_u16(addr.port());
        }
    }
}

/// Encodes an open connection request 2 packet.
///
/// Format:
/// - u8: Packet ID (0x07)
/// - [u8; 16]: Magic
/// - SocketAddr: Server address
/// - u16: MTU size
/// - i64: Client GUID
pub fn encode_open_connection_request_2(
    server_addr: SocketAddr,
    mtu: u16,
    client_guid: i64,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 16 + 7 + 2 + 8);
    buf.put_u8(crate::protocol::ID_OPEN_CONNECTION_REQUEST_2);
    buf.put_slice(&MAGIC);
    write_address(&mut buf, &server_addr);
    buf.put_u16(mtu);
    buf.put_i64(client_guid);
    buf.freeze()
}

/// Decodes an open connection request 2 packet.
///
/// Returns (server_addr, mtu, client_guid).
pub fn decode_open_connection_request_2(data: &[u8]) -> Result<(SocketAddr, u16, i64)> {
    if data.len() < 1 + 16 {
        return Err(Error::invalid_packet("OpenConnectionRequest2 too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_OPEN_CONNECTION_REQUEST_2 {
        return Err(Error::invalid_packet("Not an OpenConnectionRequest2 packet"));
    }

    if !check_magic(data, 1) {
        return Err(Error::InvalidMagic);
    }
    buf.advance(16);

    let server_addr = read_address(&mut buf)?;

    if buf.remaining() < 2 + 8 {
        return Err(Error::invalid_packet("OpenConnectionRequest2 truncated"));
    }

    let mtu = buf.get_u16();
    let client_guid = buf.get_i64();

    Ok((server_addr, mtu, client_guid))
}

/// Encodes an open connection reply 2 packet.
///
/// Format:
/// - u8: Packet ID (0x08)
/// - [u8; 16]: Magic
/// - i64: Server GUID
/// - SocketAddr: Client address
/// - u16: MTU size
/// - u8: Use encryption (0 = no)
pub fn encode_open_connection_reply_2(
    server_guid: i64,
    client_addr: SocketAddr,
    mtu: u16,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 16 + 8 + 7 + 2 + 1);
    buf.put_u8(crate::protocol::ID_OPEN_CONNECTION_REPLY_2);
    buf.put_slice(&MAGIC);
    buf.put_i64(server_guid);
    write_address(&mut buf, &client_addr);
    buf.put_u16(mtu);
    buf.put_u8(0); // No encryption
    buf.freeze()
}

/// Decodes an open connection reply 2 packet.
///
/// Returns (server_guid, client_addr, mtu).
pub fn decode_open_connection_reply_2(data: &[u8]) -> Result<(i64, SocketAddr, u16)> {
    if data.len() < 1 + 16 + 8 {
        return Err(Error::invalid_packet("OpenConnectionReply2 too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_OPEN_CONNECTION_REPLY_2 {
        return Err(Error::invalid_packet("Not an OpenConnectionReply2 packet"));
    }

    if !check_magic(data, 1) {
        return Err(Error::InvalidMagic);
    }
    buf.advance(16);

    let server_guid = buf.get_i64();
    let client_addr = read_address(&mut buf)?;

    if buf.remaining() < 2 + 1 {
        return Err(Error::invalid_packet("OpenConnectionReply2 truncated"));
    }

    let mtu = buf.get_u16();
    let _use_encryption = buf.get_u8();

    Ok((server_guid, client_addr, mtu))
}

/// Encodes a connection request packet.
///
/// Format:
/// - u8: Packet ID (0x09)
/// - i64: Client GUID
/// - i64: Timestamp
/// - u8: Use encryption (0 = no)
pub fn encode_connection_request(client_guid: i64, timestamp: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 8 + 8 + 1);
    buf.put_u8(crate::protocol::ID_CONNECTION_REQUEST);
    buf.put_i64(client_guid);
    buf.put_i64(timestamp);
    buf.put_u8(0); // No encryption
    buf.freeze()
}

/// Decodes a connection request packet.
///
/// Returns (client_guid, timestamp).
pub fn decode_connection_request(data: &[u8]) -> Result<(i64, i64)> {
    if data.len() < 1 + 8 + 8 + 1 {
        return Err(Error::invalid_packet("ConnectionRequest too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_CONNECTION_REQUEST {
        return Err(Error::invalid_packet("Not a ConnectionRequest packet"));
    }

    let client_guid = buf.get_i64();
    let timestamp = buf.get_i64();
    let _use_encryption = buf.get_u8();

    Ok((client_guid, timestamp))
}

/// Encodes a connection request accepted packet.
///
/// Format:
/// - u8: Packet ID (0x10)
/// - SocketAddr: Client address
/// - u16: System index (0)
/// - [SocketAddr; 10]: Internal addresses (all 0.0.0.0:0)
/// - i64: Request timestamp (echoed back)
/// - i64: Accepted timestamp
pub fn encode_connection_request_accepted(
    client_addr: SocketAddr,
    request_timestamp: i64,
    accepted_timestamp: i64,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 7 + 2 + (7 * 10) + 8 + 8);
    buf.put_u8(crate::protocol::ID_CONNECTION_REQUEST_ACCEPTED);
    write_address(&mut buf, &client_addr);
    buf.put_u16(0); // System index

    // Write 10 internal addresses (all 0.0.0.0:0)
    let internal_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    for _ in 0..10 {
        write_address(&mut buf, &internal_addr);
    }

    buf.put_i64(request_timestamp);
    buf.put_i64(accepted_timestamp);
    buf.freeze()
}

/// Decodes a connection request accepted packet.
///
/// Returns (client_addr, request_timestamp, accepted_timestamp).
pub fn decode_connection_request_accepted(data: &[u8]) -> Result<(SocketAddr, i64, i64)> {
    if data.len() < 1 + 7 {
        return Err(Error::invalid_packet("ConnectionRequestAccepted too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_CONNECTION_REQUEST_ACCEPTED {
        return Err(Error::invalid_packet("Not a ConnectionRequestAccepted packet"));
    }

    let client_addr = read_address(&mut buf)?;

    if buf.remaining() < 2 {
        return Err(Error::invalid_packet("ConnectionRequestAccepted truncated"));
    }

    let _system_index = buf.get_u16();

    // Skip 10 internal addresses
    for _ in 0..10 {
        read_address(&mut buf)?;
    }

    if buf.remaining() < 8 + 8 {
        return Err(Error::invalid_packet("ConnectionRequestAccepted timestamps truncated"));
    }

    let request_timestamp = buf.get_i64();
    let accepted_timestamp = buf.get_i64();

    Ok((client_addr, request_timestamp, accepted_timestamp))
}

/// Encodes a new incoming connection packet.
///
/// Format:
/// - u8: Packet ID (0x13)
/// - SocketAddr: Server address
/// - [SocketAddr; 10]: Internal addresses (all 0.0.0.0:0)
/// - i64: Request timestamp (echoed back)
/// - i64: Accepted timestamp (echoed back)
pub fn encode_new_incoming_connection(
    server_addr: SocketAddr,
    request_timestamp: i64,
    accepted_timestamp: i64,
) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 7 + (7 * 10) + 8 + 8);
    buf.put_u8(crate::protocol::ID_NEW_INCOMING_CONNECTION);
    write_address(&mut buf, &server_addr);

    // Write 10 internal addresses (all 0.0.0.0:0)
    let internal_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    for _ in 0..10 {
        write_address(&mut buf, &internal_addr);
    }

    buf.put_i64(request_timestamp);
    buf.put_i64(accepted_timestamp);
    buf.freeze()
}

/// Decodes a new incoming connection packet.
///
/// Returns (server_addr, request_timestamp, accepted_timestamp).
pub fn decode_new_incoming_connection(data: &[u8]) -> Result<(SocketAddr, i64, i64)> {
    if data.len() < 1 + 7 {
        return Err(Error::invalid_packet("NewIncomingConnection too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_NEW_INCOMING_CONNECTION {
        return Err(Error::invalid_packet("Not a NewIncomingConnection packet"));
    }

    let server_addr = read_address(&mut buf)?;

    // Skip 10 internal addresses
    for _ in 0..10 {
        read_address(&mut buf)?;
    }

    if buf.remaining() < 8 + 8 {
        return Err(Error::invalid_packet("NewIncomingConnection timestamps truncated"));
    }

    let request_timestamp = buf.get_i64();
    let accepted_timestamp = buf.get_i64();

    Ok((server_addr, request_timestamp, accepted_timestamp))
}

/// Encodes a connected ping packet.
///
/// Format:
/// - u8: Packet ID (0x00)
/// - i64: Timestamp
pub fn encode_connected_ping(timestamp: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 8);
    buf.put_u8(crate::protocol::ID_CONNECTED_PING);
    buf.put_i64(timestamp);
    buf.freeze()
}

/// Decodes a connected ping packet.
///
/// Returns timestamp.
pub fn decode_connected_ping(data: &[u8]) -> Result<i64> {
    if data.len() < 1 + 8 {
        return Err(Error::invalid_packet("ConnectedPing too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_CONNECTED_PING {
        return Err(Error::invalid_packet("Not a ConnectedPing packet"));
    }

    let timestamp = buf.get_i64();
    Ok(timestamp)
}

/// Encodes a connected pong packet.
///
/// Format:
/// - u8: Packet ID (0x03)
/// - i64: Ping timestamp (echoed back)
/// - i64: Pong timestamp
pub fn encode_connected_pong(ping_timestamp: i64, pong_timestamp: i64) -> Bytes {
    let mut buf = BytesMut::with_capacity(1 + 8 + 8);
    buf.put_u8(crate::protocol::ID_CONNECTED_PONG);
    buf.put_i64(ping_timestamp);
    buf.put_i64(pong_timestamp);
    buf.freeze()
}

/// Decodes a connected pong packet.
///
/// Returns (ping_timestamp, pong_timestamp).
pub fn decode_connected_pong(data: &[u8]) -> Result<(i64, i64)> {
    if data.len() < 1 + 8 + 8 {
        return Err(Error::invalid_packet("ConnectedPong too short"));
    }

    let mut buf = &data[..];
    let packet_id = buf.get_u8();
    if packet_id != crate::protocol::ID_CONNECTED_PONG {
        return Err(Error::invalid_packet("Not a ConnectedPong packet"));
    }

    let ping_timestamp = buf.get_i64();
    let pong_timestamp = buf.get_i64();
    Ok((ping_timestamp, pong_timestamp))
}

/// Encodes a disconnect notification packet.
///
/// Format:
/// - u8: Packet ID (0x15)
pub fn encode_disconnect_notification() -> Bytes {
    Bytes::from_static(&[crate::protocol::ID_DISCONNECT_NOTIFICATION])
}

/// Decodes a disconnect notification packet.
pub fn decode_disconnect_notification(data: &[u8]) -> Result<()> {
    if data.is_empty() {
        return Err(Error::invalid_packet("DisconnectNotification empty"));
    }

    let packet_id = data[0];
    if packet_id != crate::protocol::ID_DISCONNECT_NOTIFICATION {
        return Err(Error::invalid_packet("Not a DisconnectNotification packet"));
    }

    Ok(())
}

/// Encodes a datagram packet (FrameSet).
///
/// A datagram contains one or more frames with the reliability layer wrapping.
///
/// Format:
/// - u8: Packet ID (0x80 | 0x04 = 0x84 for valid datagram)
/// - u24: Datagram sequence number (little-endian)
/// - [Frame]: One or more frames
///
/// Note: Frames are encoded using Frame::encode() from the frame module.
pub fn encode_datagram(seq: crate::protocol::u24, frames: &[Bytes]) -> Bytes {
    use crate::protocol::{BIT_FLAG_DATAGRAM, BIT_FLAG_VALID};

    // Calculate total size
    let mut total_size = 1 + 3; // Packet ID + sequence number
    for frame in frames {
        total_size += frame.len();
    }

    let mut buf = BytesMut::with_capacity(total_size);

    // Write packet ID with flags
    buf.put_u8(BIT_FLAG_DATAGRAM | BIT_FLAG_VALID);

    // Write sequence number (u24, little-endian) using BytesMut methods
    let seq_val = seq.get();
    buf.put_u8((seq_val & 0xFF) as u8);
    buf.put_u8(((seq_val >> 8) & 0xFF) as u8);
    buf.put_u8(((seq_val >> 16) & 0xFF) as u8);

    // Write all frames
    for frame in frames {
        buf.put_slice(frame);
    }

    buf.freeze()
}

/// Decodes a datagram packet (FrameSet).
///
/// Returns (sequence_number, raw_frame_data).
/// The raw frame data contains one or more encoded frames that need to be
/// decoded separately using Frame::decode().
pub fn decode_datagram(data: &[u8]) -> Result<(crate::protocol::u24, Bytes)> {
    use crate::protocol::{read_u24_le, is_datagram};

    if data.len() < 4 {
        return Err(Error::invalid_packet("Datagram too short"));
    }

    let packet_id = data[0];
    if !is_datagram(packet_id) {
        return Err(Error::invalid_packet("Not a datagram packet"));
    }

    let mut buf = &data[1..];
    let seq = read_u24_le(&mut buf);

    // Remaining data is frames
    let frames_data = Bytes::copy_from_slice(buf);

    Ok((seq, frames_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_unconnected_ping() {
        let timestamp = 12345678;
        let client_guid = 87654321;

        let encoded = encode_unconnected_ping(timestamp, client_guid);
        let (dec_timestamp, dec_guid) = decode_unconnected_ping(&encoded).unwrap();

        assert_eq!(dec_timestamp, timestamp);
        assert_eq!(dec_guid, client_guid);
    }

    #[test]
    fn test_encode_decode_unconnected_pong() {
        let timestamp = 12345678;
        let server_guid = 87654321;
        let pong_data = b"MCPE;Rust RakNet;0;0.0.0;0;10";

        let encoded = encode_unconnected_pong(timestamp, server_guid, pong_data);
        let (dec_timestamp, dec_guid, dec_data) = decode_unconnected_pong(&encoded).unwrap();

        assert_eq!(dec_timestamp, timestamp);
        assert_eq!(dec_guid, server_guid);
        assert_eq!(&dec_data[..], &pong_data[..]);
    }

    #[test]
    fn test_encode_decode_open_connection_request_1() {
        let protocol_version = crate::PROTOCOL_VERSION;
        let mtu = 1400;

        let encoded = encode_open_connection_request_1(protocol_version, mtu);
        let (dec_version, dec_mtu) = decode_open_connection_request_1(&encoded).unwrap();

        assert_eq!(dec_version, protocol_version);
        assert_eq!(dec_mtu, mtu + 28); // Includes IP/UDP headers
    }

    #[test]
    fn test_encode_decode_open_connection_reply_1() {
        let server_guid = 12345678;
        let mtu = 1400;

        let encoded = encode_open_connection_reply_1(server_guid, mtu);
        let (dec_guid, dec_mtu) = decode_open_connection_reply_1(&encoded).unwrap();

        assert_eq!(dec_guid, server_guid);
        assert_eq!(dec_mtu, mtu);
    }

    #[test]
    fn test_write_read_address_ipv4() {
        let addr: SocketAddr = "192.168.1.100:19132".parse().unwrap();
        let mut buf = BytesMut::new();
        write_address(&mut buf, &addr);

        let mut slice = &buf[..];
        let decoded = read_address(&mut slice).unwrap();
        assert_eq!(decoded, addr);
    }

    #[test]
    fn test_write_read_address_ipv6() {
        let addr: SocketAddr = "[::1]:19132".parse().unwrap();
        let mut buf = BytesMut::new();
        write_address(&mut buf, &addr);

        let mut slice = &buf[..];
        let decoded = read_address(&mut slice).unwrap();
        assert_eq!(decoded, addr);
    }
}
