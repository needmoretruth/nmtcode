//! Format word tests: the worked examples of 2.8 and annex A (A.2.5), the masks of 2.5, the
//! reader rules of 2.3 and 2.7, and decoding with random bit errors and erasures.

use nmtcode_core::{
    Error, FORMAT_MASK_A, FORMAT_MASK_B, FORMAT_MASKS, FormatSample, FormatWord, FormatWordError,
    SymbolClass, decode_format, decode_format_with, format_codeword, format_parity,
    format_syndrome,
};
use proptest::prelude::*;
use proptest::sample::subsequence;
use sha2::{Digest, Sha256};

/// The 47 bits of a codeword.
const ALL_BITS: u64 = (1 << 47) - 1;

/// The 28-bit data word of 2.2 from its fields, including values a generator must not write.
fn data(version: u32, class: u32, w: u32, h: u32, level: u32, colour: u32, cell: u32) -> u32 {
    (version << 26) | (class << 25) | (w << 15) | (h << 5) | (level << 3) | (colour << 1) | cell
}

/// The sent words [`F_A`, `F_B`] of any data word (2.4.3, 2.5).
fn sent(data: u32) -> [u64; 2] {
    FORMAT_MASKS.map(|mask| format_codeword(data) ^ mask)
}

/// Both copies of any data word, as sampled without error.
fn both(data: u32) -> [Option<u64>; 2] {
    sent(data).map(Some)
}

/// Copy A alone.
fn only_a(word: u64) -> [Option<u64>; 2] {
    [Some(word), None]
}

/// Copy B alone.
fn only_b(word: u64) -> [Option<u64>; 2] {
    [None, Some(word)]
}

/// Flips the bits at the given indices of 2.4.3 (index i is integer bit 46 − i).
fn flip(word: u64, indices: &[u32]) -> u64 {
    indices.iter().fold(word, |w, &i| w ^ (1 << (46 - i)))
}

/// The bits at the given indices of 2.4.3, as a mask.
fn at(indices: &[u32]) -> u64 {
    flip(0, indices)
}

fn static_word(width: u32, height: u32, level: u8) -> FormatWord {
    FormatWord::new(SymbolClass::Static, width, height, level, 0, 0).unwrap()
}

/// The least number of bit errors that separates each of the 2^19 syndromes from a codeword:
/// the minimum weight of each coset of the (47, 28) code.
fn coset_weights() -> Vec<u8> {
    let singles: Vec<u32> = (0..47).map(|j| format_syndrome(1 << j)).collect();
    let mut weight = vec![u8::MAX; 1 << 19];
    weight[0] = 0;
    let mut frontier = vec![0u32];
    let mut w = 0;
    while !frontier.is_empty() {
        w += 1;
        let mut next = Vec::new();
        for s in frontier {
            for &t in &singles {
                let u = usize::try_from(s ^ t).unwrap();
                if weight[u] == u8::MAX {
                    weight[u] = w;
                    next.push(s ^ t);
                }
            }
        }
        frontier = next;
    }
    weight
}

fn coset_weight(weights: &[u8], word: u64) -> u8 {
    weights[usize::try_from(format_syndrome(word)).unwrap()]
}

#[test]
fn worked_example_2_8() {
    let word = static_word(20, 20, 1);
    assert_eq!((word.width_code(), word.height_code()), (1, 1));
    assert_eq!(word.data(), 0x000_8028);
    assert_eq!(format_parity(word.data()), 0x3_9567);
    assert_eq!(word.codeword(), 0x0004_0143_9567);
    assert_eq!(FORMAT_MASK_A, 0x51F3_694E_AFAA);
    assert_eq!(FORMAT_MASK_B, 0x3FCA_34BE_1F26);
    assert_eq!(word.encode(), 0x51F7_680D_3ACD);
    assert_eq!(word.encode_copies(), [0x51F7_680D_3ACD, 0x3FCE_35FD_8A41]);

    assert_eq!(format!("{:028b}", word.data()), "0000000000001000000000101000");
    assert_eq!(format!("{:019b}", format_parity(word.data())), "0111001010101100111");
    assert_eq!(
        format!("{:047b}", word.codeword()),
        "00000000000010000000001010000111001010101100111"
    );
    assert_eq!(format!("{FORMAT_MASK_A:047b}"), "10100011111001101101001010011101010111110101010");
    assert_eq!(format!("{FORMAT_MASK_B:047b}"), "01111111100101000110100101111100001111100100110");
    let [f_a, f_b] = word.encode_copies();
    assert_eq!(format!("{f_a:047b}"), "10100011111011101101000000011010011101011001101");
    assert_eq!(format!("{f_b:047b}"), "01111111100111000110101111111011000101001000001");

    // Bit 0 and bit 46 of `F_A` are 1 (modules A[0] and A[46] dark); bit 0 of `F_B` is 0 and bit 46
    // is 1 (module B[0] light, B[46] dark).
    assert_eq!(((f_a >> 46) & 1, f_a & 1), (1, 1));
    assert_eq!(((f_b >> 46) & 1, f_b & 1), (0, 1));
    assert_eq!((f_a >> 47, f_b >> 47), (0, 0));

    let decoded = decode_format(both(word.data())).unwrap();
    assert_eq!((decoded.word, decoded.errors, decoded.copy, decoded.both), (word, 0, 0, true));
}

