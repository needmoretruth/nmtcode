//! The worked examples of chapter 5, section 2.8 and annex A (A.2) as exact values.

use nmtcode_symbol::{
    FORMAT_COPY_A, FormatCopy, Layout, ModuleClass, ModuleGrid, SymbolCounts, SymbolError,
    WHITENING_SEED, Whitening, format_module, format_positions, read_format_copies,
    reference_mark_lines, write_format_copies,
};

/// Renders the classes of `layout` in the notation of 5.12: `#` dark function module, `o` light
/// module inside a finder, `-` separator, `a` / `b` format copy, `.` data module.
fn function_map(layout: &Layout) -> Vec<String> {
    let [_, copy_b] = format_positions(layout.width(), layout.height()).unwrap();
    (0..layout.height())
        .map(|y| {
            (0..layout.width())
                .map(|x| match layout.module_class(x, y).unwrap() {
                    ModuleClass::Finder | ModuleClass::ReferenceMark => {
                        if layout.function_value(x, y).unwrap() { '#' } else { 'o' }
                    }
                    ModuleClass::Separator => '-',
                    ModuleClass::Format if copy_b.contains(&(x, y)) => 'b',
                    ModuleClass::Format => 'a',
                    ModuleClass::Data => '.',
                })
                .collect()
        })
        .collect()
}

#[test]
fn reference_mark_lines_of_5_6_1() {
    assert_eq!(reference_mark_lines(48).unwrap(), [2, 24, 45]);
    assert_eq!(reference_mark_lines(64).unwrap(), [2, 22, 41, 61]);
    assert_eq!(reference_mark_lines(100).unwrap(), [2, 26, 50, 73, 97]);
    assert_eq!(reference_mark_lines(44).unwrap(), [2, 41]);
    assert_eq!(reference_mark_lines(20).unwrap(), [2, 17]);
    assert_eq!(reference_mark_lines(46), None);
    assert_eq!(reference_mark_lines(16), None);

    let layout = Layout::new(100, 48).unwrap();
    assert_eq!(layout.x_lines(), [2, 26, 50, 73, 97]);
    assert_eq!(layout.y_lines(), [2, 24, 45]);
    assert_eq!(layout.reference_mark_count(), 5 * 3 - 4);
}

/// A wide, short symbol has marks only on the top and bottom edge lines (5.6.1).
#[test]
fn marks_of_a_wide_short_symbol_lie_on_the_edge_lines() {
    let layout = Layout::new(100, 20).unwrap();
    let centres: Vec<_> = layout.reference_mark_centres().collect();
    assert_eq!(centres, [(26, 2), (50, 2), (73, 2), (26, 17), (50, 17), (73, 17)]);
    assert_eq!(layout.reference_mark_count(), 6);
    // The 3 × 3 around (26, 2): eight dark modules around a light centre.
    for y in 1..=3 {
        for x in 25..=27 {
            assert_eq!(layout.module_class(x, y), Some(ModuleClass::ReferenceMark));
            assert_eq!(layout.function_value(x, y), Some((x, y) != (26, 2)));
        }
    }
    assert_eq!(layout.module_class(24, 2), Some(ModuleClass::Data));
    assert_eq!(layout.module_class(26, 4), Some(ModuleClass::Data));
}

/// The table of 5.7: M, D, N and R from the formula and from the module-by-module layout.
#[test]
fn counts_table_of_5_7() {
    let table: [(u32, u32, usize, usize, usize, usize); 11] = [
        (20, 20, 0, 162, 20, 2),
        (20, 28, 0, 322, 40, 2),
        (24, 24, 0, 338, 42, 2),
        (28, 28, 0, 546, 68, 2),
        (32, 32, 0, 786, 98, 2),
        (48, 48, 5, 2021, 252, 5),
        (64, 64, 12, 3750, 468, 6),
        (128, 128, 45, 15741, 1967, 5),
        (324, 324, 221, 102_749, 12843, 5),
        (1000, 600, 1114, 589_736, 73717, 0),
        (4108, 4108, 29580, 16_609_206, 2_076_150, 6),
    ];
    for (w, h, m, d, n, r) in table {
        let expected =
            SymbolCounts { reference_marks: m, data_modules: d, codewords: n, remainder_bits: r };
        assert_eq!(SymbolCounts::new(w, h).unwrap(), expected, "{w} x {h} formula");
        let layout = Layout::new(w, h).unwrap();
        assert_eq!(layout.counts(), expected, "{w} x {h} layout");
        assert_eq!(layout.data_module_count(), d);
        assert_eq!(layout.codeword_count(), n);
        assert_eq!(layout.remainder_bits(), r);
        assert_eq!(layout.reference_mark_count(), m);
        assert_eq!(layout.placement().len(), d);
        assert_eq!(layout.placement().count(), d, "{w} x {h} walk");
    }
}

