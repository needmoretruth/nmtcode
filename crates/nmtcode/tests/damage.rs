//! Damage: data modules within and beyond the Reed-Solomon limit (chapter 4, 4.9) and format
//! copies within and beyond 3 bit errors (chapter 2, 2.7).

mod common;

use std::collections::BTreeSet;

use common::{codeword_modules, flip_codeword};
use nmtcode::{
    DecodeOptions, Decoded, EncodeOptions, ModuleGrid, Outcome, SizeRule, SpecError, Symbol,
    SymbolClass, decode, encode_file, encode_text,
};
use nmtcode_core::FormatWord;
use nmtcode_symbol::{FORMAT_BITS, Layout, format_positions};
use proptest::prelude::*;

/// A small deterministic generator for choosing positions inside one test case.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*; the seed is never 0.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }

    fn mask(&mut self) -> u8 {
        u8::try_from(self.next() % 255).unwrap() + 1
    }
}

fn symbol_for(bytes: &[u8], level: u8, size: SizeRule) -> Symbol {
    let options = EncodeOptions { level: Some(level), size, ..EncodeOptions::default() };
    encode_file("data.bin", bytes, &options).unwrap()
}

fn size_rule(square: bool, width: u32, height: u32) -> SizeRule {
    if square { SizeRule::SmallestSquare } else { SizeRule::Exact { width, height } }
}

fn clean(symbol: &Symbol) -> Decoded {
    decode(symbol.grid(), &DecodeOptions::default()).unwrap()
}

/// Damages `errors(block)` distinct codewords of every block, each by a random non-zero bit
/// pattern on its modules. Returns the number of damaged codewords.
fn damage_blocks(
    symbol: &Symbol,
    grid: &mut ModuleGrid,
    rng: &mut Rng,
    mut errors: impl FnMut(&nmtcode_ecc::Block, &mut Rng) -> usize,
) -> usize {
    let layout = Layout::new(symbol.width(), symbol.height()).unwrap();
    let modules = codeword_modules(&layout);
    let split = symbol.block_split();
    let mut damaged = 0;
    for block in split.blocks() {
        let count = errors(&block, rng).min(block.len);
        let mut bytes = BTreeSet::new();
        while bytes.len() < count {
            bytes.insert(rng.below(block.len));
        }
        for byte in bytes {
            let index = split.stream_index(block.index, byte).unwrap();
            flip_codeword(grid, &modules, index, rng.mask());
            damaged += 1;
        }
    }
    damaged
}

