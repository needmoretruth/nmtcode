//! CRC-32C (Castagnoli) as the specification defines it (chapter 1, 1.4; chapter 3, 3.7).

use crc::{CRC_32_ISCSI, Crc};

/// The CRC-32C of the ASCII string `123456789` (chapter 1, 1.4).
pub const CRC32C_CHECK: u32 = 0xE306_9283;

/// Number of bytes the CRC-32C takes in a container (chapter 3, 3.7).
pub const CRC32C_LEN: usize = 4;

static CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

/// CRC-32C of `bytes`: polynomial 0x1EDC6F41, reflected input and output, initial value and final
/// XOR 0xFFFFFFFF (RFC 3720). A container writes it most significant byte first
/// (`u32::to_be_bytes`).
pub fn crc32c(bytes: &[u8]) -> u32 {
    CASTAGNOLI.checksum(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_value() {
        assert_eq!(crc32c(b"123456789"), CRC32C_CHECK);
        assert_eq!(CRC32C_CHECK.to_be_bytes(), [0xE3, 0x06, 0x92, 0x83]);
    }
}
