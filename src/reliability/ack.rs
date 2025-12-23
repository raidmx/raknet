/// ACK/NACK range compression and encoding.
///
/// This module implements efficient range compression for acknowledgments.
/// Instead of sending individual sequence numbers, consecutive numbers are
/// compressed into ranges, significantly reducing packet size.
///
/// Format:
/// - Single: [type=1, seq (3 bytes)]
/// - Range: [type=0, start (3 bytes), end (3 bytes)]

use crate::error::{Error, Result};
use crate::protocol::{read_u24_le, u24, write_u24_le, ID_ACK, ID_NACK};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// A range of sequence numbers.
///
/// Represents either a single sequence number or a range from start to end (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckRange {
    /// A single sequence number.
    Single(u24),

    /// A range of sequence numbers (inclusive).
    Range { start: u24, end: u24 },
}

impl AckRange {
    /// Creates a new single sequence range.
    pub fn single(seq: u24) -> Self {
        Self::Single(seq)
    }

    /// Creates a new sequence range.
    pub fn range(start: u24, end: u24) -> Self {
        if start == end {
            Self::Single(start)
        } else {
            Self::Range { start, end }
        }
    }

    /// Returns the number of sequence numbers in this range.
    pub fn count(&self) -> u32 {
        match self {
            Self::Single(_) => 1,
            Self::Range { start, end } => {
                end.get().wrapping_sub(start.get()).wrapping_add(1)
            }
        }
    }

    /// Checks if this range contains a sequence number.
    pub fn contains(&self, seq: u24) -> bool {
        match self {
            Self::Single(s) => *s == seq,
            Self::Range { start, end } => {
                let seq = seq.get();
                let start = start.get();
                let end = end.get();

                if start <= end {
                    seq >= start && seq <= end
                } else {
                    // Wraparound case
                    seq >= start || seq <= end
                }
            }
        }
    }

    /// Returns the encoded size of this range.
    pub fn encoded_size(&self) -> usize {
        match self {
            Self::Single(_) => 1 + 3, // type + seq
            Self::Range { .. } => 1 + 3 + 3, // type + start + end
        }
    }

    /// Encodes this range into a buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            Self::Single(seq) => {
                buf.put_u8(1); // Type: single
                let mut temp = [0u8; 3];
                write_u24_le(&mut temp, *seq);
                buf.put_slice(&temp);
            }
            Self::Range { start, end } => {
                buf.put_u8(0); // Type: range
                let mut temp = [0u8; 3];
                write_u24_le(&mut temp, *start);
                buf.put_slice(&temp);
                write_u24_le(&mut temp, *end);
                buf.put_slice(&temp);
            }
        }
    }

    /// Decodes a range from a buffer.
    pub fn decode(buf: &mut &[u8]) -> Result<Self> {
        if buf.remaining() < 1 {
            return Err(Error::invalid_packet("ACK range too short"));
        }

        let range_type = buf.get_u8();

        match range_type {
            1 => {
                // Single
                if buf.remaining() < 3 {
                    return Err(Error::invalid_packet("ACK single range truncated"));
                }
                let seq = read_u24_le(buf);
                buf.advance(3);
                Ok(Self::Single(seq))
            }
            0 => {
                // Range
                if buf.remaining() < 6 {
                    return Err(Error::invalid_packet("ACK range truncated"));
                }
                let start = read_u24_le(buf);
                buf.advance(3);
                let end = read_u24_le(buf);
                buf.advance(3);
                Ok(Self::Range { start, end })
            }
            _ => Err(Error::invalid_packet(format!("Invalid ACK range type: {}", range_type))),
        }
    }
}

/// List of ACK/NACK ranges with automatic compression.
///
/// This structure maintains a list of sequence numbers and automatically
/// merges them into ranges for efficient encoding.
#[derive(Debug, Clone)]
pub struct AckRangeList {
    ranges: Vec<AckRange>,
}

impl AckRangeList {
    /// Creates a new empty range list.
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Creates a range list from a vector of sequence numbers.
    pub fn from_sequences(mut seqs: Vec<u32>) -> Self {
        let mut list = Self::new();
        seqs.sort_unstable();
        for seq in seqs {
            list.insert(u24::new(seq));
        }
        list
    }