fn fits(bytes_len: usize, level: u8, width: u32, height: u32) -> bool {
    // Stored file record list: at most 1 + 3 + 1 + 14 + 3 + len + 4 bytes.
    nmtcode::capacity(width, height, level).is_some_and(|k| k >= bytes_len + 26)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Up to P/2 wrong codewords in every block are corrected, and the content is unchanged.
    #[test]
    fn errors_up_to_the_limit_are_corrected(
        bytes in proptest::collection::vec(any::<u8>(), 0..700),
        level in 0u8..=3,
        square in any::<bool>(),
        width in (5u32..=40).prop_map(|k| 4 * k),
        height in (5u32..=40).prop_map(|k| 4 * k),
        at_limit in any::<bool>(),
        seed in 1u64..,
    ) {
        prop_assume!(square || fits(bytes.len(), level, width, height));
        let symbol = symbol_for(&bytes, level, size_rule(square, width, height));
        let original = clean(&symbol);
        let mut grid = symbol.grid().clone();
        let mut rng = Rng(seed);
        let damaged = damage_blocks(&symbol, &mut grid, &mut rng, |block, rng| {
            let limit = block.parity_len / 2;
            if at_limit { limit } else { rng.below(limit + 1) }
        });
        let decoded = decode(&grid, &DecodeOptions::default()).unwrap();
        prop_assert_eq!(&decoded.records, &original.records);
        prop_assert_eq!(decoded.corrected, damaged);
    }

    /// More than P/2 wrong codewords in a block: an error, never other content.
    #[test]
    fn errors_beyond_the_limit_never_give_other_content(
        bytes in proptest::collection::vec(any::<u8>(), 0..700),
        level in 0u8..=3,
        seed in 1u64..,
        every_block in any::<bool>(),
    ) {
        let symbol = symbol_for(&bytes, level, SizeRule::SmallestSquare);
        let original = clean(&symbol);
        let mut grid = symbol.grid().clone();
        let mut rng = Rng(seed);
        let target = rng.below(symbol.block_split().block_count());
        damage_blocks(&symbol, &mut grid, &mut rng, |block, rng| {
            let limit = block.parity_len / 2;
            if every_block || block.index == target {
                limit + 1 + rng.below(block.len - limit)
            } else {
                rng.below(limit + 1)
            }
        });
        match decode(&grid, &DecodeOptions::default()) {
            Ok(decoded) => prop_assert_eq!(&decoded.records, &original.records),
            Err(error) => prop_assert_eq!(error.outcome(), Outcome::Damaged, "{:?}", error),
        }
    }

    /// Random data modules flipped anywhere, at any density.
    #[test]
    fn random_module_damage_never_gives_other_content(
        text in "[ -~]{0,300}",
        level in 0u8..=3,
        per_mille in 1usize..=500,
        seed in 1u64..,
    ) {
        let options = EncodeOptions { level: Some(level), ..EncodeOptions::default() };
        let symbol = encode_text(&text, &options).unwrap();
        let original = clean(&symbol);
        let layout = Layout::new(symbol.width(), symbol.height()).unwrap();
        let mut grid = symbol.grid().clone();
        let mut rng = Rng(seed);
        for (x, y) in layout.placement() {
            if rng.below(1000) < per_mille {
                grid.toggle(x, y);
            }
        }
        match decode(&grid, &DecodeOptions::default()) {
            Ok(decoded) => prop_assert_eq!(&decoded.records, &original.records),
            Err(error) => prop_assert_eq!(error.outcome(), Outcome::Damaged, "{:?}", error),
        }
    }

    /// Any damage to one format copy: the other copy gives the symbol, or, when the damaged copy
    /// decodes to another word (2.7, 3.3% of random words pass step 2), the symbol is rejected
    /// with `E_FORMAT_CONFLICT`. Never other content.
    #[test]
    fn one_format_copy_damaged(
        text in "[ -~]{0,100}",
        level in 0u8..=3,
        copy in 0usize..2,
        mask in 1u64..(1 << 47),
    ) {
        let options = EncodeOptions { level: Some(level), ..EncodeOptions::default() };
        let symbol = encode_text(&text, &options).unwrap();
        let original = clean(&symbol);
        let mut grid = symbol.grid().clone();
        let positions = format_positions(symbol.width(), symbol.height()).unwrap();
        for (bit, &(x, y)) in positions[copy].iter().enumerate() {
            if mask >> bit & 1 == 1 {
                grid.toggle(x, y);
            }
        }
        match decode(&grid, &DecodeOptions::default()) {
            Ok(decoded) => prop_assert_eq!(&decoded.records, &original.records),
            Err(error) => prop_assert_eq!(error.error(), SpecError::FormatConflict),
        }
        // Up to 3 bit errors in the damaged copy always leave the symbol readable.
        if mask.count_ones() <= 3 {
            prop_assert!(decode(&grid, &DecodeOptions::default()).is_ok());
        }
    }

    /// Exactly 4 bit errors in each copy: no copy decodes, since the minimum distance is 8.
    #[test]
    fn both_format_copies_beyond_three_bits(
        text in "[ -~]{0,100}",
        level in 0u8..=3,
        bits_a in proptest::sample::subsequence((0..FORMAT_BITS).collect::<Vec<_>>(), 4),
        bits_b in proptest::sample::subsequence((0..FORMAT_BITS).collect::<Vec<_>>(), 4),
        spare in 0usize..=3,
    ) {
        let options = EncodeOptions { level: Some(level), ..EncodeOptions::default() };
        let symbol = encode_text(&text, &options).unwrap();
        let positions = format_positions(symbol.width(), symbol.height()).unwrap();
        let mut grid = symbol.grid().clone();
        for &bit in &bits_a {
            let (x, y) = positions[0][bit];
            grid.toggle(x, y);
        }
        let mut both = grid.clone();
        for &bit in &bits_b {
            let (x, y) = positions[1][bit];
            both.toggle(x, y);
        }
        let error = decode(&both, &DecodeOptions::default()).unwrap_err();
        prop_assert_eq!(error.error(), SpecError::FormatUnreadable);
        prop_assert_eq!(error.name(), "E_FORMAT_UNREADABLE");
        prop_assert_eq!(error.outcome(), Outcome::Damaged);

        // Copy B with at most 3 errors still decodes while copy A has 4.
        for &bit in bits_b.iter().take(spare) {
            let (x, y) = positions[1][bit];
            grid.toggle(x, y);
        }
        let decoded = decode(&grid, &DecodeOptions::default()).unwrap();
        prop_assert_eq!(&decoded.records, &clean(&symbol).records);
    }
}

