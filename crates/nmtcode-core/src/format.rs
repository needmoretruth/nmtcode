//! The format word of chapter 2: its 28 data bits (2.2, 2.3), the BCH (47, 28) code that protects
//! it (2.4), the masks of its two copies (2.5) and how a reader decodes and chooses between the
//! copies (2.7).
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
//! Module A\[i\] of copy A carries index `i` of `F_A` and module B\[i\] of copy B index `i` of
//! `F_B`, that is `(word >> (46 - i)) & 1`, dark for 1 (2.6). The two copies carry the same
//! codeword under different masks, so `F_A` and `F_B` differ.

use crate::{Error, is_valid_side};

/// Number of data bits of the format word (2.2).
pub const FORMAT_DATA_BITS: u32 = 28;
/// Number of parity bits of the format word (2.4.1).
pub const FORMAT_PARITY_BITS: u32 = 19;
/// Number of bits of a format codeword, and of modules in each of its two copies (2.4.1, 2.6).
pub const FORMAT_CODEWORD_BITS: u32 = 47;
/// The generator polynomial g(x) as an integer, bit i the coefficient of x^i (2.4.1).
pub const FORMAT_GENERATOR: u32 = 0x8_8751;
/// `MASK_A`, XOR-ed onto the codeword of copy A before it is sent (2.5): the most significant
/// 47 bits of SHA-256 over `NMT Code format mask`.
pub const FORMAT_MASK_A: u64 = 0x51F3_694E_AFAA;
/// `MASK_B`, XOR-ed onto the codeword of copy B before it is sent (2.5): bits 94 to 140 of
/// SHA-256 over `NMT Code format mask B`.
///
/// 2.5 takes the digest in 47-bit pieces from its most significant end (bits 0 to 46, 47 to 93,
/// 94 to 140, …) and uses the first piece for which `MASK_A` XOR `MASK_B` lies in a coset of
/// minimum weight 6 or more, and the all-light and all-dark words of copy B are more than 3 bit
/// errors from every codeword. The first two pieces give `MASK_A` XOR `MASK_B` a coset of weight 4.
pub const FORMAT_MASK_B: u64 = 0x3FCA_34BE_1F26;
/// The masks of copy A and copy B, in that order (2.5).
pub const FORMAT_MASKS: [u64; 2] = [FORMAT_MASK_A, FORMAT_MASK_B];
/// The most bit errors a reader corrects in one copy without erasures (2.7, step 2).
pub const FORMAT_MAX_ERRORS: u32 = 3;
/// The most bits of one copy a reader may mark as erasures (2.7, step 2).
pub const FORMAT_MAX_ERASURES: u32 = 4;
/// The decoding bound of 2.7, step 2: e errors and s erasures with 2e + s ≤ 7.
pub const FORMAT_DECODING_BOUND: u32 = 7;
/// The bound 2e + s ≤ 4 under which a single decoded copy of format version 1 to 3 may be
/// reported as needing a newer reader (2.7, step 5).
pub const FORMAT_VERSION_REPORT_BOUND: u32 = 4;
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
/// colour profile is 1, and colour profile 0 for a transfer tile. [`FormatWord::new`],
/// [`decode_format`] and [`decode_format_with`] are the only ways to make one.
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

    /// The fields of a format-version-0 data word, or `None` when the word holds a value or a
    /// combination that step 3 of 2.7 makes "not decoded": width or height code 0, colour
    /// profile 2 or 3, chroma cell size 1 with colour profile 0, or a transfer tile with a colour
    /// profile other than 0. Also `None` for format versions 1 to 3.
    fn from_data(data: u32) -> Option<Self> {
        if field(data, 26, 2) != 0 {
            return None;
        }
        let class =
            if field(data, 25, 1) == 1 { SymbolClass::TransferTile } else { SymbolClass::Static };
        let w = field(data, 15, 10);
        let h = field(data, 5, 10);
        let colour_profile = field_u8(data, 1, 2);
        let chroma_cell = field_u8(data, 0, 1);
        if w == 0
            || h == 0
            || colour_profile > 1
            || (colour_profile == 0 && chroma_cell == 1)
            || (matches!(class, SymbolClass::TransferTile) && colour_profile != 0)
        {
            return None;
        }
        Some(Self {
            version: FORMAT_VERSION,
            class,
            width: 4 * (w + 4),
            height: 4 * (h + 4),
            level: field_u8(data, 3, 2),
            colour_profile,
            chroma_cell,
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

    /// The codeword U = (d << 19) | P before the masks (2.4.3).
    pub fn codeword(&self) -> u64 {
        format_codeword(self.data())
    }

    /// The sent word of copy A, `F_A` = U XOR `MASK_A` (2.5).
    ///
    /// Bit order: see the module documentation. Index `i` of 2.4.3, which goes on module A\[i\]
    /// (2.6), is `(word >> (46 - i)) & 1`. [`FormatWord::encode_copies`] gives both copies.
    pub fn encode(&self) -> u64 {
        self.codeword() ^ FORMAT_MASK_A
    }

    /// The sent words of both copies, `[F_A, F_B]`: the codeword XOR `MASK_A` for copy A and XOR
    /// `MASK_B` for copy B (2.5, 2.6). Index `i` of each goes on module A\[i\] or B\[i\].
    pub fn encode_copies(&self) -> [u64; 2] {
        let codeword = self.codeword();
        FORMAT_MASKS.map(|mask| codeword ^ mask)
    }

    /// The format echo byte that a static container carries after its lead byte
    /// (chapter 3, 3.2.2).
    pub const fn echo(&self) -> FormatEcho {
        FormatEcho::of(self.class, self.level, self.colour_profile, self.chroma_cell)
    }
}

/// The format echo byte of a static container (chapter 3, 3.2.2): the error-correction level
/// (bits 7–6), the colour profile (bits 5–4), the chroma cell size (bit 3) and the symbol class
/// (bit 2) of the format word; bits 1–0 are reserved and 0.
///
/// A generator writes it before it knows the symbol size, since none of these fields depends on
/// W or H; a reader compares it with [`FormatWord::echo`] of the chosen format word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormatEcho(u8);

impl FormatEcho {
    /// The echo of the given fields, with the checks of [`FormatWord::new`] that do not depend
    /// on the size.
    ///
    /// # Errors
    ///
    /// [`FormatWordError::Level`], [`FormatWordError::ColourProfile`],
    /// [`FormatWordError::ChromaCell`], [`FormatWordError::ChromaCellWithoutColour`] or
    /// [`FormatWordError::TransferTileColour`], as [`FormatWord::new`].
    pub const fn new(
        class: SymbolClass,
        level: u8,
        colour_profile: u8,
        chroma_cell: u8,
    ) -> Result<Self, FormatWordError> {
        match FormatWord::new(
            class,
            MIN_SIDE_FOR_ECHO,
            MIN_SIDE_FOR_ECHO,
            level,
            colour_profile,
            chroma_cell,
        ) {
            Ok(word) => Ok(word.echo()),
            Err(error) => Err(error),
        }
    }

    const fn of(class: SymbolClass, level: u8, colour_profile: u8, chroma_cell: u8) -> Self {
        let class_bit = match class {
            SymbolClass::Static => 0,
            SymbolClass::TransferTile => 1,
        };
        Self(
            ((level & 3) << 6)
                | ((colour_profile & 3) << 4)
                | ((chroma_cell & 1) << 3)
                | (class_bit << 2),
        )
    }

    /// The byte as written in the container.
    pub const fn byte(self) -> u8 {
        self.0
    }
}

/// A valid side, used by [`FormatEcho::new`] to reuse the field checks of [`FormatWord::new`].
const MIN_SIDE_FOR_ECHO: u32 = crate::MIN_SIDE;

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

/// The codeword U = (d << 19) | P of any 28-bit data word, before the masks (2.4.3).
///
/// Bits of `data` above bit 27 are ignored. XOR the result with [`FORMAT_MASK_A`] or
/// [`FORMAT_MASK_B`] to get the sent word of a copy. Unlike [`FormatWord::encode_copies`] this
/// accepts reserved and invalid field values, so that tests can build the words a reader must
/// reject.
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

/// One sampled copy of the format word (2.7, step 1): the 47 bits as read, still masked, and the
/// bits the reader marks as erasures.
///
/// Both values use the bit order of the module documentation: bit index `i` is integer bit
/// `46 − i`. Bits above bit 46 are ignored. A reader marks a bit as an erasure when it cannot
/// tell its value, for example under glare; at most [`FORMAT_MAX_ERASURES`] bits may be marked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatSample {
    /// The raw word F' as read: bit set for a dark module.
    pub bits: u64,
    /// The erased bits: bit set for a module whose value is unknown. Its value in `bits` is
    /// ignored.
    pub erasures: u64,
}

impl FormatSample {
    /// A sample without erasures.
    pub const fn new(bits: u64) -> Self {
        Self { bits, erasures: 0 }
    }

    /// A sample with the given erased bits.
    pub const fn with_erasures(bits: u64, erasures: u64) -> Self {
        Self { bits, erasures }
    }
}

/// Steps 1 and 2 of 2.7 for one copy: the data word, the number of errors e outside the
/// erasures and the number of erasures s, or `None` when no codeword lies within 2e + s ≤ 7, or
/// more than [`FORMAT_MAX_ERASURES`] bits are erased.
///
/// Every result is a codeword of the full generator g(x), including its x + 1 factor: it is the
/// received word XOR an error pattern with the same syndrome under g(x).
fn correct(sample: FormatSample, mask: u64) -> Option<(u32, u32, u32)> {
    let erasures = sample.erasures & CODEWORD_MASK;
    let s = erasures.count_ones();
    if s > FORMAT_MAX_ERASURES {
        return None;
    }
    let received = (sample.bits & CODEWORD_MASK) ^ mask;
    // With the erased bits filled all 0 and then all 1, one filling is within 3 bits of every
    // codeword that meets 2e + s ≤ 7 (e + floor(s / 2) ≤ 3); such a codeword is unique, since
    // two of them would lie at most 7 apart and the minimum distance is 8.
    let fillings = [received & !erasures, received | erasures];
    fillings.iter().take(if s == 0 { 1 } else { 2 }).find_map(|&filled| {
        let (pattern, _) = error_pattern(format_syndrome(filled))?;
        let codeword = filled ^ pattern;
        let e = ((codeword ^ received) & !erasures).count_ones();
        (2 * e + s <= FORMAT_DECODING_BOUND)
            .then(|| (low_u32(codeword >> FORMAT_PARITY_BITS) & DATA_MASK, e, s))
    })
}

/// A copy that passed steps 1 to 3 of 2.7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DecodedCopy {
    /// The data word d.
    data: u32,
    /// Errors e outside the erasures.
    errors: u32,
    /// Erasures s.
    erasures: u32,
}

