//! The whitening sequence (5.10).

/// The seed of the base layer's whitening sequence (5.10.1): w\[0\] … w\[30\] are its 31 bits,
/// most significant first.
pub const WHITENING_SEED: u32 = 0x15D9_C3FC;

/// The 31 bits of the register.
const REGISTER: u32 = 0x7FFF_FFFF;

/// The whitening sequence w\[0\], w\[1\], … of 5.10.1: w\[k\] = w\[k − 28\] XOR w\[k − 31\].
///
/// It is the register form of 5.10.1: a 31-bit register whose bit 30 is the next output.
/// [`Whitening::next_bit`] yields one bit and [`Whitening::next_byte`] yields the next eight
/// with the first as the most significant bit; both advance the same register. The iterator
/// never ends (the period is 2^31 − 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Whitening {
    register: u32,
}

impl Whitening {
    /// The base layer's sequence, starting at w\[0\].
    pub const fn new() -> Self {
        Self::with_seed(WHITENING_SEED)
    }

    /// The same generator from another seed: its low 31 bits are the first 31 outputs, most
    /// significant first. The colour layer uses the seed 0x644E9D0D (chapter 7, 7.8.4).
    pub const fn with_seed(seed: u32) -> Self {
        Self { register: seed & REGISTER }
    }

    /// The next bit, `true` for 1.
    pub fn next_bit(&mut self) -> bool {
        let r = self.register;
        let out = r >> 30 & 1;
        let feedback = (r >> 30 ^ r >> 27) & 1;
        self.register = (r << 1 & REGISTER) | feedback;
        out == 1
    }

    /// The next eight bits, the first as bit 7.
    pub fn next_byte(&mut self) -> u8 {
        let r = self.register;
        // Bit 30 − j of the register is output j; eight steps of the recurrence need outputs
        // j and j + 3 for j = 0 … 7, which are all in the register already.
        let [_, _, _, out] = (r >> 23).to_be_bytes();
        let feedback = (r >> 23 ^ r >> 20) & 0xFF;
        self.register = (r << 8 & REGISTER) | feedback;
        out
    }
}

impl Default for Whitening {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for Whitening {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        Some(self.next_bit())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