/// 5.7: the formula agrees with a count of `module_class` for every W and H from 20 to 200,
/// and the class sizes are 100, 44, 94 and 9 · M.
#[test]
fn formula_matches_module_count() {
    for w in (20..=200).step_by(4) {
        for h in (20..=200).step_by(4) {
            let layout = Layout::new(w, h).unwrap();
            let mut per_class = [0usize; 5];
            for y in 0..h {
                for x in 0..w {
                    let class = layout.module_class(x, y).unwrap();
                    per_class[class as usize] += 1;
                }
            }
            let counts = SymbolCounts::new(w, h).unwrap();
            assert_eq!(
                per_class,
                [100, 44, 94, 9 * counts.reference_marks, counts.data_modules],
                "{w} x {h}"
            );
            assert_eq!(layout.counts(), counts, "{w} x {h}");
        }
    }
}

#[test]
fn whitening_of_5_10_1() {
    assert_eq!(WHITENING_SEED, 0x15D9_C3FC);
    let seed_bits = "0010101110110011100001111111100";
    let first: String = Whitening::new().take(31).map(|b| if b { '1' } else { '0' }).collect();
    assert_eq!(first, seed_bits);

    let mut w = Whitening::new();
    let first_64 = (0..64).fold(0u64, |acc, _| acc << 1 | u64::from(w.next_bit()));
    assert_eq!(first_64, 0x2BB3_87F8_EC5F_707F);

    let mut bytes = Whitening::new();
    let first_64_bytes = (0..8).fold(0u64, |acc, _| acc << 8 | u64::from(bytes.next_byte()));
    assert_eq!(first_64_bytes, 0x2BB3_87F8_EC5F_707F);
}

/// w\[k\] = w\[k − 28\] XOR w\[k − 31\], and the byte form equals the bit form, far into the sequence.
#[test]
fn whitening_recurrence_and_byte_form() {
    let bits: Vec<bool> = Whitening::new().take(200_000).collect();
    for k in 31..bits.len() {
        assert_eq!(bits[k], bits[k - 28] ^ bits[k - 31], "k = {k}");
    }
    let mut bytes = Whitening::new();
    for chunk in bits.as_chunks::<8>().0 {
        let expected = chunk.iter().fold(0u8, |acc, &b| acc << 1 | u8::from(b));
        assert_eq!(bytes.next_byte(), expected);
    }
    // Mixed use advances one sequence.
    let mut mixed = Whitening::new();
    let mut k = 0;
    while k + 9 < 10_000 {
        assert_eq!(mixed.next_bit(), bits[k]);
        let expected = bits[k + 1..k + 9].iter().fold(0u8, |acc, &b| acc << 1 | u8::from(b));
        assert_eq!(mixed.next_byte(), expected);
        k += 9;
    }
    // The colour layer's seed (chapter 7, 7.8.4).
    let mut chroma = Whitening::with_seed(0x644E_9D0D);
    let first_64 = (0..8).fold(0u64, |acc, _| acc << 8 | u64::from(chroma.next_byte()));
    assert_eq!(first_64, 0xC89D_3A1B_18E9_D587);
}

/// The module map of 5.12.
#[test]
fn function_module_map_of_5_12() {
    let expected = [
        "#####-aaaa....-#####",
        "#oo##-aaaa....-##oo#",
        "#oo##-aaaa....-#####",
        "#####-aaaa....-##oo#",
        "#####-aaaa....-#####",
        "------aaa.....------",
        "aaaaaa..............",
        "aaaaaa..............",
        "aaaaaa..............",
        "aaaaaa..............",
        "..............bbbbbb",
        "..............bbbbbb",
        "..............bbbbbb",
        "..............bbbbbb",
        "------.....bbb------",
        "#####-....bbbb-#####",
        "#####-....bbbb-##oo#",
        "#####-....bbbb-#ooo#",
        "#####-....bbbb-#ooo#",
        "#####-....bbbb-#####",
    ];
    let layout = Layout::new(20, 20).unwrap();
    assert_eq!(function_map(&layout), expected);
    assert_eq!(layout.reference_mark_count(), 0);
    assert_eq!(
        (layout.data_module_count(), layout.codeword_count(), layout.remainder_bits()),
        (162, 20, 2)
    );
}