impl DecodedCopy {
    /// 2e + s, the distance measure of step 2.
    const fn weight(&self) -> u32 {
        2 * self.errors + self.erasures
    }
}

/// Steps 1 to 3 of 2.7 for one copy. Step 3: a format-version-0 word with an invalid field or
/// combination, or one that `accept` refuses, is not decoded; a word of format version 1 to 3
/// is decoded, and only its version field is read.
fn decode_copy(
    sample: FormatSample,
    mask: u64,
    accept: &impl Fn(&FormatWord) -> bool,
) -> Option<DecodedCopy> {
    let (data, errors, erasures) = correct(sample, mask)?;
    let decoded = DecodedCopy { data, errors, erasures };
    if field(data, 26, 2) != 0 {
        return Some(decoded);
    }
    FormatWord::from_data(data).filter(|word| accept(word)).map(|_| decoded)
}

/// The format word a reader uses, from [`decode_format`] or [`decode_format_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormatDecoded {
    /// The chosen word (2.7, step 4), after 2.3 has been applied (step 5).
    pub word: FormatWord,
    /// The number of bit errors e corrected in the chosen copy, outside its erasures.
    pub errors: u32,
    /// The number of erasures s of the chosen copy.
    pub erasures: u32,
    /// The chosen copy: 0 for copy A, 1 for copy B. When both copies decoded, the one with the
    /// smaller 2e + s, and copy A on a tie.
    pub copy: usize,
    /// True when both copies decoded, to this word.
    pub both: bool,
}

