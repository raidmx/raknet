use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

/// A 24-bit unsigned integer (0 to 16,777,215).
/// Internally stored as u32 with the upper 8 bits always zero.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct u24(u32);

impl u24 {
    /// Maximum value for a 24-bit unsigned integer (0x00FFFFFF = 16,777,215).
    pub const MAX: u24 = u24(0x00FFFFFF);

    /// Minimum value for a 24-bit unsigned integer (0).
    pub const MIN: u24 = u24(0);

    /// Mask for 24-bit values.
    const MASK: u32 = 0x00FFFFFF;

    /// Creates a new u24 from a u32, masking off the upper 8 bits.
    #[inline]
    pub const fn new(val: u32) -> Self {
        Self(val & Self::MASK)
    }

    /// Returns the value as a u32.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Wrapping addition. Overflows wrap around to 0.
    #[inline]
    pub const fn wrapping_add(self, rhs: u32) -> Self {
        Self::new(self.0.wrapping_add(rhs))
    }

    /// Wrapping subtraction. Underflows wrap around to MAX.
    #[inline]
    pub const fn wrapping_sub(self, rhs: u32) -> Self {
        Self::new(self.0.wrapping_sub(rhs))
    }

    /// Checked addition. Returns None on overflow.
    #[inline]
    pub const fn checked_add(self, rhs: u32) -> Option<Self> {
        let result = self.0 + rhs;
        if result > Self::MASK {
            None
        } else {
            Some(Self(result))
        }
    }

    /// Checked subtraction. Returns None on underflow.
    #[inline]
    pub const fn checked_sub(self, rhs: u32) -> Option<Self> {
        if rhs > self.0 {
            None
        } else {
            Some(Self(self.0 - rhs))
        }
    }

    /// Saturating addition. Clamps to MAX on overflow.
    #[inline]
    pub const fn saturating_add(self, rhs: u32) -> Self {
        let result = self.0.saturating_add(rhs);
        if result > Self::MASK {
            Self::MAX
        } else {
            Self(result)
        }
    }

    /// Saturating subtraction. Clamps to MIN on underflow.
    #[inline]
    pub const fn saturating_sub(self, rhs: u32) -> Self {
        Self(self.0.saturating_sub(rhs))
    }
}

impl fmt::Debug for u24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "u24({})", self.0)
    }
}

impl fmt::Display for u24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u24> for u32 {
    fn from(val: u24) -> Self {
        val.0
    }
}

impl From<u16> for u24 {
    fn from(val: u16) -> Self {
        u24(val as u32)
    }
}

impl From<u8> for u24 {
    fn from(val: u8) -> Self {
        u24(val as u32)
    }
}

/// Atomic 24-bit unsigned integer.
/// Internally uses AtomicU32 with values masked to 24 bits.
pub struct AtomicU24(AtomicU32);

impl AtomicU24 {
    /// Creates a new AtomicU24.
    #[inline]
    pub const fn new(val: u32) -> Self {
        Self(AtomicU32::new(val & u24::MASK))
    }

    /// Loads the value.
    #[inline]
    pub fn load(&self, order: Ordering) -> u24 {
        u24::new(self.0.load(order))
    }

    /// Stores a value.
    #[inline]
    pub fn store(&self, val: u24, order: Ordering) {
        self.0.store(val.0, order);
    }

    /// Swaps the value, returning the previous value.
    #[inline]
    pub fn swap(&self, val: u24, order: Ordering) -> u24 {
        u24::new(self.0.swap(val.0, order))
    }

    /// Atomically adds to the current value, returning the previous value.
    /// Wraps around on overflow.
    #[inline]
    pub fn fetch_add(&self, val: u32, order: Ordering) -> u24 {
        let old = self.0.fetch_add(val, order);
        u24::new(old)
    }

