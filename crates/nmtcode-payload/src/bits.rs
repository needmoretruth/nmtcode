//! Bit strings, most significant bit first (01-scope-and-conventions.md, 1.4).

use alloc::vec::Vec;

use crate::CodecError;

/// The least significant byte of `x`.
pub(crate) fn low_byte(x: u64) -> u8 {
    x.to_le_bytes()[0]
}

/// Appends fields of up to 32 bits, most significant bit first.
pub(crate) struct BitWriter {
    bytes: Vec<u8>,
    /// Pending bits, right-aligned; fewer than 8 between calls.
    acc: u64,
    pending: u32,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        Self { bytes: Vec::new(), acc: 0, pending: 0 }
    }

    /// Appends the low `width` bits of `value` (`width` ≤ 32).
    pub(crate) fn put(&mut self, value: u32, width: u32) {
        let width = width.min(32);
        let mask = if width == 32 { u32::MAX } else { (1u32 << width) - 1 };
        self.acc = (self.acc << width) | u64::from(value & mask);
        self.pending += width;
        while self.pending >= 8 {
            self.pending -= 8;
            self.bytes.push(low_byte(self.acc >> self.pending));
        }
        self.acc &= (1u64 << self.pending) - 1;
    }

    /// Pads with 0 bits to a byte boundary and returns the bytes.
    pub(crate) fn finish(mut self) -> Vec<u8> {
        if self.pending > 0 {
            self.bytes.push(low_byte(self.acc << (8 - self.pending)));
        }
        self.bytes
    }
}

/// Reads fields of up to 32 bits, most significant bit first. A read past the
/// end is `E_MALFORMED`.
pub(crate) struct BitReader<'a> {
    bytes: &'a [u8],
    /// Position in bits.
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn total_bits(&self) -> usize {
        self.bytes.len().saturating_mul(8)
    }

    /// Bits not yet read.
    pub(crate) fn remaining(&self) -> usize {
        self.total_bits().saturating_sub(self.pos)
    }

    /// Reads `width` bits (`width` ≤ 32).
    pub(crate) fn read(&mut self, width: u32) -> Result<u32, CodecError> {
        let width = width.min(32);
        let needed = usize::try_from(width).map_err(|_| CodecError::Malformed)?;
        if needed > self.remaining() {
            return Err(CodecError::Malformed);
        }
        let mut value = 0u32;
        let mut left = width;
        while left > 0 {
            let byte = self.bytes.get(self.pos / 8).copied().ok_or(CodecError::Malformed)?;
            let used = u32::try_from(self.pos % 8).map_err(|_| CodecError::Malformed)?;
            let available = 8 - used;
            let take = available.min(left);
            // The `take` bits of `byte` that start `used` bits from its top.
            let bits = (u32::from(byte) >> (available - take)) & ((1u32 << take) - 1);
            // `take` ≤ 8 and at most 32 bits are gathered, so nothing is lost.
            value = (value << take) | bits;
            left -= take;
            self.pos += usize::try_from(take).map_err(|_| CodecError::Malformed)?;
        }
        Ok(value)
    }

    /// True when fewer than 8 bits remain and all of them are 0: the padding
    /// rule of codecs 1, 2 and 4.
    pub(crate) fn only_zero_padding_left(&self) -> bool {
        let remaining = self.remaining();
        if remaining == 0 {
            return true;
        }
        if remaining >= 8 {
            return false;
        }
        let Some(&last) = self.bytes.last() else {
            return false;
        };
        let mask = (1u16 << remaining) - 1;
        u16::from(last) & mask == 0
    }
}

#[cfg(test)]
mod tests {
    use super::{BitReader, BitWriter};

    #[test]
    fn writer_and_reader_agree() {
        let mut w = BitWriter::new();
        w.put(0b101, 3);
        w.put(0x3FFF, 14);
        w.put(0, 1);
        w.put(0xDEAD_BEEF, 32);
        let bytes = w.finish();
        assert_eq!(bytes.len(), 7);
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read(3), Ok(0b101));
        assert_eq!(r.read(14), Ok(0x3FFF));
        assert_eq!(r.read(1), Ok(0));
        assert_eq!(r.read(32), Ok(0xDEAD_BEEF));
        assert_eq!(r.remaining(), 6);
        assert!(r.only_zero_padding_left());
        assert!(r.read(7).is_err());
    }
}
