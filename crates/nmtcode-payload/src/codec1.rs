//! Codec 1 — digits (6.6): three digits in 10 bits, as QR numeric mode.

use alloc::vec::Vec;

use crate::bits::{BitReader, BitWriter, low_byte};
use crate::{CodecError, check_decoded_len, len_matches, to_usize};

/// Codec ID.
pub const ID: u32 = 1;

/// True when every byte is an ASCII digit (0x30–0x39).
pub fn is_applicable(content: &[u8]) -> bool {
    content.iter().all(u8::is_ascii_digit)
}

/// `B` of 6.6 step 3: the number of bits for `decoded_len` digits.
pub const fn bit_len(decoded_len: u64) -> u64 {
    10 * (decoded_len / 3)
        + match decoded_len % 3 {
            0 => 0,
            1 => 4,
            _ => 7,
        }
}

/// `Lc` = ⌈`B`/8⌉ for `decoded_len` digits.
pub const fn coded_len(decoded_len: u64) -> u64 {
    bit_len(decoded_len).div_ceil(8)
}

/// Field width and largest valid value for a group of `digits` digits.
const fn group(digits: usize) -> (u32, u32) {
    match digits {
        3 => (10, 999),
        2 => (7, 99),
        _ => (4, 9),
    }
}

/// The coded field, or `None` when `content` is not all digits.
pub fn encode(content: &[u8]) -> Option<Vec<u8>> {
    if !is_applicable(content) {
        return None;
    }
    let mut writer = BitWriter::new();
    for chunk in content.chunks(3) {
        let value = chunk.iter().fold(0u32, |acc, &digit| acc * 10 + u32::from(digit - b'0'));
        writer.put(value, group(chunk.len()).0);
    }
    Some(writer.finish())
}

/// Decodes a digits coded field of `decoded_len` digits.
///
/// # Errors
///
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   `bytes.len()` ≠ ⌈`B`/8⌉ (checked before allocating); a group value out of
///   range; a non-zero padding bit.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(decoded_len: u32, bytes: &[u8], limit: u32) -> Result<Vec<u8>, CodecError> {
    check_decoded_len(decoded_len, limit)?;
    if !len_matches(bytes.len(), coded_len(u64::from(decoded_len))) {
        return Err(CodecError::Malformed);
    }
    let total = to_usize(decoded_len)?;
    let mut out = Vec::with_capacity(total);
    let mut reader = BitReader::new(bytes);
    while out.len() < total {
        let digits = (total - out.len()).min(3);
        let (width, max) = group(digits);
        let value = reader.read(width)?;
        if value > max {
            return Err(CodecError::Malformed);
        }
        let mut divisor = match digits {
            3 => 100,
            2 => 10,
            _ => 1,
        };
        while divisor > 0 {
            out.push(b'0' + low_byte(u64::from(value / divisor % 10)));
            divisor /= 10;
        }
    }
    if !reader.only_zero_padding_left() {
        return Err(CodecError::Malformed);
    }
    Ok(out)
}
