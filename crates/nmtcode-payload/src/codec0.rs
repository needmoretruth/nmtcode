//! Codec 0 — stored (6.5). The coded field is the content; `L` = `Lc`.

use alloc::vec::Vec;

use crate::{CodecError, check_decoded_len, len_matches};

/// Codec ID.
pub const ID: u32 = 0;

/// The coded field for `content`: the content itself. Every content is applicable.
pub fn encode(content: &[u8]) -> Vec<u8> {
    content.to_vec()
}

/// Decodes a stored coded field.
///
/// `decoded_len` is `L`, which for codec 0 the container derives from `Lc`.
///
/// # Errors
///
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`],
///   or `decoded_len` ≠ `bytes.len()`.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(decoded_len: u32, bytes: &[u8], limit: u32) -> Result<Vec<u8>, CodecError> {
    check_decoded_len(decoded_len, limit)?;
    if !len_matches(bytes.len(), u64::from(decoded_len)) {
        return Err(CodecError::Malformed);
    }
    Ok(bytes.to_vec())
}
