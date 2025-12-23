/// RakNet magic constant.
///
/// This 16-byte sequence is included in all unconnected packets to identify RakNet traffic.
/// It helps distinguish RakNet packets from other UDP traffic and provides version identification.
pub const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00,
    0xfe, 0xfe, 0xfe, 0xfe,
    0xfd, 0xfd, 0xfd, 0xfd,
    0x12, 0x34, 0x56, 0x78,
];

/// Checks if a buffer contains the RakNet magic at the given offset.
#[inline]
pub fn check_magic(buf: &[u8], offset: usize) -> bool {
    if buf.len() < offset + MAGIC.len() {
        return false;
    }
    &buf[offset..offset + MAGIC.len()] == &MAGIC
}

/// Writes the RakNet magic to a buffer at the given offset.
#[inline]
pub fn write_magic(buf: &mut [u8], offset: usize) {
    debug_assert!(
        buf.len() >= offset + MAGIC.len(),
        "Buffer too small for magic"
    );
    buf[offset..offset + MAGIC.len()].copy_from_slice(&MAGIC);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_constant() {
        assert_eq!(MAGIC.len(), 16);
        assert_eq!(
            MAGIC,
            [
                0x00, 0xff, 0xff, 0x00,
                0xfe, 0xfe, 0xfe, 0xfe,
                0xfd, 0xfd, 0xfd, 0xfd,
                0x12, 0x34, 0x56, 0x78,
            ]
        );
    }

    #[test]
    fn test_check_magic() {
        let mut buf = vec![0u8; 32];
        write_magic(&mut buf, 8);

        assert!(check_magic(&buf, 8));
        assert!(!check_magic(&buf, 0));
        assert!(!check_magic(&buf, 7));
    }

    #[test]
    fn test_write_magic() {
        let mut buf = vec![0u8; 16];
        write_magic(&mut buf, 0);
        assert_eq!(&buf[..], &MAGIC);
    }

    #[test]
    fn test_check_magic_buffer_too_small() {
        let buf = vec![0u8; 10];
        assert!(!check_magic(&buf, 0));
    }
}
