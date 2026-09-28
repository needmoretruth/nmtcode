//! The format word of chapter 2: its 28 data bits (2.2, 2.3), the BCH (47, 28) code that protects
//! it (2.4), its mask (2.5) and how a reader decodes and chooses between its copies (2.7).
//!
//! Placing the bits on modules (2.6, chapter 5) is not done here.
//!
//! # Bit order of a codeword
//!
//! A format codeword is the low 47 bits of a `u64`; bits 47 to 63 are 0. Bit index `i` of 2.4.3
//! (0 ≤ i ≤ 46, the coefficient of x^(46 − i)) is integer bit `46 − i`, so index 0 is the most
//! significant of the 47 bits and is sent first:
//!
//! - integer bits 46 to 19 (indices 0 to 27) are d\[27\] … d\[0\];
//! - integer bits 18 to 0 (indices 28 to 46) are the 19 parity bits, most significant first.
//!
//! Module A\[i\] of copy A and module B\[i\] of copy B carry index `i`, that is
//! `(word >> (46 - i)) & 1`, dark for 1 (2.6).

use alloc::vec::Vec;

use crate::{Error, is_valid_side};

/// Number of data bits of the format word (2.2).
pub const FORMAT_DATA_BITS: u32 = 28;
/// Number of parity bits of the format word (2.4.1).
pub const FORMAT_PARITY_BITS: u32 = 19;
/// Number of bits of a format codeword, and of modules in each of its two copies (2.4.1, 2.6).
pub const FORMAT_CODEWORD_BITS: u32 = 47;
/// The generator polynomial g(x) as an integer, bit i the coefficient of x^i (2.4.1).
pub const FORMAT_GENERATOR: u32 = 0x8_8751;
/// The mask that is XOR-ed onto the codeword before it is sent (2.5).
pub const FORMAT_MASK: u64 = 0x51F3_694E_AFAA;
/// The most bit errors a reader corrects in one copy (2.7, step 2).
pub const FORMAT_MAX_ERRORS: u32 = 3;
/// The format version of this specification (2.2, 9.2). Versions 1 to 3 are reserved.
pub const FORMAT_VERSION: u8 = 0;

/// The low 47 bits of a `u64`.
const CODEWORD_MASK: u64 = (1 << FORMAT_CODEWORD_BITS) - 1;
const DATA_MASK: u32 = (1 << FORMAT_DATA_BITS) - 1;
const PARITY_MASK: u32 = (1 << FORMAT_PARITY_BITS) - 1;
/// g(x) without its x^19 term (2.4.3).
const GENERATOR_LOW: u32 = FORMAT_GENERATOR & PARITY_MASK;
/// Bit 18 of the shift register: the coefficient that becomes x^19 on the next shift.
const PARITY_TOP: u32 = 1 << (FORMAT_PARITY_BITS - 1);

/// The symbol class of the format word, d\[25\] (2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolClass {
    /// 0: a static single code. Its container is that of chapter 3, 3.2.
    Static,
    /// 1: a transfer tile. Its container is that of chapter 3, 3.3, defined by a later 0.x
    /// version; a reader without transfer support reports it as unsupported.
    TransferTile,
}

impl SymbolClass {
    /// The value of the class bit: 0 for [`SymbolClass::Static`], 1 for
    /// [`SymbolClass::TransferTile`].
    pub const fn bit(self) -> u32 {
        match self {
            Self::Static => 0,
            Self::TransferTile => 1,
        }
    }
}

/// Why [`FormatWord::new`] refused a combination of fields (2.3; 3.3 for transfer tiles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatWordError {
    /// The width is not a multiple of 4 from 20 to 4108.
    Width(u32),
    /// The height is not a multiple of 4 from 20 to 4108.
    Height(u32),
    /// The error-correction level is not 0 to 3.
    Level(u8),
    /// The colour profile is not 0 or 1 (2 and 3 are reserved).
    ColourProfile(u8),
    /// The chroma cell size code is not 0 or 1.
    ChromaCell(u8),
    /// Chroma cell size 1 with colour profile 0 (2.2, 2.3).
    ChromaCellWithoutColour,
    /// A transfer tile with a colour profile other than 0 (3.3, 7.3).
    TransferTileColour,
}