/// One row of the table of 5.12: k, P[k], codeword index, b[k], w[k], module value.
type PlacementRow = (usize, (u32, u32), usize, u8, u8, u8);

/// The placement, codeword bits, whitening and module values of the first 32 data modules and
/// the two remainder modules of 5.12.
#[test]
fn placement_and_whitening_of_5_12() {
    let table: [PlacementRow; 32] = [
        (0, (0, 10), 0, 0, 0, 0),
        (1, (1, 10), 0, 1, 0, 1),
        (2, (0, 11), 0, 0, 1, 1),
        (3, (1, 11), 0, 0, 0, 0),
        (4, (0, 12), 0, 1, 1, 0),
        (5, (1, 12), 0, 1, 0, 1),
        (6, (0, 13), 0, 1, 1, 0),
        (7, (1, 13), 0, 0, 1, 1),
        (8, (2, 13), 1, 0, 1, 1),
        (9, (3, 13), 1, 1, 0, 1),
        (10, (2, 12), 1, 0, 1, 1),
        (11, (3, 12), 1, 0, 1, 1),
        (12, (2, 11), 1, 1, 0, 1),
        (13, (3, 11), 1, 1, 0, 1),
        (14, (2, 10), 1, 0, 1, 1),
        (15, (3, 10), 1, 1, 1, 0),
        (16, (4, 10), 2, 0, 1, 1),
        (17, (5, 10), 2, 1, 0, 1),
        (18, (4, 11), 2, 0, 0, 0),
        (19, (5, 11), 2, 1, 0, 1),
        (20, (4, 12), 2, 0, 0, 0),
        (21, (5, 12), 2, 1, 1, 0),
        (22, (4, 13), 2, 0, 1, 1),
        (23, (5, 13), 2, 0, 1, 1),
        (24, (6, 19), 3, 0, 1, 1),
        (25, (7, 19), 3, 0, 1, 1),
        (26, (6, 18), 3, 1, 1, 0),
        (27, (7, 18), 3, 0, 1, 1),
        (28, (6, 17), 3, 0, 1, 1),
        (29, (7, 17), 3, 0, 0, 0),
        (30, (6, 16), 3, 0, 0, 0),
        (31, (7, 16), 3, 0, 0, 0),
    ];
    let layout = Layout::new(20, 20).unwrap();
    let placement: Vec<(u32, u32)> = layout.placement().collect();
    let whitening: Vec<bool> = Whitening::new().take(placement.len()).collect();
    let mut stream = vec![0u8; layout.codeword_count()];
    stream[..4].copy_from_slice(&[0x4E, 0x4D, 0x54, 0x20]);
    // The rest of the stream is not given in 5.12; any values leave the first 32 modules alone.
    for (i, byte) in stream.iter_mut().enumerate().skip(4) {
        *byte = u8::try_from(i * 37 % 256).unwrap();
    }
    let grid = layout.draw_copies([0, 0], &stream).unwrap();

    for (k, p, codeword, b, w, module) in table {
        assert_eq!(placement[k], p, "P[{k}]");
        assert_eq!(k / 8, codeword, "codeword of k = {k}");
        assert_eq!(stream[codeword] >> (7 - k % 8) & 1, b, "b[{k}]");
        assert_eq!(u8::from(whitening[k]), w, "w[{k}]");
        assert_eq!(u8::from(grid.get(p.0, p.1).unwrap()), module, "module at P[{k}]");
        assert_eq!(layout.placement_index(p.0, p.1), Some(k));
    }

    // Column pairs 0, 1 and 2 hold 8 data modules each, rows 10 to 13.
    for pair in 0..3 {
        let in_pair: Vec<_> = placement.iter().filter(|(x, _)| x / 2 == pair).collect();
        assert_eq!(in_pair.len(), 8, "pair {pair}");
        assert!(in_pair.iter().all(|(_, y)| (10..=13).contains(y)));
    }
    // The last codeword ends at k = 159; the remainder modules follow.
    assert_eq!(8 * layout.codeword_count() - 1, 159);
    assert_eq!(placement[160..], [(18, 6), (19, 6)]);
    assert_eq!((whitening[160], whitening[161]), (true, false));
    assert_eq!(grid.get(18, 6), Some(true));
    assert_eq!(grid.get(19, 6), Some(false));
    // Whatever the stream, the remainder modules are w[160] and w[161].
    let other = layout.draw_copies([0, 0], &[0xFF; 20]).unwrap();
    assert_eq!((other.get(18, 6), other.get(19, 6)), (Some(true), Some(false)));
}