#[test]
fn worked_example_2_8_decoding_with_errors() {
    let word = static_word(20, 20, 1);
    let [f_a, f_b] = word.encode_copies();
    let read_a = flip(f_a, &[3, 17, 30]);
    assert_eq!(read_a, 0x59F7_480C_3ACD);
    assert_eq!(format_syndrome(read_a ^ FORMAT_MASK_A), 0x1_135D);

    let a_alone = decode_format(only_a(read_a)).unwrap();
    assert_eq!(a_alone.word, word);
    assert_eq!(a_alone.word.data(), 0x000_8028);
    assert_eq!((a_alone.errors, a_alone.copy, a_alone.both), (3, 0, false));

    // Copy B read without error: both copies give the same word, and copy B has the smaller e.
    let decoded = decode_format([Some(read_a), Some(f_b)]).unwrap();
    assert_eq!(decoded.word, word);
    assert_eq!((decoded.errors, decoded.copy, decoded.both), (0, 1, true));
}

#[test]
fn worked_example_2_8_erasures() {
    // Bits 3, 17, 30 and 44 of copy A are marked as erasures and bit 5 is read wrong:
    // 2e + s = 2 + 4 = 6 ≤ 7.
    let word = static_word(20, 20, 1);
    let f_a = word.encode();
    let read = flip(f_a, &[5, 17, 30]);
    let erased = at(&[3, 17, 30, 44]);
    assert_eq!((read, erased), (0x53F7_480C_3ACD, 0x0800_2001_0004));
    let sample = FormatSample::with_erasures(read, erased);
    let decoded = decode_format_with([Some(sample), None], |_| true).unwrap();
    assert_eq!((decoded.word, decoded.errors, decoded.erasures), (word, 1, 4));
    // Without the erasures the three errors at bits 5, 17 and 30 are still corrected.
    assert_eq!(decode_format(only_a(read)).unwrap().errors, 3);
    // Two errors with four erasures is 2e + s = 8: not decoded.
    let two = FormatSample::with_erasures(flip(f_a, &[5, 6]), erased);
    assert_eq!(decode_format_with([Some(two), None], |_| true), Err(Error::FormatUnreadable));
    // Five erasures are more than a reader may mark.
    let five = FormatSample::with_erasures(f_a, at(&[1, 2, 3, 4, 5]));
    assert_eq!(decode_format_with([Some(five), None], |_| true), Err(Error::FormatUnreadable));
}

#[test]
fn worked_example_2_8_turned_symbol() {
    // A reader that takes the symbol turned by 180° for upright reads `F_B` where copy A should
    // be and `F_A` where copy B should be. Neither decodes.
    let word = static_word(20, 20, 1);
    let [f_a, f_b] = word.encode_copies();
    assert_eq!(format_syndrome(f_b ^ FORMAT_MASK_A), 0x6_5BC8);
    assert_eq!(format_syndrome(f_a ^ FORMAT_MASK_B), 0x6_5BC8);
    assert_eq!(decode_format([Some(f_b), Some(f_a)]), Err(Error::FormatUnreadable));
    // Even with 4 erasures on the worst positions: 2 · (6 − 4) + 4 = 8.
    let weights = coset_weights();
    assert_eq!(coset_weight(&weights, f_b ^ FORMAT_MASK_A), 6);
}