impl core::fmt::Display for FormatWordError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Width(w) => write!(f, "width {w} is not a multiple of 4 from 20 to 4108"),
            Self::Height(h) => write!(f, "height {h} is not a multiple of 4 from 20 to 4108"),
            Self::Level(l) => write!(f, "error-correction level {l} is not 0 to 3"),
            Self::ColourProfile(c) => write!(f, "colour profile {c} is not 0 or 1"),
            Self::ChromaCell(c) => write!(f, "chroma cell size code {c} is not 0 or 1"),
            Self::ChromaCellWithoutColour => {
                f.write_str("chroma cell size 1 requires a colour profile other than 0")
            }
            Self::TransferTileColour => f.write_str("a transfer tile must have colour profile 0"),
        }
    }
}

impl core::error::Error for FormatWordError {}

/// The fields of a format word (2.2) whose values a reader accepts (2.3).
///
/// A value of this type always holds format version 0, a width and height that are multiples
/// of 4 from 20 to 4108, level 0 to 3, colour profile 0 or 1, chroma cell size 0 unless the
/// colour profile is 1, and colour profile 0 for a transfer tile. [`FormatWord::new`] and
/// [`decode_format`] are the only ways to make one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormatWord {
    version: u8,
    class: SymbolClass,
    width: u32,
    height: u32,
    level: u8,
    colour_profile: u8,
    chroma_cell: u8,
}

impl FormatWord {
    /// A format word of format version 0 with the given fields.
    ///
    /// `width` and `height` are in modules. `level` is the error-correction level (0 to 3),
    /// `colour_profile` is 0 (black and white) or 1 (chapter 7), and `chroma_cell` is the chroma
    /// cell size code: 0 for 1 × 1 modules, 1 for 2 × 2 modules.
    ///
    /// # Errors
    ///
    /// [`FormatWordError`] when a field holds a value a generator must not write (2.3), or when a
    /// transfer tile has a colour profile other than 0 (3.3).
    pub const fn new(
        class: SymbolClass,
        width: u32,
        height: u32,
        level: u8,
        colour_profile: u8,
        chroma_cell: u8,
    ) -> Result<Self, FormatWordError> {
        if !is_valid_side(width) {
            return Err(FormatWordError::Width(width));
        }
        if !is_valid_side(height) {
            return Err(FormatWordError::Height(height));
        }
        if level > 3 {
            return Err(FormatWordError::Level(level));
        }
        if colour_profile > 1 {
            return Err(FormatWordError::ColourProfile(colour_profile));
        }
        if chroma_cell > 1 {
            return Err(FormatWordError::ChromaCell(chroma_cell));
        }
        if colour_profile == 0 && chroma_cell == 1 {
            return Err(FormatWordError::ChromaCellWithoutColour);
        }
        if matches!(class, SymbolClass::TransferTile) && colour_profile != 0 {
            return Err(FormatWordError::TransferTileColour);
        }
        Ok(Self {
            version: FORMAT_VERSION,
            class,
            width,
            height,
            level,
            colour_profile,
            chroma_cell,
        })
    }

    /// The fields of a decoded data word whose version-0 fields passed step 3 of 2.7.
    ///
    /// Applies 2.3 to the chosen word: format version 1 to 3 gives [`Error::FormatVersion`], and
    /// a transfer tile with a colour profile other than 0 gives [`Error::TileColour`] (3.3, 7.3).
    fn from_chosen(data: u32) -> Result<Self, Error> {
        if field(data, 26, 2) != 0 {
            return Err(Error::FormatVersion);
        }
        let class =
            if field(data, 25, 1) == 1 { SymbolClass::TransferTile } else { SymbolClass::Static };
        let colour_profile = field_u8(data, 1, 2);
        if matches!(class, SymbolClass::TransferTile) && colour_profile != 0 {
            return Err(Error::TileColour);
        }
        Ok(Self {
            version: FORMAT_VERSION,
            class,
            width: 4 * (field(data, 15, 10) + 4),
            height: 4 * (field(data, 5, 10) + 4),
            level: field_u8(data, 3, 2),
            colour_profile,
            chroma_cell: field_u8(data, 0, 1),
        })
    }

