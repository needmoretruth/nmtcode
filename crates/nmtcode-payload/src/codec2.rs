//! Codec 2 — upper-case alphanumeric (6.7): the 45-character set of QR
//! alphanumeric mode, two characters in 11 bits.

use alloc::vec::Vec;

use crate::bits::{BitReader, BitWriter};
use crate::{CodecError, check_decoded_len, len_matches, to_usize};

/// Codec ID.
pub const ID: u32 = 2;

/// The 45 characters in value order (6.7).
pub const ALPHABET: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// The value of `byte` in [`ALPHABET`], or `None` when it is not in the set.
pub fn value_of(byte: u8) -> Option<u32> {
    ALPHABET.iter().position(|&c| c == byte).and_then(|index| u32::try_from(index).ok())
}

/// True when every byte is in the 45-character set.
pub fn is_applicable(content: &[u8]) -> bool {
    content.iter().all(|&byte| value_of(byte).is_some())
}

/// `B` of 6.7 step 3 for `decoded_len` characters.
pub const fn bit_len(decoded_len: u64) -> u64 {
    11 * (decoded_len / 2) + 6 * (decoded_len % 2)
}

/// `Lc` = ⌈`B`/8⌉ for `decoded_len` characters.
pub const fn coded_len(decoded_len: u64) -> u64 {
    bit_len(decoded_len).div_ceil(8)
}

/// The coded field, or `None` when `content` has a byte outside the set.
pub fn encode(content: &[u8]) -> Option<Vec<u8>> {
    let mut writer = BitWriter::new();
    for chunk in content.chunks(2) {
        match *chunk {
            [a, b] => writer.put(45 * value_of(a)? + value_of(b)?, 11),
            [a] => writer.put(value_of(a)?, 6),
            _ => return None,
        }
    }
    Some(writer.finish())
}

fn character(value: u32) -> Result<u8, CodecError> {
    let index = usize::try_from(value).map_err(|_| CodecError::Malformed)?;
    ALPHABET.get(index).copied().ok_or(CodecError::Malformed)
}

/// Decodes an alphanumeric coded field of `decoded_len` characters.
///
/// # Errors
///
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   `bytes.len()` ≠ ⌈`B`/8⌉ (checked before allocating); a pair value > 2024;
///   a single value > 44; a non-zero padding bit.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(decoded_len: u32, bytes: &[u8], limit: u32) -> Result<Vec<u8>, CodecError> {
    check_decoded_len(decoded_len, limit)?;
    if !len_matches(bytes.len(), coded_len(u64::from(decoded_len))) {
        return Err(CodecError::Malformed);
    }
    let total = to_usize(decoded_len)?;
    let mut out = Vec::with_capacity(total);
    let mut reader = BitReader::new(bytes);
    while total - out.len() >= 2 {
        let pair = reader.read(11)?;
        if pair > 2024 {
            return Err(CodecError::Malformed);
        }
        out.push(character(pair / 45)?);
        out.push(character(pair % 45)?);
    }
    if out.len() < total {
        let single = reader.read(6)?;
        if single > 44 {
            return Err(CodecError::Malformed);
        }
        out.push(character(single)?);
    }
    if !reader.only_zero_padding_left() {
        return Err(CodecError::Malformed);
    }
    Ok(out)
}
