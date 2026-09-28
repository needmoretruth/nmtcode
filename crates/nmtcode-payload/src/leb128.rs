//! Unsigned LEB128 as defined in 01-scope-and-conventions.md (1.4): at most
//! 5 bytes, value below 2^32, minimal encoding only.
//!
//! In this layer LEB128 appears only inside short-text model dictionaries
//! (6.8.4); the container reads `c`, `d` and `L` with the same rules.

use alloc::vec::Vec;

use crate::CodecError;

/// The most bytes a LEB128 value may take.
pub const MAX_LEN: usize = 5;

/// Reads one LEB128 value from the start of `bytes` and returns it with the
/// number of bytes it took.
///
/// # Errors
///
/// [`CodecError::Malformed`] when the value runs past the end of `bytes`,
/// takes more than 5 bytes, is 2^32 or more, or is not minimal (a multi-byte
/// encoding whose last byte is `00`).
pub fn read(bytes: &[u8]) -> Result<(u32, usize), CodecError> {
    let mut value: u64 = 0;
    for (index, &byte) in bytes.iter().take(MAX_LEN).enumerate() {
        let shift = 7 * u32::try_from(index).map_err(|_| CodecError::Malformed)?;
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            if index > 0 && byte == 0 {
                return Err(CodecError::Malformed);
            }
            let value = u32::try_from(value).map_err(|_| CodecError::Malformed)?;
            return Ok((value, index + 1));
        }
    }
    Err(CodecError::Malformed)
}

/// Appends the minimal LEB128 encoding of `value`.
pub fn write(value: u32, out: &mut Vec<u8>) {
    let mut rest = value;
    loop {
        let byte = rest.to_le_bytes()[0] & 0x7F;
        rest >>= 7;
        if rest == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// The number of bytes of the LEB128 encoding of `value` (1 to 5).
pub const fn encoded_len(value: u32) -> usize {
    match value {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1F_FFFF => 3,
        0x20_0000..=0x0FFF_FFFF => 4,
        _ => 5,
    }
}