#[test]
fn worked_example_2_8_reflectance_reversed() {
    // Every bit inverted: R = NOT U under either mask, 3 bits from U XOR c44.
    let word = static_word(20, 20, 1);
    let [f_a, f_b] = word.encode_copies();
    let (inv_a, inv_b) = (!f_a & ALL_BITS, !f_b & ALL_BITS);
    assert_eq!((inv_a, inv_b), (0x2E08_97F2_C532, 0x4031_CA02_75BE));
    // c44, the one codeword of weight 44: zeros at bit indices 1, 13 and 31.
    let c44 = ALL_BITS ^ at(&[1, 13, 31]);
    assert_eq!(format_syndrome(c44), 0);
    assert_eq!(c44 >> 19, 0xBFF_BFFF);
    assert_eq!((word.codeword() ^ c44) >> 19, 0xBFF_3FD7);
    // One inverted copy alone: format version 2 with e = 3 > 2, not reported as newer.
    assert_eq!(decode_format(only_a(inv_a)), Err(Error::FormatUnreadable));
    assert_eq!(decode_format(only_b(inv_b)), Err(Error::FormatUnreadable));
    // Both inverted copies decode to the same word, so step 5 reports a newer version; the
    // polarity rule of 1.4 keeps a reader from reading an inverted symbol this way.
    assert_eq!(decode_format([Some(inv_a), Some(inv_b)]), Err(Error::FormatVersion));
}

#[test]
fn worked_example_annex_a_2_5() {
    let word = static_word(20, 28, 0);
    assert_eq!((word.width_code(), word.height_code()), (1, 3));
    assert_eq!(word.data(), 0x000_8060);
    assert_eq!(format_parity(word.data()), 0x4_66EB);
    assert_eq!(word.codeword(), 0x0004_0304_66EB);
    assert_eq!(word.encode(), 0x51F7_6A4A_C941);
    assert_eq!(word.encode_copies(), [0x51F7_6A4A_C941, 0x3FCE_37BA_79CD]);
}

#[test]
fn masks_are_derived_as_2_5_says() {
    // Bits 47·index to 47·index + 46 of a 256-bit digest, bit 0 the most significant.
    let piece = |digest: &[u8], index: usize| {
        let bit = |i: usize| u64::from((digest[i / 8] >> (7 - i % 8)) & 1);
        (47 * index..47 * index + 47).fold(0u64, |acc, i| (acc << 1) | bit(i))
    };
    let a = Sha256::digest(b"NMT Code format mask");
    let b = Sha256::digest(b"NMT Code format mask B");
    assert_eq!(piece(&a, 0), FORMAT_MASK_A);
    assert_eq!(piece(&b, 2), FORMAT_MASK_B);

    let weights = coset_weights();
    let pieces: Vec<u64> = (0..3).map(|i| piece(&b, i)).collect();
    assert_eq!(pieces, [0x2417_0E65_B938, 0x7503_E72D_AFCA, FORMAT_MASK_B]);
    // The first two pieces leave `MASK_A` XOR `MASK_B` in a coset of weight 4.
    assert_eq!(coset_weight(&weights, FORMAT_MASK_A ^ pieces[0]), 4);
    assert_eq!(coset_weight(&weights, FORMAT_MASK_A ^ pieces[1]), 4);
    assert_eq!(FORMAT_MASK_A ^ FORMAT_MASK_B, 0x6E39_5DF0_B08C);
    assert_eq!(coset_weight(&weights, FORMAT_MASK_A ^ FORMAT_MASK_B), 6);

    // All-light and all-dark copies: distances 5 and 6 for copy A, 5 and 4 for copy B.
    assert_eq!(coset_weight(&weights, FORMAT_MASK_A), 5);
    assert_eq!(coset_weight(&weights, !FORMAT_MASK_A & ALL_BITS), 6);
    assert_eq!(coset_weight(&weights, FORMAT_MASK_B), 5);
    assert_eq!(coset_weight(&weights, !FORMAT_MASK_B & ALL_BITS), 4);
    for copies in [[Some(0), Some(0)], [Some(ALL_BITS), Some(ALL_BITS)]] {
        assert_eq!(decode_format(copies), Err(Error::FormatUnreadable));
    }

    // Coset weights of the code: 17 344 correctable cosets (weights 0 to 3), 135 927 of weight
    // 6 or more; the covering radius is 7.
    let mut counts = [0u32; 8];
    for &w in &weights {
        counts[usize::from(w)] += 1;
    }
    assert_eq!(counts, [1, 47, 1081, 16_215, 126_685, 244_332, 134_377, 1550]);
}