/// Decodes the format word from the raw 47-bit words of copy A and copy B, in that order (2.7).
///
/// `None` stands for a copy that was not sampled, for example because its finder is not
/// visible. Each copy is unmasked with its own mask ([`FORMAT_MASKS`]) and corrected with up to
/// [`FORMAT_MAX_ERRORS`] bit errors. This is [`decode_format_with`] without erasures and without
/// a size check.
///
/// # Errors
///
/// As [`decode_format_with`].
pub fn decode_format(copies: [Option<u64>; 2]) -> Result<FormatDecoded, Error> {
    decode_format_with(copies.map(|copy| copy.map(FormatSample::new)), |_| true)
}

/// Decodes the format word from the samples of copy A and copy B, in that order (2.7).
///
/// 1. Each sampled copy is unmasked with its own mask ([`FORMAT_MASKS`]) and corrected with e
///    errors and s erasures, 2e + s ≤ 7 and s ≤ 4 (steps 1 and 2).
/// 2. A copy of format version 0 is not decoded when a field or a combination is invalid (2.3),
///    or when `accept` returns false for its word. `accept` is where a reader applies the checks
///    that need more than the format word: that W and H agree with the finders it found
///    (chapter 5), and that a colour-profile-1 size has at least 16 colour codewords
///    (chapter 7, 7.8.2). A copy of format version 1 to 3 is decoded (step 3).
/// 3. No decoded copy rejects the symbol; two decoded copies with different words reject it too
///    (step 4).
/// 4. A word of format version 1 to 3 rejects the symbol. It is reported as needing a newer
///    reader only when both copies decoded to it or its copy has 2e + s ≤ 4 (step 5).
///
/// Symbol class 1 and colour profile 1 are returned in the word: the container of chapter 3
/// decides what follows (3.3, 3.5).
///
/// # Errors
///
/// - [`Error::FormatUnreadable`] when no copy decodes, and for a word of format version 1 to 3
///   from one copy with 2e + s > 4.
/// - [`Error::FormatConflict`] when both copies decode, to different words.
/// - [`Error::FormatVersion`] when the chosen word has format version 1, 2 or 3 and both
///   copies decoded to it or its copy has 2e + s ≤ 4 (2.3, 9.2).
pub fn decode_format_with(
    copies: [Option<FormatSample>; 2],
    accept: impl Fn(&FormatWord) -> bool,
) -> Result<FormatDecoded, Error> {
    let [a, b] = [0, 1].map(|index| {
        copies
            .get(index)
            .copied()
            .flatten()
            .zip(FORMAT_MASKS.get(index))
            .and_then(|(sample, &mask)| decode_copy(sample, mask, &accept))
    });
    let (copy, chosen, both) = match (a, b) {
        (None, None) => return Err(Error::FormatUnreadable),
        (Some(a), Some(b)) if a.data != b.data => return Err(Error::FormatConflict),
        (Some(a), Some(b)) => {
            if b.weight() < a.weight() {
                (1, b, true)
            } else {
                (0, a, true)
            }
        }
        (Some(a), None) => (0, a, false),
        (None, Some(b)) => (1, b, false),
    };
    if field(chosen.data, 26, 2) != 0 {
        return Err(if both || chosen.weight() <= FORMAT_VERSION_REPORT_BOUND {
            Error::FormatVersion
        } else {
            Error::FormatUnreadable
        });
    }
    let word = FormatWord::from_data(chosen.data).ok_or(Error::FormatUnreadable)?;
    Ok(FormatDecoded { word, errors: chosen.errors, erasures: chosen.erasures, copy, both })
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
