/// RakNet reliability levels.
///
/// These determine how packets are delivered:
/// - Unreliable: Best-effort, may be lost, duplicated, or reordered
/// - Reliable: Guaranteed delivery, but may be reordered
/// - Ordered: Guaranteed delivery in sequence order
/// - Sequenced: Latest packet only, older packets dropped
use std::fmt;

/// Reliability level for a frame.
///
/// This is encoded in the frame header (bits 5-7) and determines
/// how the packet is handled by the reliability layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Reliability {
    /// Unreliable delivery.
    /// Packets may be lost, duplicated, or arrive out of order.
    /// Fastest, no overhead.
    Unreliable = 0,

    /// Unreliable sequenced delivery.
    /// Packets may be lost or arrive out of order.
    /// Out-of-order packets are discarded (only latest is kept).
    /// Uses sequence index.
    UnreliableSequenced = 1,

    /// Reliable delivery.
    /// Guaranteed to arrive, but may be out of order.
    /// Retransmitted if lost, duplicates filtered.
    /// Uses message index for deduplication.
    Reliable = 2,

    /// Reliable ordered delivery.
    /// Guaranteed to arrive in the order sent.
    /// Packets are queued until gaps are filled.
    /// Uses message index + order index + order channel.
    /// This is the most commonly used reliability level.
    ReliableOrdered = 3,

    /// Reliable sequenced delivery.
    /// Guaranteed to arrive, but out-of-order packets are dropped.
    /// Only the latest packet in sequence is delivered.
    /// Uses message index + sequence index + order channel.
    ReliableSequenced = 4,
}

impl Reliability {
    /// Creates a Reliability from a raw u8 value.
    ///
    /// Returns None if the value is not a valid reliability level.
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Unreliable),
            1 => Some(Self::UnreliableSequenced),
            2 => Some(Self::Reliable),
            3 => Some(Self::ReliableOrdered),
            4 => Some(Self::ReliableSequenced),
            _ => None,
        }
    }

    /// Converts the Reliability to its raw u8 value.
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Returns true if this reliability level guarantees delivery.
    pub const fn is_reliable(self) -> bool {
        matches!(
            self,
            Self::Reliable | Self::ReliableOrdered | Self::ReliableSequenced
        )
    }

    /// Returns true if this reliability level uses sequencing.
    pub const fn is_sequenced(self) -> bool {
        matches!(self, Self::UnreliableSequenced | Self::ReliableSequenced)
    }

    /// Returns true if this reliability level uses ordering.
    pub const fn is_ordered(self) -> bool {
        matches!(self, Self::ReliableOrdered | Self::ReliableSequenced)
    }

    /// Returns the header size for this reliability level (in bytes).
    ///
    /// This does not include the split packet overhead.
    pub const fn header_size(self) -> usize {
        // Base: 1 byte flags + 2 bytes length (in bits) = 3 bytes
        let mut size = 3;

        // Reliable: +3 bytes for message index
        if self.is_reliable() {
            size += 3;
        }

        // Sequenced: +3 bytes for sequence index
        if self.is_sequenced() {
            size += 3;
        }

        // Ordered: +3 bytes for order index + 1 byte for order channel
        if self.is_ordered() {
            size += 4;
        }

        size
    }

    /// Returns the maximum header size for any reliability level.
    pub const MAX_HEADER_SIZE: usize = 14; // Worst case: ReliableSequenced

    /// Returns the minimum header size.
    pub const MIN_HEADER_SIZE: usize = 3; // Best case: Unreliable
}

impl Default for Reliability {
    /// Default reliability is ReliableOrdered (most commonly used).
    fn default() -> Self {
        Self::ReliableOrdered
    }
}

impl fmt::Display for Reliability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreliable => write!(f, "Unreliable"),
            Self::UnreliableSequenced => write!(f, "UnreliableSequenced"),
            Self::Reliable => write!(f, "Reliable"),
            Self::ReliableOrdered => write!(f, "ReliableOrdered"),
            Self::ReliableSequenced => write!(f, "ReliableSequenced"),
        }
    }
}