    /// Inserts a sequence number into the list.
    ///
    /// Automatically merges with adjacent ranges.
    pub fn insert(&mut self, seq: u24) {
        let seq_val = seq.get();

        // Find where to insert
        let insert_pos = self.ranges.iter().position(|range| {
            match range {
                AckRange::Single(s) => {
                    if s.get() == seq_val {
                        return true; // Duplicate
                    }
                    s.get() > seq_val
                }
                AckRange::Range { start, end } => {
                    if range.contains(seq) {
                        return true; // Already in range
                    }
                    start.get() > seq_val
                }
            }
        });

        match insert_pos {
            Some(pos) if self.ranges[pos].contains(seq) => {
                // Already in range, nothing to do
                return;
            }
            Some(pos) => {
                // Try to merge with previous range
                if pos > 0 && self.can_merge_with_next(pos - 1, seq) {
                    self.merge_at(pos - 1, seq);
                } else if self.can_merge_with_prev(pos, seq) {
                    self.merge_at(pos, seq);
                } else {
                    // Insert new single range
                    self.ranges.insert(pos, AckRange::Single(seq));
                }
            }
            None => {
                // Insert at end
                if !self.ranges.is_empty() && self.can_merge_with_next(self.ranges.len() - 1, seq) {
                    let last_idx = self.ranges.len() - 1;
                    self.merge_at(last_idx, seq);
                } else {
                    self.ranges.push(AckRange::Single(seq));
                }
            }
        }
    }

    /// Checks if a sequence can merge with the range after it.
    fn can_merge_with_next(&self, idx: usize, seq: u24) -> bool {
        match &self.ranges[idx] {
            AckRange::Single(s) => s.get().wrapping_add(1) == seq.get(),
            AckRange::Range { end, .. } => end.get().wrapping_add(1) == seq.get(),
        }
    }

    /// Checks if a sequence can merge with the range before it.
    fn can_merge_with_prev(&self, idx: usize, seq: u24) -> bool {
        match &self.ranges[idx] {
            AckRange::Single(s) => seq.get().wrapping_add(1) == s.get(),
            AckRange::Range { start, .. } => seq.get().wrapping_add(1) == start.get(),
        }
    }

    /// Merges a sequence number with the range at idx.
    fn merge_at(&mut self, idx: usize, seq: u24) {
        let range = &self.ranges[idx];

        let new_range = match range {
            AckRange::Single(s) => {
                if s.get().wrapping_add(1) == seq.get() {
                    AckRange::Range { start: *s, end: seq }
                } else if seq.get().wrapping_add(1) == s.get() {
                    AckRange::Range { start: seq, end: *s }
                } else {
                    return; // Can't merge
                }
            }
            AckRange::Range { start, end } => {
                if end.get().wrapping_add(1) == seq.get() {
                    AckRange::Range { start: *start, end: seq }
                } else if seq.get().wrapping_add(1) == start.get() {
                    AckRange::Range { start: seq, end: *end }
                } else {
                    return; // Can't merge
                }
            }
        };

        self.ranges[idx] = new_range;

        // Try to merge with next range
        if idx + 1 < self.ranges.len() {
            if let AckRange::Range { start, end } = new_range {
                if end.get().wrapping_add(1) == self.get_range_start(idx + 1).get() {
                    let next_end = self.get_range_end(idx + 1);
                    self.ranges[idx] = AckRange::Range { start, end: next_end };
                    self.ranges.remove(idx + 1);
                }
            }
        }
    }

    fn get_range_start(&self, idx: usize) -> u24 {
        match &self.ranges[idx] {
            AckRange::Single(s) => *s,
            AckRange::Range { start, .. } => *start,
        }
    }

    fn get_range_end(&self, idx: usize) -> u24 {
        match &self.ranges[idx] {
            AckRange::Single(s) => *s,
            AckRange::Range { end, .. } => *end,
        }
    }

    /// Returns the number of ranges in this list.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Returns the ranges in this list.
    pub fn ranges(&self) -> &[AckRange] {
        &self.ranges
    }

    /// Returns the total encoded size of all ranges.
    pub fn encoded_size(&self) -> usize {
        self.ranges.iter().map(|r| r.encoded_size()).sum()
    }

    /// Clears all ranges.
    pub fn clear(&mut self) {
        self.ranges.clear();
    }
}

impl Default for AckRangeList {
    fn default() -> Self {
        Self::new()
    }
}

/// Encodes an ACK packet.
pub fn encode_ack(ranges: &AckRangeList) -> Bytes {
    let mut buf = BytesMut::with_capacity(3 + ranges.encoded_size());

    buf.put_u8(ID_ACK);
    buf.put_u16(ranges.len() as u16);

    for range in ranges.ranges() {
        range.encode(&mut buf);
    }

    buf.freeze()
}