    /// Atomically subtracts from the current value, returning the previous value.
    /// Wraps around on underflow.
    #[inline]
    pub fn fetch_sub(&self, val: u32, order: Ordering) -> u24 {
        let old = self.0.fetch_sub(val, order);
        u24::new(old)
    }

    /// Compares and exchanges the value.
    #[inline]
    pub fn compare_exchange(
        &self,
        current: u24,
        new: u24,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u24, u24> {
        self.0
            .compare_exchange(current.0, new.0, success, failure)
            .map(u24::new)
            .map_err(u24::new)
    }

    /// Compares and exchanges the value (weak version).
    #[inline]
    pub fn compare_exchange_weak(
        &self,
        current: u24,
        new: u24,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u24, u24> {
        self.0
            .compare_exchange_weak(current.0, new.0, success, failure)
            .map(u24::new)
            .map_err(u24::new)
    }
}

impl Default for AtomicU24 {
    fn default() -> Self {
        Self::new(0)
    }
}

impl fmt::Debug for AtomicU24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AtomicU24({})", self.load(Ordering::Relaxed))
    }
}

/// Writes a u24 value in little-endian byte order.
#[inline]
pub fn write_u24_le(buf: &mut [u8], val: u24) {
    debug_assert!(buf.len() >= 3, "Buffer too small for u24");
    let v = val.get();
    buf[0] = (v & 0xFF) as u8;
    buf[1] = ((v >> 8) & 0xFF) as u8;
    buf[2] = ((v >> 16) & 0xFF) as u8;
}

/// Reads a u24 value in little-endian byte order.
#[inline]
pub fn read_u24_le(buf: &[u8]) -> u24 {
    debug_assert!(buf.len() >= 3, "Buffer too small for u24");
    u24::new(
        (buf[0] as u32)
        | ((buf[1] as u32) << 8)
        | ((buf[2] as u32) << 16)
    )
}

/// Writes a u24 value in big-endian byte order.
#[inline]
pub fn write_u24_be(buf: &mut [u8], val: u24) {
    debug_assert!(buf.len() >= 3, "Buffer too small for u24");
    let v = val.get();
    buf[0] = ((v >> 16) & 0xFF) as u8;
    buf[1] = ((v >> 8) & 0xFF) as u8;
    buf[2] = (v & 0xFF) as u8;
}

/// Reads a u24 value in big-endian byte order.
#[inline]
pub fn read_u24_be(buf: &[u8]) -> u24 {
    debug_assert!(buf.len() >= 3, "Buffer too small for u24");
    u24::new(
        ((buf[0] as u32) << 16)
        | ((buf[1] as u32) << 8)
        | (buf[2] as u32)
    )
}

/// Compares two sequence numbers accounting for wraparound.
/// Returns true if `a` is less than `b` in sequence space.
///
/// This correctly handles wraparound by using signed arithmetic in 24-bit space.
/// For example, seq 5 is "greater than" seq 16777210 (near MAX),
/// because 5 comes after 16777210 in wraparound sequence.
#[inline]
pub fn seq_less_than(a: u32, b: u32) -> bool {
    // Compute difference and mask to 24 bits
    let diff = a.wrapping_sub(b) & 0x00FFFFFF;

    // Sign-extend from 24-bit to 32-bit
    // If bit 23 is set, it's negative in 24-bit space
    let signed_diff = if (diff & 0x00800000) != 0 {
        (diff | 0xFF000000) as i32 // Negative: set upper bits
    } else {
        diff as i32 // Positive
    };

    signed_diff < 0
}