/// The explicit list of copy A in 5.5, and copy B turned by 180°.
#[test]
fn format_positions_of_5_5() {
    let table: [(u32, u32); 47] = [
        (6, 0),
        (7, 0),
        (8, 0),
        (9, 0),
        (6, 1),
        (7, 1),
        (8, 1),
        (9, 1),
        (6, 2),
        (7, 2),
        (8, 2),
        (9, 2),
        (6, 3),
        (7, 3),
        (8, 3),
        (9, 3),
        (6, 4),
        (7, 4),
        (8, 4),
        (9, 4),
        (6, 5),
        (7, 5),
        (8, 5),
        (0, 6),
        (0, 7),
        (0, 8),
        (0, 9),
        (1, 6),
        (1, 7),
        (1, 8),
        (1, 9),
        (2, 6),
        (2, 7),
        (2, 8),
        (2, 9),
        (3, 6),
        (3, 7),
        (3, 8),
        (3, 9),
        (4, 6),
        (4, 7),
        (4, 8),
        (4, 9),
        (5, 6),
        (5, 7),
        (5, 8),
        (5, 9),
    ];
    assert_eq!(FORMAT_COPY_A, table);
    assert_eq!(format_module(FormatCopy::A, 0, 20, 20), Some((6, 0)));
    assert_eq!(format_module(FormatCopy::B, 0, 20, 20), Some((13, 19)));
    assert_eq!(format_module(FormatCopy::B, 46, 20, 20), Some((14, 10)));
    assert_eq!(format_module(FormatCopy::B, 46, 36, 24), Some((30, 14)));
    assert_eq!(format_module(FormatCopy::A, 47, 20, 20), None);
    // (9, 5) and its turned image are data modules.
    let layout = Layout::new(36, 24).unwrap();
    assert_eq!(layout.module_class(9, 5), Some(ModuleClass::Data));
    assert_eq!(layout.module_class(36 - 10, 24 - 6), Some(ModuleClass::Data));
}

/// The module map of 2.8: `F_A` = 0x51F7680D3ACD on copy A and `F_B` = 0x3FCE35FD8A41 on copy B of
/// a 20 × 20 symbol, with the finder modules drawn as `F` as chapter 2 prints them.
#[test]
fn format_bits_of_2_8() {
    const F_A: u64 = 0x51F7_680D_3ACD;
    const F_B: u64 = 0x3FCE_35FD_8A41;
    let expected = [
        "FFFFF-#o#o....-FFFFF",
        "FFFFF-oo##....-FFFFF",
        "FFFFF-###o....-FFFFF",
        "FFFFF-###o....-FFFFF",
        "FFFFF-##o#....-FFFFF",
        "------ooo.....------",
        "o#o###..............",
        "o#oo##..............",
        "oo##oo..............",
        "o##oo#..............",
        "..............#ooo##",
        "..............oo#oo#",
        "..............o#oo##",
        "..............oo####",
        "------.....#o#------",
        "FFFFF-....o##o-FFFFF",
        "FFFFF-....oo##-FFFFF",
        "FFFFF-....#oo#-FFFFF",
        "FFFFF-....####-FFFFF",
        "FFFFF-....###o-FFFFF",
    ];
    assert_eq!(0x0004_0143_9567 ^ 0x51F3_694E_AFAA, F_A);
    assert_eq!(0x0004_0143_9567 ^ 0x3FCA_34BE_1F26, F_B);
    let layout = Layout::new(20, 20).unwrap();
    let grid = layout.draw_copies([F_A, F_B], &[0; 20]).unwrap();
    let map: Vec<String> = (0..20)
        .map(|y| {
            (0..20)
                .map(|x| match layout.module_class(x, y).unwrap() {
                    ModuleClass::Separator => '-',
                    ModuleClass::Data => '.',
                    ModuleClass::Finder => 'F',
                    _ => {
                        if grid.get(x, y).unwrap() {
                            '#'
                        } else {
                            'o'
                        }
                    }
                })
                .collect()
        })
        .collect();
    assert_eq!(map, expected);
    assert_eq!(read_format_copies(&grid).unwrap(), [F_A, F_B]);
    // Bit 0 is 1 in F_A and 0 in F_B; bit 46 is 1 in both.
    assert_eq!(grid.get(6, 0), Some(true));
    assert_eq!(grid.get(13, 19), Some(false));
    for (x, y) in [(5, 9), (14, 10)] {
        assert_eq!(grid.get(x, y), Some(true));
    }
}

