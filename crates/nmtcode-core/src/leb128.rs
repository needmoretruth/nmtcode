//! Unsigned LEB128 as the specification defines it (chapter 1, 1.4; chapter 3, 3.1).
//!
//! The first byte holds the least significant 7 bits; bit 7 (0x80) of a byte is set when another
//! byte follows. A value is below 2^32 and takes at most 5 bytes. A decoder rejects a non-minimal
//! encoding and a value of 2^32 or more.

use alloc::vec::Vec;

/// Largest number of bytes of one LEB128 value (a value below 2^32).
pub const LEB128_MAX_LEN: usize = 5;

/// Why [`read_leb128`] refused its input.
///
/// The container maps these to the reader errors of chapter 9 (9.8) by the field they occur in:
/// in the body length Lb all three are [`crate::Error::LengthField`]; in the body a
/// [`Leb128Error::Truncated`] value is [`crate::Error::HeaderOverrun`] and the other two are
/// [`crate::Error::Leb128`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leb128Error {
    /// The input ended while bit 7 said another byte follows.
    Truncated,
    /// The last byte is `00` after at least one other byte: a shorter encoding exists.
    NonMinimal,
    /// The value is 2^32 or more, or the encoding is longer than 5 bytes.
    TooLarge,
}

/// Number of bytes of the LEB128 encoding of `value` (1 to 5).
pub const fn leb128_len(value: u32) -> usize {
    match value {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1F_FFFF => 3,
        0x20_0000..=0x0FFF_FFFF => 4,
        _ => 5,
    }
}

/// Appends the minimal LEB128 encoding of `value` to `out`.
pub fn write_leb128(out: &mut Vec<u8>, value: u32) {
    let mut v = value;
    loop {
        let [first, ..] = v.to_le_bytes();
        let low = first & 0x7F;
        v >>= 7;
        if v == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

/// Reads one LEB128 value from the start of `bytes`.
///
/// Returns the value and the number of bytes it took.
///
/// # Errors
///
/// [`Leb128Error`] when the input ends inside the value, the encoding is not minimal, or the value
/// is 2^32 or more.
pub fn read_leb128(bytes: &[u8]) -> Result<(u32, usize), Leb128Error> {
    let mut value: u32 = 0;
    for i in 0..LEB128_MAX_LEN {
        let Some(&byte) = bytes.get(i) else {
            return Err(Leb128Error::Truncated);
        };
        let low = u32::from(byte & 0x7F);
        // The fifth byte holds bits 28 to 31: only its low 4 bits may be set, and it must be last.
        if i == LEB128_MAX_LEN - 1 && (byte & 0x80 != 0 || low > 0x0F) {
            return Err(Leb128Error::TooLarge);
        }
        value |= low << (7 * i);
        if byte & 0x80 == 0 {
            if i > 0 && byte == 0 {
                return Err(Leb128Error::NonMinimal);
            }
            return Ok((value, i + 1));
        }
    }
    // Unreachable: the fifth byte either returned a value or an error above.
    Err(Leb128Error::TooLarge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_of_3_1() {
        let cases: [(u32, &[u8]); 6] = [
            (0, &[0x00]),
            (127, &[0x7F]),
            (128, &[0x80, 0x01]),
            (300, &[0xAC, 0x02]),
            (16_384, &[0x80, 0x80, 0x01]),
            (u32::MAX, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
        ];
        for (value, bytes) in cases {
            let mut out = Vec::new();
            write_leb128(&mut out, value);
            assert_eq!(out, bytes, "{value}");
            assert_eq!(read_leb128(bytes), Ok((value, bytes.len())));
        }
    }

    #[test]
    fn lengths_match_encodings() {
        for v in [
            0,
            1,
            0x7F,
            0x80,
            0x3FFF,
            0x4000,
            0x1F_FFFF,
            0x20_0000,
            0x0FFF_FFFF,
            0x1000_0000,
            u32::MAX,
        ] {
            let mut out = Vec::new();
            write_leb128(&mut out, v);
            assert_eq!(out.len(), leb128_len(v), "{v}");
            assert_eq!(read_leb128(&out), Ok((v, out.len())));
        }
    }

    #[test]
    fn rejects() {
        assert_eq!(read_leb128(&[]), Err(Leb128Error::Truncated));
        assert_eq!(read_leb128(&[0x80]), Err(Leb128Error::Truncated));
        assert_eq!(read_leb128(&[0x80, 0x00]), Err(Leb128Error::NonMinimal));
        assert_eq!(read_leb128(&[0xFF, 0x80, 0x00]), Err(Leb128Error::NonMinimal));
        assert_eq!(read_leb128(&[0x80, 0x80, 0x80, 0x80, 0x00]), Err(Leb128Error::NonMinimal));
        // 2^32 = 80 80 80 80 10.
        assert_eq!(read_leb128(&[0x80, 0x80, 0x80, 0x80, 0x10]), Err(Leb128Error::TooLarge));
        assert_eq!(read_leb128(&[0xFF, 0xFF, 0xFF, 0xFF, 0x1F]), Err(Leb128Error::TooLarge));
        // Six bytes.
        assert_eq!(read_leb128(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x00]), Err(Leb128Error::TooLarge));
        // Trailing bytes after a value are not read.
        assert_eq!(read_leb128(&[0x05, 0xFF]), Ok((5, 1)));
    }
}
