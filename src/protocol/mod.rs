/// Protocol constants and packet definitions.

pub mod uint24;
pub mod packet;
pub mod magic;
pub mod codec;
pub mod frame;

// Re-export commonly used items
pub use uint24::{u24, AtomicU24, read_u24_le, write_u24_le, read_u24_be, write_u24_be, seq_less_than, seq_in_range};
pub use packet::*;
pub use magic::{MAGIC, check_magic, write_magic};
pub use codec::*;
pub use frame::{Frame, SplitInfo};

/// RakNet protocol version.
pub const PROTOCOL_VERSION: u8 = 11;