    /// The format version, d\[27..26\]. Always 0 (this specification).
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// The symbol class, d\[25\].
    pub const fn class(&self) -> SymbolClass {
        self.class
    }

    /// The width W in modules.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The height H in modules.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The width code w = W / 4 − 4, d\[24..15\] (1 to 1023).
    pub const fn width_code(&self) -> u32 {
        self.width / 4 - 4
    }

    /// The height code h = H / 4 − 4, d\[14..5\] (1 to 1023).
    pub const fn height_code(&self) -> u32 {
        self.height / 4 - 4
    }

    /// The error-correction level, d\[4..3\] (0 to 3).
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// The colour profile, d\[2..1\]: 0 for black and white, 1 for chapter 7's profile 1.
    pub const fn colour_profile(&self) -> u8 {
        self.colour_profile
    }

    /// The chroma cell size code, d\[0\]: 0 for 1 × 1 modules, 1 for 2 × 2 modules.
    pub const fn chroma_cell(&self) -> u8 {
        self.chroma_cell
    }

    /// The side of a chroma cell in modules: 1 or 2 (7.3).
    pub const fn chroma_cell_side(&self) -> u32 {
        if self.chroma_cell == 1 { 2 } else { 1 }
    }

    /// The 28-bit data word d of 2.2.
    pub fn data(&self) -> u32 {
        (u32::from(self.version) << 26)
            | (self.class.bit() << 25)
            | (self.width_code() << 15)
            | (self.height_code() << 5)
            | (u32::from(self.level) << 3)
            | (u32::from(self.colour_profile) << 1)
            | u32::from(self.chroma_cell)
    }

    /// The codeword C = (d << 19) | P before the mask (2.4.3).
    pub fn codeword(&self) -> u64 {
        format_codeword(self.data())
    }

    /// The sent word F = C XOR `FORMAT_MASK` (2.5): the value both copies carry.
    ///
    /// Bit order: see the module documentation. Index `i` of 2.4.3, which goes on modules A\[i\]
    /// and B\[i\] (2.6), is `(word >> (46 - i)) & 1`.
    pub fn encode(&self) -> u64 {
        self.codeword() ^ FORMAT_MASK
    }
}

/// The 19 parity bits P = (d(x) · x^19) mod g(x) of a data word (2.4.3).
///
/// Bits of `data` above bit 27 are ignored.
pub const fn format_parity(data: u32) -> u32 {
    let data = data & DATA_MASK;
    let mut rem: u32 = 0;
    let mut i = FORMAT_DATA_BITS;
    while i > 0 {
        i -= 1;
        let feedback = ((data >> i) ^ (rem >> (FORMAT_PARITY_BITS - 1))) & 1;
        rem = (rem << 1) & PARITY_MASK;
        if feedback == 1 {
            rem ^= GENERATOR_LOW;
        }
    }
    rem
}

/// The codeword C = (d << 19) | P of any 28-bit data word, before the mask (2.4.3).
///
/// Bits of `data` above bit 27 are ignored. XOR the result with [`FORMAT_MASK`] to get the sent
/// word. Unlike [`FormatWord::encode`] this accepts reserved and invalid field values, so that
/// tests can build the words a reader must reject.
pub fn format_codeword(data: u32) -> u64 {
    let data = data & DATA_MASK;
    (u64::from(data) << FORMAT_PARITY_BITS) | u64::from(format_parity(data))
}

/// The syndrome R mod g(x) of the low 47 bits of `word` (2.7).
///
/// Zero exactly when those bits are a codeword.
pub const fn format_syndrome(word: u64) -> u32 {
    let mut rem: u32 = 0;
    let mut i = FORMAT_CODEWORD_BITS;
    while i > 0 {
        i -= 1;
        let top = rem & PARITY_TOP != 0;
        rem = (rem << 1) & PARITY_MASK;
        if (word >> i) & 1 == 1 {
            rem |= 1;
        }
        if top {
            rem ^= GENERATOR_LOW;
        }
    }
    rem
}

/// The syndrome of every single-bit error pattern with that pattern, sorted by syndrome.
const SINGLE_ERRORS: [(u32, u64); FORMAT_CODEWORD_BITS as usize] = single_errors();

