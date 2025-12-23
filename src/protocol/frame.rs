/// Frame (encapsulated packet) structure and encoding.
///
/// Frames are the basic unit of data transmission in RakNet's reliability layer.
/// Each datagram can contain multiple frames.

use crate::error::{Error, Result};
use crate::protocol::{read_u24_le, u24, write_u24_le};
use crate::reliability::Reliability;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Split packet information.
///
/// When a packet is too large for the MTU, it's split into fragments.
/// Each fragment includes this information to enable reassembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitInfo {
    /// Total number of fragments.
    pub count: u32,

    /// Unique identifier for this split packet.
    pub id: u16,

    /// Index of this fragment (0 to count-1).
    pub index: u32,
}

/// A RakNet frame (encapsulated packet).
///
/// This represents a single packet within a datagram, with its own
/// reliability and ordering information.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Reliability level for this frame.
    pub reliability: Reliability,

    /// Message index (for reliable packets).
    /// Used for deduplication.
    pub message_index: Option<u24>,

    /// Sequence index (for sequenced packets).
    /// Used to discard out-of-order packets.
    pub sequence_index: Option<u24>,

    /// Order index (for ordered packets).
    /// Used to ensure in-order delivery.
    pub order_index: Option<u24>,

    /// Order channel (0-15).
    /// Each channel has independent ordering.
    pub order_channel: u8,

    /// Split packet information (if this is a fragment).
    pub split: Option<SplitInfo>,

    /// The actual payload data.
    pub payload: Bytes,
}

impl Frame {
    /// Creates a new frame with the given reliability and payload.
    pub fn new(reliability: Reliability, payload: Bytes) -> Self {
        Self {
            reliability,
            message_index: None,
            sequence_index: None,
            order_index: None,
            order_channel: 0,
            split: None,
            payload,
        }
    }

    /// Builder pattern: Sets the message index.
    pub fn with_message_index(mut self, index: u24) -> Self {
        self.message_index = Some(index);
        self
    }

    /// Builder pattern: Sets the sequence index.
    pub fn with_sequence_index(mut self, index: u24) -> Self {
        self.sequence_index = Some(index);
        self
    }

    /// Builder pattern: Sets the order index and channel.
    pub fn with_order(mut self, index: u24, channel: u8) -> Self {
        self.order_index = Some(index);
        self.order_channel = channel;
        self
    }

    /// Builder pattern: Sets the split information.
    pub fn with_split(mut self, split: SplitInfo) -> Self {
        self.split = Some(split);
        self
    }

    /// Returns the total size of this frame when encoded.
    pub fn encoded_size(&self) -> usize {
        let mut size = 1; // Flags byte

        // Length in bits (2 bytes)
        size += 2;

        // Reliable: message index (3 bytes)
        if self.reliability.is_reliable() {
            size += 3;
        }

        // Sequenced: sequence index (3 bytes)
        if self.reliability.is_sequenced() {
            size += 3;
        }

        // Ordered: order index (3 bytes) + order channel (1 byte)
        if self.reliability.is_ordered() {
            size += 4;
        }

        // Split: count (4) + id (2) + index (4) = 10 bytes
        if self.split.is_some() {
            size += 10;
        }

        // Payload
        size += self.payload.len();

        size
    }

    /// Encodes this frame into a buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        // Flags byte: reliability (bits 5-7) + split flag (bit 4)
        let mut flags = self.reliability.to_u8() << 5;
        if self.split.is_some() {
            flags |= 0b0001_0000; // Set split flag (bit 4)
        }
        buf.put_u8(flags);

        // Length in bits (big-endian u16)
        let length_bits = (self.payload.len() * 8) as u16;
        buf.put_u16(length_bits);

        // Reliable: message index
        if self.reliability.is_reliable() {
            if let Some(index) = self.message_index {
                let mut temp = [0u8; 3];
                write_u24_le(&mut temp, index);
                buf.put_slice(&temp);
            } else {
                // Should not happen if used correctly
                buf.put_u8(0);
                buf.put_u8(0);
                buf.put_u8(0);
            }
        }

        // Sequenced: sequence index
        if self.reliability.is_sequenced() {
            if let Some(index) = self.sequence_index {
                let mut temp = [0u8; 3];
                write_u24_le(&mut temp, index);
                buf.put_slice(&temp);
            } else {
                buf.put_u8(0);
                buf.put_u8(0);
                buf.put_u8(0);
            }
        }

        // Ordered: order index + order channel
        if self.reliability.is_ordered() {
            if let Some(index) = self.order_index {
                let mut temp = [0u8; 3];
                write_u24_le(&mut temp, index);
                buf.put_slice(&temp);
            } else {
                buf.put_u8(0);
                buf.put_u8(0);
                buf.put_u8(0);
            }
            buf.put_u8(self.order_channel);
        }

        // Split: count + id + index
        if let Some(ref split) = self.split {
            buf.put_u32(split.count);
            buf.put_u16(split.id);
            buf.put_u32(split.index);
        }

