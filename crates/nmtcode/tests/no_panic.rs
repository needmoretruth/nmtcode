//! The whole reader never panics, whatever the grid holds.

mod common;

use common::{sealed_container, symbol_with_container};
use nmtcode::{DecodeOptions, ModuleGrid, SymbolClass, decode};
use nmtcode_core::{FORMAT_MASK, format_codeword};
use nmtcode_symbol::{Layout, write_format_copies};
use proptest::prelude::*;

fn side() -> impl Strategy<Value = u32> {
    (5u32..=50).prop_map(|k| 4 * k)
}

fn fill(width: u32, height: u32, bits: &[u8]) -> ModuleGrid {
    let mut grid = ModuleGrid::new(width, height).unwrap();
    let mut index = 0usize;
    for y in 0..height {
        for x in 0..width {
            let byte = bits.get(index / 8 % bits.len().max(1)).copied().unwrap_or(0);
            grid.set(x, y, byte >> (index % 8) & 1 == 1);
            index += 1;
        }
    }
    grid
}

/// Decodes and checks that a result, when there is one, is internally consistent.
fn decode_anything(grid: &ModuleGrid, limit: u32) {
    if let Ok(decoded) = decode(grid, &DecodeOptions { limit }) {
        assert_eq!(
            (decoded.format.width(), decoded.format.height()),
            (grid.width(), grid.height())
        );
        assert!(!decoded.records.is_empty());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Random modules on a grid of a valid size.
    #[test]
    fn random_grid_of_a_valid_size(
        width in side(),
        height in side(),
        bits in proptest::collection::vec(any::<u8>(), 1..512),
    ) {
        decode_anything(&fill(width, height, &bits), 16 << 20);
    }

    /// Random modules on a grid of any size, valid or not.
    #[test]
    fn random_grid_of_any_size(
        width in 1u32..=64,
        height in 1u32..=64,
        bits in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        decode_anything(&fill(width, height, &bits), 1000);
    }

    /// Any 28-bit format data word on both copies over random data, so that every value of
    /// every format field reaches the later steps.
    #[test]
    fn random_format_word_and_data(
        width in side(),
        height in side(),
        data in 0u32..(1 << 28),
        bits in proptest::collection::vec(any::<u8>(), 1..256),
    ) {
        let mut grid = fill(width, height, &bits);
        write_format_copies(&mut grid, format_codeword(data) ^ FORMAT_MASK).unwrap();
        decode_anything(&grid, 16 << 20);
    }

    /// A random message with correct Reed-Solomon parity: the container checks see any bytes.
    #[test]
    fn random_message_with_valid_parity(
        width in side(),
        height in side(),
        level in 0u8..=3,
        colour in 0u8..=1,
        tile in any::<bool>(),
        bytes in proptest::collection::vec(any::<u8>(), 0..400),
    ) {
        let layout = Layout::new(width, height).unwrap();
        let k = nmtcode_ecc::split(layout.codeword_count(), level).unwrap().capacity();
        let message: Vec<u8> = bytes.iter().copied().cycle().take(k).collect();
        let class = if tile { SymbolClass::TransferTile } else { SymbolClass::Static };
        let colour = if tile { 0 } else { colour };
        decode_anything(&symbol_with_container(&message, width, height, level, colour, class), 1 << 16);
    }

    /// A random lead byte and body with a correct CRC-32C: every header field, codec and record
    /// check sees any bytes.
    #[test]
    fn random_sealed_container(
        lead in any::<u8>(),
        body in proptest::collection::vec(any::<u8>(), 0..200),
        colour in 0u8..=1,
        limit in prop_oneof![Just(16u32 << 20), 0u32..2000],
    ) {
        let container = sealed_container(lead, &body);
        decode_anything(&symbol_with_container(&container, 64, 64, 0, colour, SymbolClass::Static), limit);
    }
}