#[test]
fn blank_and_solid_format_areas_are_unreadable() {
    let symbol = encode_text("NMT Code", &EncodeOptions::default()).unwrap();
    let positions = format_positions(symbol.width(), symbol.height()).unwrap();
    for dark in [false, true] {
        let mut grid = symbol.grid().clone();
        for &(x, y) in positions.iter().flatten() {
            grid.set(x, y, dark);
        }
        let error = decode(&grid, &DecodeOptions::default()).unwrap_err();
        assert_eq!(error.name(), "E_FORMAT_UNREADABLE");
    }
}

#[test]
fn a_grid_of_another_size_than_its_format_word_is_unreadable() {
    let symbol = encode_text("NMT Code", &EncodeOptions::default()).unwrap();
    assert_eq!((symbol.width(), symbol.height()), (20, 20));
    let mut larger = ModuleGrid::new(24, 24).unwrap();
    for y in 0..20 {
        for x in 0..20 {
            larger.set(x, y, symbol.grid().get(x, y).unwrap());
        }
    }
    let error = decode(&larger, &DecodeOptions::default()).unwrap_err();
    assert_eq!(error.error(), SpecError::FormatUnreadable);
    let tiny = ModuleGrid::new(8, 8).unwrap();
    assert_eq!(decode(&tiny, &DecodeOptions::default()).unwrap_err().name(), "E_FORMAT_UNREADABLE");
}

/// 2.7 steps 3 and 4: two copies that decode to different words reject the symbol, and no word
/// is tried after the other; a copy whose W and H do not fit the grid is not decoded, and the
/// other copy gives the symbol.
#[test]
fn format_copies_that_differ_reject_the_symbol() {
    let symbol = encode_text("NMT Code", &EncodeOptions::default()).unwrap();
    let original = clean(&symbol);
    let positions = format_positions(20, 20).unwrap();
    let write_copy_a = |grid: &mut ModuleGrid, word: u64| {
        for (i, &(x, y)) in positions[0].iter().enumerate() {
            grid.set(x, y, word >> (46 - i) & 1 == 1);
        }
    };
    // The same size at another level: both copies decode, to different words.
    let other_level = FormatWord::new(SymbolClass::Static, 20, 20, 2, 0, 0).unwrap();
    let mut grid = symbol.grid().clone();
    write_copy_a(&mut grid, other_level.encode());
    let error = decode(&grid, &DecodeOptions::default()).unwrap_err();
    assert_eq!((error.error(), error.outcome()), (SpecError::FormatConflict, Outcome::Damaged));
    // Another size: copy A does not fit the grid, so copy B alone gives the symbol.
    let other_size = FormatWord::new(SymbolClass::Static, 24, 24, 0, 0, 0).unwrap();
    let mut grid = symbol.grid().clone();
    write_copy_a(&mut grid, other_size.encode());
    let decoded = decode(&grid, &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.records, original.records);
    assert_eq!(decoded.format, *symbol.format());
}

/// 2.7 step 5: an area above the reader's largest area rejects the symbol with `E_SIZE_LIMIT`;
/// a symbol of exactly that area decodes.
#[test]
fn an_area_above_the_readers_largest_is_unsupported() {
    let symbol = encode_text("NMT Code", &EncodeOptions::default()).unwrap();
    let area = u64::from(symbol.width() * symbol.height());
    let at = DecodeOptions { max_area: area, ..DecodeOptions::default() };
    assert_eq!(decode(symbol.grid(), &at).unwrap().records, clean(&symbol).records);
    let below = DecodeOptions { max_area: area - 1, ..DecodeOptions::default() };
    let error = decode(symbol.grid(), &below).unwrap_err();
    assert_eq!((error.error(), error.outcome()), (SpecError::SizeLimit, Outcome::Unsupported));
    assert_eq!(nmtcode::MAX_AREA, 4108 * 4108);
    assert_eq!(DecodeOptions::default().max_area, nmtcode::MAX_AREA);
}

/// 2.5 and 5.11: a symbol taken as upright when it is turned by 180° reads copy B where copy A
/// should be and copy A where copy B should be. Neither decodes under the other copy's mask.
#[test]
fn a_symbol_turned_by_180_degrees_fails_at_the_format_word() {
    let symbol = encode_text("NMT Code", &EncodeOptions::default()).unwrap();
    let (w, h) = (symbol.width(), symbol.height());
    let mut turned = ModuleGrid::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            turned.set(x, y, symbol.grid().get(w - 1 - x, h - 1 - y).unwrap());
        }
    }
    let error = decode(&turned, &DecodeOptions::default()).unwrap_err();
    assert_eq!(error.error(), SpecError::FormatUnreadable);
}