#[test]
fn constructor_rejects_what_a_generator_must_not_write() {
    use FormatWordError as E;
    use SymbolClass::{Static, TransferTile};
    for side in [0, 16, 19, 21, 22, 4112, u32::MAX] {
        assert_eq!(FormatWord::new(Static, side, 20, 0, 0, 0), Err(E::Width(side)));
        assert_eq!(FormatWord::new(Static, 20, side, 0, 0, 0), Err(E::Height(side)));
    }
    assert_eq!(FormatWord::new(Static, 20, 20, 4, 0, 0), Err(E::Level(4)));
    assert_eq!(FormatWord::new(Static, 20, 20, 0, 2, 0), Err(E::ColourProfile(2)));
    assert_eq!(FormatWord::new(Static, 20, 20, 0, 3, 0), Err(E::ColourProfile(3)));
    assert_eq!(FormatWord::new(Static, 20, 20, 0, 1, 2), Err(E::ChromaCell(2)));
    assert_eq!(FormatWord::new(Static, 20, 20, 0, 0, 1), Err(E::ChromaCellWithoutColour));
    assert_eq!(FormatWord::new(TransferTile, 20, 20, 0, 1, 0), Err(E::TransferTileColour));

    let largest = FormatWord::new(Static, 4108, 20, 3, 1, 1).unwrap();
    assert_eq!((largest.width_code(), largest.height_code()), (1023, 1));
    assert_eq!(largest.data(), data(0, 0, 1023, 1, 3, 1, 1));
    assert_eq!(largest.chroma_cell_side(), 2);
    let tile = FormatWord::new(TransferTile, 20, 4108, 2, 0, 0).unwrap();
    assert_eq!(tile.data(), data(0, 1, 1, 1023, 2, 0, 0));
    assert_eq!(decode_format(tile.encode_copies().map(Some)).unwrap().word, tile);
}

#[test]
fn rule_2_3_invalid_version_0_fields_count_as_not_decoded() {
    let good = static_word(20, 20, 1);
    let invalid = [
        data(0, 0, 0, 1, 1, 0, 0), // width code 0 (W = 16)
        data(0, 0, 1, 0, 1, 0, 0), // height code 0 (H = 16)
        data(0, 0, 1, 1, 1, 2, 0), // colour profile 2
        data(0, 0, 1, 1, 1, 3, 1), // colour profile 3
        data(0, 0, 1, 1, 1, 0, 1), // colour profile 0 with chroma cell size 1
        data(0, 1, 0, 1, 0, 0, 0), // transfer tile with width code 0
        data(0, 1, 1, 1, 0, 1, 0), // transfer tile with colour profile 1 (3.3)
    ];
    for bad in invalid {
        let [bad_a, bad_b] = sent(bad);
        // Error-free, the invalid copy is still not decoded.
        assert_eq!(decode_format(only_a(bad_a)), Err(Error::FormatUnreadable), "{bad:#x}");
        assert_eq!(decode_format(both(bad)), Err(Error::FormatUnreadable));
        // The other copy is used even with more errors.
        let [_, good_b] = good.encode_copies();
        let chosen = decode_format([Some(bad_a), Some(flip(good_b, &[0, 20, 46]))]).unwrap();
        assert_eq!((chosen.word, chosen.errors, chosen.copy, chosen.both), (good, 3, 1, false));
        let chosen = decode_format([Some(good.encode()), Some(bad_b)]).unwrap();
        assert_eq!((chosen.word, chosen.copy, chosen.both), (good, 0, false));
    }
}

#[test]
fn rule_2_7_step_3_checks_beyond_the_format_word() {
    // `accept` stands for the reader's checks of W and H against the finders and of the colour
    // layer size (2.7, step 3): a refused copy counts as not decoded.
    let small = static_word(20, 20, 1);
    let large = static_word(4108, 4108, 1);
    let fits = |word: &FormatWord| word.width() == 20 && word.height() == 20;
    let samples = |a: u64, b: u64| [Some(FormatSample::new(a)), Some(FormatSample::new(b))];
    let [_, small_b] = small.encode_copies();
    let [large_a, large_b] = large.encode_copies();
    let chosen = decode_format_with(samples(large_a, small_b), fits).unwrap();
    assert_eq!((chosen.word, chosen.copy, chosen.both), (small, 1, false));
    assert_eq!(decode_format_with(samples(large_a, large_b), fits), Err(Error::FormatUnreadable));
    // Without the check, the two copies disagree and the symbol is rejected.
    assert_eq!(decode_format([Some(large_a), Some(small_b)]), Err(Error::FormatConflict));
}