/// The corrupted copy A of 2.8 reads back as the corrupted word, copy B as `F_B`.
#[test]
fn corrupted_copy_of_2_8_reads_raw() {
    let layout = Layout::new(20, 20).unwrap();
    let mut grid = layout.draw_copies([0x51F7_680D_3ACD, 0x3FCE_35FD_8A41], &[0; 20]).unwrap();
    for i in [3usize, 17, 30] {
        let (x, y) = format_module(FormatCopy::A, i, 20, 20).unwrap();
        grid.toggle(x, y);
    }
    assert_eq!(read_format_copies(&grid).unwrap(), [0x59F7_480C_3ACD, 0x3FCE_35FD_8A41]);
}

const A2_STREAM: [u8; 40] = [
    0x03, 0x00, 0x16, 0x00, 0x28, 0x02, 0xDE, 0xA7, 0x40, 0xBA, 0x96, 0xDC, 0xEE, 0xAA, 0xEE, 0x6B,
    0xEF, 0xDF, 0x35, 0x76, 0x47, 0xAE, 0xBA, 0xF7, 0x91, 0x13, 0x7D, 0xCF, 0xC4, 0xEC, 0x11, 0xEC,
    0x11, 0xEC, 0x83, 0x8C, 0xFB, 0x28, 0xC9, 0xED,
];
/// A.2.5: the codeword U and the sent words `F_A` and `F_B`.
const A2_CODEWORD: u64 = 0x0004_0304_66EB;
const A2_FORMAT: [u64; 2] = [0x51F7_6A4A_C941, 0x3FCE_37BA_79CD];
const A2_ROWS: [u32; 28] = [
    0xF_A89F, 0x9_8CD9, 0x9_BA9F, 0xF_BA19, 0xF_B4DF, 0x0_1200, 0x7_12C2, 0xA_97CB, 0x4_331F,
    0x1_54B3, 0x1_A547, 0xB_A4AD, 0x9_C8BC, 0x1_8AC9, 0x8_C65D, 0xD_4A79, 0x0_B1B4, 0xF_0EF1,
    0x8_5C2D, 0x6_7E07, 0x3_D4F4, 0x7_433B, 0x0_3FC0, 0xF_959F, 0xF_B4D9, 0xF_8251, 0xF_BFD1,
    0xF_BF9F,
];
const A2_PICTURE: [&str; 28] = [
    "#####.#.#...#..#####",
    "#..##...##..##.##..#",
    "#..##.###.#.#..#####",
    "#####.###.#....##..#",
    "#####.##.#..##.#####",
    ".......#..#.........",
    ".###...#..#.##....#.",
    "#.#.#..#.#####..#.##",
    ".#....##..##...#####",
    "...#.#.#.#..#.##..##",
    "...##.#..#.#.#...###",
    "#.###.#..#..#.#.##.#",
    "#..###..#...#.####..",
    "...##...#.#.##..#..#",
    "#...##...##..#.###.#",
    "##.#.#..#.#..####..#",
    "....#.##...##.##.#..",
    "####....###.####...#",
    "#....#.###....#.##.#",
    ".##..######......###",
    "..####.#.#..####.#..",
    ".###.#....##..###.##",
    "......########......",
    "#####..#.#.##..#####",
    "#####.##.#..##.##..#",
    "#####.....#..#.#...#",
    "#####.########.#...#",
    "#####.#######..#####",
];

fn a2_matrix() -> ModuleGrid {
    let mut grid = ModuleGrid::new(20, 28).unwrap();
    for (y, row) in (0..).zip(A2_ROWS) {
        for x in 0..20 {
            grid.set(x, y, row >> (19 - x) & 1 == 1);
        }
    }
    grid
}

/// The hexadecimal and the drawn form of A.2.7 are the same matrix.
#[test]
fn annex_a2_7_two_forms_agree() {
    assert_eq!(a2_matrix(), ModuleGrid::from_rows(&A2_PICTURE).unwrap());
}