/// Macro to match on reliability and execute code blocks.
///
/// Useful for handling different reliability levels with pattern matching.
#[macro_export]
macro_rules! match_reliability {
    ($reliability:expr, {
        Unreliable => $unreliable:expr,
        UnreliableSequenced => $unreliable_seq:expr,
        Reliable => $reliable:expr,
        ReliableOrdered => $reliable_ord:expr,
        ReliableSequenced => $reliable_seq:expr,
    }) => {
        match $reliability {
            $crate::reliability::Reliability::Unreliable => $unreliable,
            $crate::reliability::Reliability::UnreliableSequenced => $unreliable_seq,
            $crate::reliability::Reliability::Reliable => $reliable,
            $crate::reliability::Reliability::ReliableOrdered => $reliable_ord,
            $crate::reliability::Reliability::ReliableSequenced => $reliable_seq,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reliability_from_u8() {
        assert_eq!(Reliability::from_u8(0), Some(Reliability::Unreliable));
        assert_eq!(Reliability::from_u8(1), Some(Reliability::UnreliableSequenced));
        assert_eq!(Reliability::from_u8(2), Some(Reliability::Reliable));
        assert_eq!(Reliability::from_u8(3), Some(Reliability::ReliableOrdered));
        assert_eq!(Reliability::from_u8(4), Some(Reliability::ReliableSequenced));
        assert_eq!(Reliability::from_u8(5), None);
        assert_eq!(Reliability::from_u8(255), None);
    }

    #[test]
    fn test_reliability_to_u8() {
        assert_eq!(Reliability::Unreliable.to_u8(), 0);
        assert_eq!(Reliability::UnreliableSequenced.to_u8(), 1);
        assert_eq!(Reliability::Reliable.to_u8(), 2);
        assert_eq!(Reliability::ReliableOrdered.to_u8(), 3);
        assert_eq!(Reliability::ReliableSequenced.to_u8(), 4);
    }

    #[test]
    fn test_is_reliable() {
        assert!(!Reliability::Unreliable.is_reliable());
        assert!(!Reliability::UnreliableSequenced.is_reliable());
        assert!(Reliability::Reliable.is_reliable());
        assert!(Reliability::ReliableOrdered.is_reliable());
        assert!(Reliability::ReliableSequenced.is_reliable());
    }

    #[test]
    fn test_is_sequenced() {
        assert!(!Reliability::Unreliable.is_sequenced());
        assert!(Reliability::UnreliableSequenced.is_sequenced());
        assert!(!Reliability::Reliable.is_sequenced());
        assert!(!Reliability::ReliableOrdered.is_sequenced());
        assert!(Reliability::ReliableSequenced.is_sequenced());
    }

    #[test]
    fn test_is_ordered() {
        assert!(!Reliability::Unreliable.is_ordered());
        assert!(!Reliability::UnreliableSequenced.is_ordered());
        assert!(!Reliability::Reliable.is_ordered());
        assert!(Reliability::ReliableOrdered.is_ordered());
        assert!(Reliability::ReliableSequenced.is_ordered());
    }

    #[test]
    fn test_header_size() {
        assert_eq!(Reliability::Unreliable.header_size(), 3);
        assert_eq!(Reliability::UnreliableSequenced.header_size(), 6);
        assert_eq!(Reliability::Reliable.header_size(), 6);
        assert_eq!(Reliability::ReliableOrdered.header_size(), 10);
        assert_eq!(Reliability::ReliableSequenced.header_size(), 13);
    }

    #[test]
    fn test_default() {
        assert_eq!(Reliability::default(), Reliability::ReliableOrdered);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Reliability::Unreliable), "Unreliable");
        assert_eq!(format!("{}", Reliability::ReliableOrdered), "ReliableOrdered");
    }
}