#[test]
fn rule_2_3_newer_format_version_rejects_the_symbol() {
    for version in 1..=3 {
        // Only the version field is read: invalid other fields do not make it undecoded.
        for word in [data(version, 0, 1, 1, 0, 0, 0), data(version, 1, 0, 0, 3, 3, 1)] {
            let [a, b] = sent(word);
            assert_eq!(decode_format(both(word)), Err(Error::FormatVersion));
            assert_eq!(decode_format(only_a(a)), Err(Error::FormatVersion));
            // One copy with e = 2 is still reported as newer; with e = 3 it is not.
            assert_eq!(decode_format(only_b(flip(b, &[5, 6]))), Err(Error::FormatVersion));
            assert_eq!(decode_format(only_b(flip(b, &[5, 6, 7]))), Err(Error::FormatUnreadable));
            // Both copies agree: reported as newer at e = 3.
            assert_eq!(
                decode_format([Some(flip(a, &[1, 2, 3])), Some(flip(b, &[4, 5, 6]))]),
                Err(Error::FormatVersion)
            );
            // With erasures: 2e + s ≤ 4 is reported, 2e + s = 5 is not.
            let with = |errors: &[u32], erased: &[u32]| {
                let sample = FormatSample::with_erasures(flip(a, errors), at(erased));
                decode_format_with([Some(sample), None], |_| true)
            };
            assert_eq!(with(&[9], &[10, 11]), Err(Error::FormatVersion));
            assert_eq!(with(&[9], &[10, 11, 12]), Err(Error::FormatUnreadable));
        }
    }
    let good = static_word(20, 20, 1);
    let [newer_a, newer_b] = sent(data(1, 0, 1, 1, 1, 0, 0));
    // Two decoded copies that differ reject the symbol, whatever their versions.
    assert_eq!(decode_format([Some(good.encode()), Some(newer_b)]), Err(Error::FormatConflict));
    assert_eq!(
        decode_format([Some(newer_a), Some(good.encode_copies()[1])]),
        Err(Error::FormatConflict)
    );
}

#[test]
fn rule_2_3_transfer_tile_and_colour_profile_are_returned() {
    let decoded = decode_format(both(data(0, 1, 1, 1, 0, 0, 0))).unwrap();
    assert_eq!(decoded.word.class(), SymbolClass::TransferTile);

    for cell in 0..=1 {
        let colour = decode_format(both(data(0, 0, 2, 2, 0, 1, cell))).unwrap();
        assert_eq!(colour.word.colour_profile(), 1);
        assert_eq!(u32::from(colour.word.chroma_cell()), cell);
    }
}

#[test]
fn step_4_two_different_words_reject_the_symbol() {
    let x = static_word(20, 20, 1);
    let y = static_word(24, 24, 0);
    let (x_a, y_b) = (x.encode_copies()[0], y.encode_copies()[1]);
    assert_eq!(decode_format([Some(x_a), Some(y_b)]), Err(Error::FormatConflict));
    assert_eq!(
        decode_format([Some(flip(x_a, &[1, 2])), Some(flip(y_b, &[3]))]),
        Err(Error::FormatConflict)
    );
    // Level 0 in copy A and level 3 in copy B, the case of chapter 3 (3.2.2).
    let level_0 = static_word(48, 48, 0).encode_copies()[0];
    let level_3 = static_word(48, 48, 3).encode_copies()[1];
    assert_eq!(decode_format([Some(level_0), Some(level_3)]), Err(Error::FormatConflict));
    assert_eq!(Error::FormatConflict.name(), "E_FORMAT_CONFLICT");
}

#[test]
fn blank_and_solid_copies_do_not_decode() {
    for copies in [[Some(0), None], [None, Some(0)], [Some(ALL_BITS), None], [None, None]] {
        assert_eq!(decode_format(copies), Err(Error::FormatUnreadable));
    }
    assert_eq!(decode_format([Some(0), Some(ALL_BITS)]), Err(Error::FormatUnreadable));
}

#[test]
fn bits_above_47_are_ignored() {
    let word = static_word(20, 20, 1);
    let decoded = decode_format(only_a(word.encode() | (0xFFFF << 47))).unwrap();
    assert_eq!((decoded.word, decoded.errors), (word, 0));
    let sample = FormatSample::with_erasures(word.encode_copies()[1], 0xFFFF << 47);
    let decoded = decode_format_with([None, Some(sample)], |_| true).unwrap();
    assert_eq!((decoded.word, decoded.erasures, decoded.copy), (word, 0, 1));
}