/// Encodes a NACK packet.
pub fn encode_nack(ranges: &AckRangeList) -> Bytes {
    let mut buf = BytesMut::with_capacity(3 + ranges.encoded_size());

    buf.put_u8(ID_NACK);
    buf.put_u16(ranges.len() as u16);

    for range in ranges.ranges() {
        range.encode(&mut buf);
    }

    buf.freeze()
}

/// Decodes an ACK or NACK packet.
pub fn decode_ack_nack(data: &[u8]) -> Result<(bool, AckRangeList)> {
    if data.is_empty() {
        return Err(Error::invalid_packet("ACK/NACK packet empty"));
    }

    let packet_id = data[0];
    let is_ack = match packet_id {
        ID_ACK => true,
        ID_NACK => false,
        _ => return Err(Error::invalid_packet("Not an ACK/NACK packet")),
    };

    let mut buf = &data[1..];

    if buf.remaining() < 2 {
        return Err(Error::invalid_packet("ACK/NACK missing record count"));
    }

    let record_count = buf.get_u16();
    let mut list = AckRangeList::new();

    for _ in 0..record_count {
        let range = AckRange::decode(&mut buf)?;
        match range {
            AckRange::Single(seq) => list.insert(seq),
            AckRange::Range { start, end } => {
                let start_val = start.get();
                let end_val = end.get();

                // Handle wraparound
                if start_val <= end_val {
                    for seq in start_val..=end_val {
                        list.insert(u24::new(seq));
                    }
                } else {
                    // Wraparound: start..MAX and 0..=end
                    for seq in start_val..=0x00FFFFFF {
                        list.insert(u24::new(seq));
                    }
                    for seq in 0..=end_val {
                        list.insert(u24::new(seq));
                    }
                }
            }
        }
    }

    Ok((is_ack, list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_range_single() {
        let range = AckRange::single(u24::new(42));
        assert_eq!(range.count(), 1);
        assert!(range.contains(u24::new(42)));
        assert!(!range.contains(u24::new(43)));
    }

    #[test]
    fn test_ack_range_range() {
        let range = AckRange::range(u24::new(10), u24::new(20));
        assert_eq!(range.count(), 11);
        assert!(range.contains(u24::new(10)));
        assert!(range.contains(u24::new(15)));
        assert!(range.contains(u24::new(20)));
        assert!(!range.contains(u24::new(9)));
        assert!(!range.contains(u24::new(21)));
    }

    #[test]
    fn test_ack_range_list_compression() {
        let mut list = AckRangeList::new();

        // Insert consecutive numbers
        list.insert(u24::new(0));
        list.insert(u24::new(1));
        list.insert(u24::new(2));
        list.insert(u24::new(3));

        // Should be compressed into one range
        assert_eq!(list.len(), 1);
        assert!(matches!(list.ranges()[0], AckRange::Range { .. }));
    }

    #[test]
    fn test_ack_range_list_gaps() {
        let mut list = AckRangeList::new();

        list.insert(u24::new(0));
        list.insert(u24::new(1));
        list.insert(u24::new(5));
        list.insert(u24::new(6));

        // Should have two ranges
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_encode_decode_ack() {
        let mut list = AckRangeList::new();
        list.insert(u24::new(1));
        list.insert(u24::new(2));
        list.insert(u24::new(3));
        list.insert(u24::new(10));

        let encoded = encode_ack(&list);
        let (is_ack, decoded_list) = decode_ack_nack(&encoded).unwrap();

        assert!(is_ack);
        // Decoded list should contain the same sequences
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(1))));
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(2))));
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(3))));
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(10))));
    }

    #[test]
    fn test_encode_decode_nack() {
        let mut list = AckRangeList::new();
        list.insert(u24::new(5));
        list.insert(u24::new(7));

        let encoded = encode_nack(&list);
        let (is_ack, decoded_list) = decode_ack_nack(&encoded).unwrap();

        assert!(!is_ack);
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(5))));
        assert!(decoded_list.ranges().iter().any(|r| r.contains(u24::new(7))));
    }

    #[test]
    fn test_ack_range_list_from_sequences() {
        let seqs = vec![1, 2, 3, 5, 6, 10];
        let list = AckRangeList::from_sequences(seqs);

        // Should have 3 ranges: [1-3], [5-6], [10]
        assert_eq!(list.len(), 3);
    }
}