const fn single_errors() -> [(u32, u64); FORMAT_CODEWORD_BITS as usize] {
    let mut table = [(0, 0); FORMAT_CODEWORD_BITS as usize];
    let mut j = 0;
    while j < table.len() {
        let pattern = 1u64 << j;
        table[j] = (format_syndrome(pattern), pattern);
        j += 1;
    }
    // Insertion sort by syndrome, so that a lookup is a binary search.
    let mut i = 1;
    while i < table.len() {
        let mut k = i;
        while k > 0 && table[k - 1].0 > table[k].0 {
            let tmp = table[k - 1];
            table[k - 1] = table[k];
            table[k] = tmp;
            k -= 1;
        }
        i += 1;
    }
    table
}

/// The single-bit error pattern whose syndrome is `syndrome`, if there is one.
fn single_error(syndrome: u32) -> Option<u64> {
    let index = SINGLE_ERRORS.binary_search_by_key(&syndrome, |&(s, _)| s).ok()?;
    SINGLE_ERRORS.get(index).map(|&(_, pattern)| pattern)
}

/// The error pattern of weight 0 to 3 with this syndrome, and its weight.
///
/// Such a pattern is unique because the minimum distance is 8 (2.4.1); `None` when there is
/// none.
fn error_pattern(syndrome: u32) -> Option<(u64, u32)> {
    if syndrome == 0 {
        return Some((0, 0));
    }
    if let Some(pattern) = single_error(syndrome) {
        return Some((pattern, 1));
    }
    for (a, &(syndrome_a, pattern_a)) in SINGLE_ERRORS.iter().enumerate() {
        for &(syndrome_b, pattern_b) in SINGLE_ERRORS.iter().skip(a + 1) {
            let rest = syndrome ^ syndrome_a ^ syndrome_b;
            if rest == 0 {
                return Some((pattern_a | pattern_b, 2));
            }
            // A third position equal to a or b would mean weight 1, excluded above.
            if let Some(pattern_c) = single_error(rest) {
                return Some((pattern_a | pattern_b | pattern_c, 3));
            }
        }
    }
    None
}

/// Bits `shift .. shift + width` of `data`.
const fn field(data: u32, shift: u32, width: u32) -> u32 {
    (data >> shift) & ((1 << width) - 1)
}

/// A field of at most 8 bits, as a `u8`.
fn field_u8(data: u32, shift: u32, width: u32) -> u8 {
    let [low, ..] = field(data, shift, width.min(8)).to_le_bytes();
    low
}

/// The low 32 bits of `value`.
fn low_u32(value: u64) -> u32 {
    let [a, b, c, d, ..] = value.to_le_bytes();
    u32::from_le_bytes([a, b, c, d])
}

/// True when a version-0 data word holds none of the values that step 3 of 2.7 makes "not
/// decoded". Words of versions 1 to 3 are always decoded.
const fn fields_decodable(data: u32) -> bool {
    if field(data, 26, 2) != 0 {
        return true;
    }
    let w = field(data, 15, 10);
    let h = field(data, 5, 10);
    let colour = field(data, 1, 2);
    let cell = field(data, 0, 1);
    w != 0 && h != 0 && colour <= 1 && !(colour == 0 && cell == 1)
}

/// Steps 1 to 3 of 2.7 for one copy: the data word and the number of corrected bits, or `None`
/// when the copy is not decoded.
fn decode_copy(raw: u64) -> Option<(u32, u32)> {
    let received = (raw & CODEWORD_MASK) ^ FORMAT_MASK;
    let (pattern, errors) = error_pattern(format_syndrome(received))?;
    let data = low_u32((received ^ pattern) >> FORMAT_PARITY_BITS) & DATA_MASK;
    fields_decodable(data).then_some((data, errors))
}

/// The format word a reader uses, from [`decode_format`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormatDecoded {
    /// The chosen word (2.7, step 4), after 2.3 has been applied (step 5).
    pub word: FormatWord,
    /// The number of bit errors e corrected in the chosen copy (0 to 3).
    pub errors: u32,
    /// The index in the `copies` argument of the chosen copy.
    pub copy: usize,
    /// Another decoded copy's word when it differs from the chosen one and is itself acceptable
    /// under 2.3. The reader MAY try it if the base layer fails under `word` (2.7, step 4).
    pub alternative: Option<FormatWord>,
}

