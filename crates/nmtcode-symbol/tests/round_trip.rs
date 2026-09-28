//! Round trips: draw, then read back the codeword stream and both format copies.

use nmtcode_symbol::{Layout, ModuleClass, read_format_copies};
use proptest::prelude::*;

/// A stream of `len` bytes from a 64-bit seed (`SplitMix64`), so that large streams do not need a
/// byte-by-byte strategy.
fn stream_from_seed(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)).to_le_bytes()[0]
        })
        .collect()
}

fn check_round_trip(layout: &Layout, format: [u64; 2], stream: &[u8]) {
    let grid = layout.draw_copies(format, stream).unwrap();
    assert_eq!(layout.read_stream(&grid).unwrap(), stream);
    assert_eq!(read_format_copies(&grid).unwrap(), format);
    // Every function module except the format copies has its fixed value.
    for y in 0..layout.height() {
        for x in 0..layout.width() {
            if let Some(value) = layout.function_value(x, y) {
                assert_eq!(grid.get(x, y), Some(value), "({x}, {y})");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]

    #[test]
    fn round_trip_any_size(
        w_code in 1u32..=70,
        h_code in 1u32..=70,
        format_a in 0u64..(1 << 47),
        format_b in 0u64..(1 << 47),
        seed in any::<u64>(),
    ) {
        let layout = Layout::new(4 * (w_code + 4), 4 * (h_code + 4)).unwrap();
        let stream = stream_from_seed(seed, layout.codeword_count());
        check_round_trip(&layout, [format_a, format_b], &stream);
    }

    /// The placement order visits every data module once, in the column-pair walk of 5.8, and
    /// `placement_index` inverts it.
    #[test]
    fn placement_is_the_walk_of_5_8(w_code in 1u32..=40, h_code in 1u32..=40) {
        let (w, h) = (4 * (w_code + 4), 4 * (h_code + 4));
        let layout = Layout::new(w, h).unwrap();
        let mut walk = Vec::new();
        for p in 0..w / 2 {
            let ys: Vec<u32> = if p % 2 == 0 { (0..h).collect() } else { (0..h).rev().collect() };
            for y in ys {
                for x in [2 * p, 2 * p + 1] {
                    if layout.module_class(x, y) == Some(ModuleClass::Data) {
                        walk.push((x, y));
                    }
                }
            }
        }
        let placement: Vec<_> = layout.placement().collect();
        prop_assert_eq!(&placement, &walk);
        prop_assert_eq!(placement.len(), layout.data_module_count());
        for (k, &(x, y)) in placement.iter().enumerate() {
            prop_assert_eq!(layout.placement_index(x, y), Some(k));
        }
    }

    /// One flipped data module changes exactly one bit of the stream read back, in codeword
    /// floor(k / 8), or nothing when it is a remainder module (5.9).
    #[test]
    fn one_flipped_module_is_one_bit(w_code in 1u32..=20, h_code in 1u32..=20, pick in any::<u64>(), seed in any::<u64>()) {
        let layout = Layout::new(4 * (w_code + 4), 4 * (h_code + 4)).unwrap();
        let stream = stream_from_seed(seed, layout.codeword_count());
        let mut grid = layout.draw_copies([0, 0], &stream).unwrap();
        let d = u64::try_from(layout.data_module_count()).unwrap();
        let k = usize::try_from(pick % d).unwrap();
        let (x, y) = layout.placement().nth(k).unwrap();
        grid.toggle(x, y);
        let read = layout.read_stream(&grid).unwrap();
        let mut expected = stream.clone();
        if k < 8 * layout.codeword_count() {
            expected[k / 8] ^= 0x80 >> (k % 8);
        }
        prop_assert_eq!(read, expected);
    }
}

/// Large and very unequal sizes, including both extremes.
#[test]
fn round_trip_large_and_unequal_sizes() {
    for (w, h) in [(4108, 20), (20, 4108), (1000, 600), (48, 20), (20, 48), (4108, 4108)] {
        let layout = Layout::new(w, h).unwrap();
        let stream = stream_from_seed(u64::from(w * 7 + h), layout.codeword_count());
        let format = [0x51F3_694E_AFAA, 0x3FCA_34BE_1F26];
        let grid = layout.draw_copies(format, &stream).unwrap();
        assert_eq!(layout.read_stream(&grid).unwrap(), stream, "{w} x {h}");
        assert_eq!(read_format_copies(&grid).unwrap(), format, "{w} x {h}");
    }
    check_round_trip(
        &Layout::new(1000, 600).unwrap(),
        [0x7FFF_FFFF_FFFF, 0],
        &stream_from_seed(3, 73717),
    );
}