/// A.2.7 reproduced from the format word of A.2.5 and the codeword stream of A.2.4.
#[test]
fn annex_a2_7_matrix_is_reproduced() {
    let layout = Layout::new(20, 28).unwrap();
    assert_eq!(
        (layout.data_module_count(), layout.codeword_count(), layout.remainder_bits()),
        (322, 40, 2)
    );
    assert_eq!(A2_CODEWORD ^ 0x51F3_694E_AFAA, A2_FORMAT[0]);
    assert_eq!(A2_CODEWORD ^ 0x3FCA_34BE_1F26, A2_FORMAT[1]);
    let grid = layout.draw_copies(A2_FORMAT, &A2_STREAM).unwrap();
    let expected = a2_matrix();
    assert_eq!(grid.to_rows(), expected.to_rows());
    assert_eq!(read_format_copies(&expected).unwrap(), A2_FORMAT);
    assert_eq!(layout.read_stream(&expected).unwrap(), A2_STREAM);

    // A.2.6: the remainder modules and their whitening bits.
    let placement: Vec<_> = layout.placement().collect();
    assert_eq!(placement[320..], [(18, 6), (19, 6)]);
    let w: Vec<bool> = Whitening::new().take(322).collect();
    assert_eq!((w[320], w[321]), (true, false));
    assert_eq!((expected.get(18, 6), expected.get(19, 6)), (Some(true), Some(false)));
}

/// The function and format modules of A.2.7 alone, independent of the data modules.
#[test]
fn annex_a2_7_function_and_format_modules() {
    let layout = Layout::new(20, 28).unwrap();
    let expected = a2_matrix();
    let [copy_a, copy_b] = format_positions(20, 28).unwrap();
    for y in 0..28 {
        for x in 0..20 {
            match layout.module_class(x, y).unwrap() {
                ModuleClass::Data => {}
                ModuleClass::Format => {
                    let (copy, i) = match copy_a.iter().position(|&p| p == (x, y)) {
                        Some(i) => (0, i),
                        None => (1, copy_b.iter().position(|&p| p == (x, y)).unwrap()),
                    };
                    assert_eq!(
                        expected.get(x, y),
                        Some(A2_FORMAT[copy] >> (46 - i) & 1 == 1),
                        "({x}, {y})"
                    );
                }
                _ => assert_eq!(expected.get(x, y), layout.function_value(x, y), "({x}, {y})"),
            }
        }
    }
}

#[test]
fn invalid_sizes_are_rejected() {
    for (w, h) in [(16, 20), (20, 16), (22, 20), (20, 4112), (0, 0), (4108, 4104 + 8), (21, 21)] {
        assert_eq!(Layout::new(w, h).err(), Some(SymbolError::InvalidSize { width: w, height: h }));
        assert_eq!(
            SymbolCounts::new(w, h).err(),
            Some(SymbolError::InvalidSize { width: w, height: h })
        );
    }
    assert!(Layout::new(20, 4108).is_ok());
    assert!(Layout::new(4108, 20).is_ok());
}

#[test]
fn draw_and_read_reject_bad_input() {
    let layout = Layout::new(20, 24).unwrap();
    let n = layout.codeword_count();
    let stream = vec![0u8; n];
    assert_eq!(
        layout.draw_copies([0, 0], &stream[1..]).err(),
        Some(SymbolError::StreamLength { expected: n, actual: n - 1 })
    );
    assert_eq!(
        layout.draw_copies([1 << 47, 0], &stream).err(),
        Some(SymbolError::FormatCodewordTooWide)
    );
    assert_eq!(
        layout.draw_copies([0, 1 << 47], &stream).err(),
        Some(SymbolError::FormatCodewordTooWide)
    );
    assert!(layout.draw_copies([(1 << 47) - 1, (1 << 47) - 1], &stream).is_ok());
    let mut grid = ModuleGrid::new(20, 24).unwrap();
    assert_eq!(
        write_format_copies(&mut grid, [0, 1 << 47]).err(),
        Some(SymbolError::FormatCodewordTooWide)
    );

    let other = ModuleGrid::new(24, 20).unwrap();
    assert_eq!(
        layout.read_stream(&other).err(),
        Some(SymbolError::GridSizeMismatch {
            expected_width: 20,
            expected_height: 24,
            width: 24,
            height: 20
        })
    );
    let tiny = ModuleGrid::new(9, 30).unwrap();
    assert_eq!(
        read_format_copies(&tiny).err(),
        Some(SymbolError::GridTooSmall { width: 9, height: 30 })
    );
    assert_eq!(read_format_copies(&ModuleGrid::new(10, 10).unwrap()).unwrap(), [0, 0]);

    assert_eq!(layout.module_class(20, 0), None);
    assert_eq!(layout.module_class(0, 24), None);
    assert_eq!(layout.function_value(0, 24), None);
    assert_eq!(layout.placement_index(20, 0), None);
    assert_eq!(layout.placement_index(0, 0), None);
}