/// Decodes the format word from the raw 47-bit word sampled from each copy (2.7).
///
/// `copies` holds the word F' read from each copy whose finder is visible, copy A first, then
/// copy B; leave out a copy that was not sampled. Only the low 47 bits of each value are read,
/// in the bit order of the module documentation. Each copy is unmasked and corrected with up to
/// [`FORMAT_MAX_ERRORS`] bit errors; a copy whose fields are invalid in format version 0 counts
/// as not decoded (step 3). Among the decoded copies the one with the fewest corrected bits
/// wins, and an earlier copy wins a tie (step 4). Then 2.3 is applied to the chosen word.
///
/// Symbol class 1 and colour profile 1 are returned in the word: the container of chapter 3
/// decides what follows (3.3, 3.5).
///
/// # Errors
///
/// - [`Error::FormatUnreadable`] when no copy decodes (or `copies` is empty).
/// - [`Error::FormatVersion`] when the chosen word has format version 1, 2 or 3 (2.3, 9.2).
/// - [`Error::TileColour`] when the chosen word is a transfer tile with a colour profile other
///   than 0 (3.3, 7.3).
pub fn decode_format(copies: &[u64]) -> Result<FormatDecoded, Error> {
    let decoded: Vec<(usize, u32, u32)> = copies
        .iter()
        .enumerate()
        .filter_map(|(index, &raw)| decode_copy(raw).map(|(data, errors)| (index, data, errors)))
        .collect();
    // Fewest errors first; `min_by_key` keeps the earliest of equal keys.
    let &(copy, data, errors) =
        decoded.iter().min_by_key(|&&(_, _, errors)| errors).ok_or(Error::FormatUnreadable)?;
    let word = FormatWord::from_chosen(data)?;
    let alternative = decoded
        .iter()
        .filter(|&&(_, other, _)| other != data)
        .min_by_key(|&&(_, _, errors)| errors)
        .and_then(|&(_, other, _)| FormatWord::from_chosen(other).ok());
    Ok(FormatDecoded { word, errors, copy, alternative })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_the_product_of_the_minimal_polynomials() {
        // (x + 1) · m1 · m3 · m5 over GF(2) (2.4.2).
        fn mul(a: u64, b: u64) -> u64 {
            let mut out = 0;
            for i in 0..64 {
                if (b >> i) & 1 == 1 {
                    out ^= a << i;
                }
            }
            out
        }
        let m1 = 0b100_0011;
        let m3 = 0b101_0111;
        let m5 = 0b110_0111;
        assert_eq!(mul(m1, mul(m3, m5)), 0o1_701_317);
        assert_eq!(mul(0b11, mul(m1, mul(m3, m5))), u64::from(FORMAT_GENERATOR));
    }

    #[test]
    fn single_error_syndromes_are_distinct_and_non_zero() {
        for pair in SINGLE_ERRORS.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        assert!(SINGLE_ERRORS.iter().all(|&(s, _)| s != 0));
    }

    #[test]
    fn every_pattern_of_weight_up_to_three_is_corrected() {
        // All 17 344 patterns of weight 0 to 3 (2.7, permitted variations).
        let mut count = 0u32;
        let bits = FORMAT_CODEWORD_BITS;
        let mut check = |pattern: u64, weight: u32| {
            assert_eq!(error_pattern(format_syndrome(pattern)), Some((pattern, weight)));
            count += 1;
        };
        check(0, 0);
        for a in 0..bits {
            check(1 << a, 1);
            for b in a + 1..bits {
                check((1 << a) | (1 << b), 2);
                for c in b + 1..bits {
                    check((1 << a) | (1 << b) | (1 << c), 3);
                }
            }
        }
        assert_eq!(count, 17_344);
    }

    #[test]
    fn codewords_have_zero_syndrome() {
        for data in [0, 1, 0x000_8028, 0x001_0040, DATA_MASK, 0x0AB_CDEF] {
            assert_eq!(format_syndrome(format_codeword(data)), 0);
        }
    }
}