/// Any word a generator may write.
fn any_word() -> impl Strategy<Value = FormatWord> {
    (any::<bool>(), 1u32..=1023, 1u32..=1023, 0u8..=3, 0u8..3).prop_map(
        |(tile, w, h, level, colour_and_cell)| {
            // 0: profile 0; 1: profile 1, cell 0; 2: profile 1, cell 1. A tile uses profile 0.
            let (colour, cell) = match (tile, colour_and_cell) {
                (true, _) | (false, 0) => (0, 0),
                (false, 1) => (1, 0),
                (false, _) => (1, 1),
            };
            let class = if tile { SymbolClass::TransferTile } else { SymbolClass::Static };
            FormatWord::new(class, 4 * (w + 4), 4 * (h + 4), level, colour, cell).unwrap()
        },
    )
}

/// Between `min` and `max` distinct bit indices of 2.4.3.
fn error_bits(min: usize, max: usize) -> impl Strategy<Value = Vec<u32>> {
    subsequence((0..47).collect::<Vec<u32>>(), min..=max)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn corrects_up_to_three_errors_in_both_copies(
        word in any_word(),
        errors_a in error_bits(0, 3),
        errors_b in error_bits(0, 3),
    ) {
        let [a, b] = word.encode_copies();
        let decoded = decode_format([Some(flip(a, &errors_a)), Some(flip(b, &errors_b))]).unwrap();
        let (e_a, e_b) = (errors_a.len(), errors_b.len());
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!(decoded.errors as usize, e_a.min(e_b));
        prop_assert_eq!(decoded.copy, usize::from(e_b < e_a));
        prop_assert!(decoded.both);
    }

    #[test]
    fn corrects_errors_and_erasures_within_the_bound(
        word in any_word(),
        positions in error_bits(0, 7),
        erasures in 0usize..=4,
        copy in 0usize..2,
    ) {
        // The first `s` positions are erased (their read value is inverted), the next `e` are
        // errors, with 2e + s ≤ 7.
        let s = erasures.min(positions.len());
        let e = ((7 - s) / 2).min(positions.len() - s);
        let (erased, errors) = positions.split_at(s);
        let errors = &errors[..e];
        let sent = word.encode_copies()[copy];
        let read = flip(flip(sent, erased), errors);
        let sample = FormatSample::with_erasures(read, at(erased));
        let mut copies = [None, None];
        copies[copy] = Some(sample);
        let decoded = decode_format_with(copies, |_| true).unwrap();
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!((decoded.errors as usize, decoded.erasures as usize), (e, s));
        prop_assert_eq!(decoded.copy, copy);
    }

    #[test]
    fn four_errors_never_decode(
        word in any_word(),
        four in error_bits(4, 4),
        errors_b in error_bits(0, 3),
    ) {
        // Every pattern of 4 bit errors is detected (2.4.1).
        let [a, b] = word.encode_copies();
        prop_assert_eq!(decode_format(only_a(flip(a, &four))), Err(Error::FormatUnreadable));
        // With copy A lost to 4 errors, copy B decides.
        let decoded = decode_format([Some(flip(a, &four)), Some(flip(b, &errors_b))]).unwrap();
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!(decoded.copy, 1);
        prop_assert!(!decoded.both);
        prop_assert_eq!(decoded.errors as usize, errors_b.len());
    }

    #[test]
    fn a_copy_read_with_the_other_mask_never_decodes(word in any_word(), errors in error_bits(0, 2)) {
        // `MASK_A` XOR `MASK_B` lies in a coset of weight 6: a turned symbol read as upright, with up
        // to 2 more bit errors, is still more than 3 errors from every codeword.
        let [a, b] = word.encode_copies();
        prop_assert_eq!(decode_format(only_a(flip(b, &errors))), Err(Error::FormatUnreadable));
        prop_assert_eq!(decode_format(only_b(flip(a, &errors))), Err(Error::FormatUnreadable));
    }

    #[test]
    fn arbitrary_samples_never_panic(
        bits in any::<[u64; 2]>(),
        erasures in any::<[u64; 2]>(),
        present in any::<[bool; 2]>(),
    ) {
        let copies = [0, 1].map(|i| {
            present[i].then(|| FormatSample::with_erasures(bits[i], erasures[i]))
        });
        if let Ok(decoded) = decode_format_with(copies, |_| true) {
            prop_assert!(2 * decoded.errors + decoded.erasures <= 7);
            prop_assert!(decoded.erasures <= 4);
            prop_assert!(decoded.copy < 2);
        }
    }
}