        // Payload
        buf.put_slice(&self.payload);
    }

    /// Decodes a frame from a buffer.
    pub fn decode(buf: &mut &[u8]) -> Result<Self> {
        if buf.len() < 3 {
            return Err(Error::invalid_packet("Frame too short"));
        }

        // Flags byte
        let flags = buf.get_u8();
        let reliability_bits = (flags >> 5) & 0b111;
        let split_flag = (flags & 0b0001_0000) != 0;

        let reliability = Reliability::from_u8(reliability_bits)
            .ok_or_else(|| Error::invalid_packet(format!("Invalid reliability: {}", reliability_bits)))?;

        // Length in bits
        let length_bits = buf.get_u16();
        let length_bytes = ((length_bits + 7) / 8) as usize; // Round up to bytes

        // Message index (reliable)
        let message_index = if reliability.is_reliable() {
            if buf.remaining() < 3 {
                return Err(Error::invalid_packet("Frame missing message index"));
            }
            Some(read_u24_le(buf))
        } else {
            None
        };
        if message_index.is_some() {
            buf.advance(3);
        }

        // Sequence index (sequenced)
        let sequence_index = if reliability.is_sequenced() {
            if buf.remaining() < 3 {
                return Err(Error::invalid_packet("Frame missing sequence index"));
            }
            Some(read_u24_le(buf))
        } else {
            None
        };
        if sequence_index.is_some() {
            buf.advance(3);
        }

        // Order index + channel (ordered)
        let (order_index, order_channel) = if reliability.is_ordered() {
            if buf.remaining() < 4 {
                return Err(Error::invalid_packet("Frame missing order index"));
            }
            let index = read_u24_le(buf);
            buf.advance(3);
            let channel = buf.get_u8();
            (Some(index), channel)
        } else {
            (None, 0)
        };

        // Split info
        let split = if split_flag {
            if buf.remaining() < 10 {
                return Err(Error::invalid_packet("Frame missing split info"));
            }
            let count = buf.get_u32();
            let id = buf.get_u16();
            let index = buf.get_u32();
            Some(SplitInfo { count, id, index })
        } else {
            None
        };

        // Payload
        if buf.remaining() < length_bytes {
            return Err(Error::invalid_packet(format!(
                "Frame payload truncated: expected {} bytes, got {}",
                length_bytes,
                buf.remaining()
            )));
        }
        let payload = Bytes::copy_from_slice(&buf[..length_bytes]);
        buf.advance(length_bytes);

        Ok(Self {
            reliability,
            message_index,
            sequence_index,
            order_index,
            order_channel,
            split,
            payload,
        })
    }
}

/// Macro to create a frame with builder pattern.
///
/// Usage:
/// ```
/// let frame = frame! {
///     reliability: Reliability::ReliableOrdered,
///     message_index: u24::new(42),
///     order_index: u24::new(10),
///     order_channel: 0,
///     payload: Bytes::from("hello"),
/// };
/// ```
#[macro_export]
macro_rules! frame {
    (
        reliability: $reliability:expr,
        payload: $payload:expr
        $(, message_index: $msg_idx:expr)?
        $(, sequence_index: $seq_idx:expr)?
        $(, order_index: $ord_idx:expr)?
        $(, order_channel: $ord_ch:expr)?
        $(, split: $split:expr)?
        $(,)?
    ) => {{
        let mut frame = $crate::protocol::Frame::new($reliability, $payload);
        $(frame.message_index = Some($msg_idx);)?
        $(frame.sequence_index = Some($seq_idx);)?
        $(frame.order_index = Some($ord_idx);)?
        $(frame.order_channel = $ord_ch;)?
        $(frame.split = Some($split);)?
        frame
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_unreliable() {
        let payload = Bytes::from("hello");
        let frame = Frame::new(Reliability::Unreliable, payload.clone());

        let mut buf = BytesMut::new();
        frame.encode(&mut buf);

        let mut slice = &buf[..];
        let decoded = Frame::decode(&mut slice).unwrap();

        assert_eq!(decoded.reliability, Reliability::Unreliable);
        assert_eq!(decoded.payload, payload);
        assert!(decoded.message_index.is_none());
        assert!(decoded.sequence_index.is_none());
        assert!(decoded.order_index.is_none());
        assert!(decoded.split.is_none());
    }

    #[test]
    fn test_frame_reliable_ordered() {
        let payload = Bytes::from("test data");
        let frame = Frame::new(Reliability::ReliableOrdered, payload.clone())
            .with_message_index(u24::new(42))
            .with_order(u24::new(10), 3);

        let mut buf = BytesMut::new();
        frame.encode(&mut buf);

        let mut slice = &buf[..];
        let decoded = Frame::decode(&mut slice).unwrap();

        assert_eq!(decoded.reliability, Reliability::ReliableOrdered);
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.message_index, Some(u24::new(42)));
        assert_eq!(decoded.order_index, Some(u24::new(10)));
        assert_eq!(decoded.order_channel, 3);
    }

    #[test]
    fn test_frame_with_split() {
        let payload = Bytes::from("fragment");
        let split = SplitInfo {
            count: 5,
            id: 123,
            index: 2,
        };
        let frame = Frame::new(Reliability::Reliable, payload.clone())
            .with_message_index(u24::new(100))
            .with_split(split.clone());

        let mut buf = BytesMut::new();
        frame.encode(&mut buf);

        let mut slice = &buf[..];
        let decoded = Frame::decode(&mut slice).unwrap();

        assert_eq!(decoded.reliability, Reliability::Reliable);
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.split, Some(split));
    }

    #[test]
    fn test_frame_encoded_size() {
        let payload = Bytes::from("test");

        let unreliable = Frame::new(Reliability::Unreliable, payload.clone());
        assert_eq!(unreliable.encoded_size(), 3 + 4); // flags + length + payload

        let reliable = Frame::new(Reliability::Reliable, payload.clone())
            .with_message_index(u24::new(1));
        assert_eq!(reliable.encoded_size(), 3 + 3 + 4); // + message_index

        let ordered = Frame::new(Reliability::ReliableOrdered, payload.clone())
            .with_message_index(u24::new(1))
            .with_order(u24::new(1), 0);
        assert_eq!(ordered.encoded_size(), 3 + 3 + 4 + 4); // + message_index + order
    }
}