/// Checks if a sequence number is within a range, accounting for wraparound.
///
/// If start <= end, this is a simple range check.
/// If start > end, the range wraps around (e.g., [16777200, 100]).
#[inline]
pub fn seq_in_range(seq: u32, start: u32, end: u32) -> bool {
    if start <= end {
        seq >= start && seq <= end
    } else {
        // Wraparound case
        seq >= start || seq <= end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u24_basic() {
        assert_eq!(u24::new(42).get(), 42);
        assert_eq!(u24::MAX.get(), 0x00FFFFFF);
        assert_eq!(u24::MIN.get(), 0);
    }

    #[test]
    fn test_u24_masking() {
        // Values above 24 bits should be masked
        assert_eq!(u24::new(0x01FFFFFF).get(), 0x00FFFFFF);
        assert_eq!(u24::new(0xFF123456).get(), 0x00123456);
    }

    #[test]
    fn test_u24_wrapping_add() {
        assert_eq!(u24::new(0x00FFFFFE).wrapping_add(1).get(), 0x00FFFFFF);
        assert_eq!(u24::new(0x00FFFFFF).wrapping_add(1).get(), 0);
        assert_eq!(u24::new(0x00FFFFFF).wrapping_add(2).get(), 1);
    }

    #[test]
    fn test_u24_wrapping_sub() {
        assert_eq!(u24::new(1).wrapping_sub(1).get(), 0);
        assert_eq!(u24::new(0).wrapping_sub(1).get(), 0x00FFFFFF);
        assert_eq!(u24::new(0).wrapping_sub(2).get(), 0x00FFFFFE);
    }

    #[test]
    fn test_atomic_u24() {
        let atomic = AtomicU24::new(0);
        assert_eq!(atomic.load(Ordering::Relaxed).get(), 0);

        atomic.store(u24::new(42), Ordering::Relaxed);
        assert_eq!(atomic.load(Ordering::Relaxed).get(), 42);

        let old = atomic.fetch_add(10, Ordering::Relaxed);
        assert_eq!(old.get(), 42);
        assert_eq!(atomic.load(Ordering::Relaxed).get(), 52);
    }

    #[test]
    fn test_atomic_u24_wraparound() {
        let atomic = AtomicU24::new(0x00FFFFFF);
        let old = atomic.fetch_add(1, Ordering::Relaxed);
        assert_eq!(old.get(), 0x00FFFFFF);
        assert_eq!(atomic.load(Ordering::Relaxed).get(), 0);
    }

    #[test]
    fn test_write_read_le() {
        let mut buf = [0u8; 3];
        write_u24_le(&mut buf, u24::new(0x123456));
        assert_eq!(buf, [0x56, 0x34, 0x12]);
        assert_eq!(read_u24_le(&buf).get(), 0x123456);
    }

    #[test]
    fn test_write_read_be() {
        let mut buf = [0u8; 3];
        write_u24_be(&mut buf, u24::new(0x123456));
        assert_eq!(buf, [0x12, 0x34, 0x56]);
        assert_eq!(read_u24_be(&buf).get(), 0x123456);
    }

    #[test]
    fn test_seq_less_than() {
        assert!(seq_less_than(0, 1));
        assert!(seq_less_than(100, 200));
        assert!(!seq_less_than(200, 100));

        // Wraparound cases
        assert!(seq_less_than(0x00FFFFFF, 0));
        assert!(seq_less_than(0x00FFFFFE, 5));
        assert!(!seq_less_than(5, 0x00FFFFFE));
    }

    #[test]
    fn test_seq_in_range() {
        // Normal range
        assert!(seq_in_range(50, 0, 100));
        assert!(seq_in_range(0, 0, 100));
        assert!(seq_in_range(100, 0, 100));
        assert!(!seq_in_range(101, 0, 100));

        // Wraparound range
        assert!(seq_in_range(0x00FFFFFE, 0x00FFFFFA, 10));
        assert!(seq_in_range(0x00FFFFFF, 0x00FFFFFA, 10));
        assert!(seq_in_range(0, 0x00FFFFFA, 10));
        assert!(seq_in_range(5, 0x00FFFFFA, 10));
        assert!(seq_in_range(10, 0x00FFFFFA, 10));
        assert!(!seq_in_range(50, 0x00FFFFFA, 10));
    }
}
