//! Format word tests: the worked examples of 2.8 and annex A (A.2.5), the reader rules of 2.3
//! and 2.7, and decoding with random bit errors.

use nmtcode_core::{
    Error, FORMAT_MASK, FormatWord, FormatWordError, SymbolClass, decode_format, format_codeword,
    format_parity, format_syndrome,
};
use proptest::prelude::*;
use proptest::sample::subsequence;

/// The 28-bit data word of 2.2 from its fields, including values a generator must not write.
fn data(version: u32, class: u32, w: u32, h: u32, level: u32, colour: u32, cell: u32) -> u32 {
    (version << 26) | (class << 25) | (w << 15) | (h << 5) | (level << 3) | (colour << 1) | cell
}

/// The sent word F of any data word (2.4.3, 2.5).
fn sent(data: u32) -> u64 {
    format_codeword(data) ^ FORMAT_MASK
}

/// Flips the bits at the given indices of 2.4.3 (index i is integer bit 46 − i).
fn flip(word: u64, indices: &[u32]) -> u64 {
    indices.iter().fold(word, |w, &i| w ^ (1 << (46 - i)))
}

fn static_word(width: u32, height: u32, level: u8) -> FormatWord {
    FormatWord::new(SymbolClass::Static, width, height, level, 0, 0).unwrap()
}

#[test]
fn worked_example_2_8() {
    let word = static_word(20, 20, 1);
    assert_eq!((word.width_code(), word.height_code()), (1, 1));
    assert_eq!(word.data(), 0x000_8028);
    assert_eq!(format_parity(word.data()), 0x3_9567);
    assert_eq!(word.codeword(), 0x0004_0143_9567);
    assert_eq!(FORMAT_MASK, 0x51F3_694E_AFAA);
    assert_eq!(word.encode(), 0x51F7_680D_3ACD);

    assert_eq!(format!("{:028b}", word.data()), "0000000000001000000000101000");
    assert_eq!(format!("{:019b}", format_parity(word.data())), "0111001010101100111");
    assert_eq!(
        format!("{:047b}", word.codeword()),
        "00000000000010000000001010000111001010101100111"
    );
    assert_eq!(format!("{FORMAT_MASK:047b}"), "10100011111001101101001010011101010111110101010");
    assert_eq!(
        format!("{:047b}", word.encode()),
        "10100011111011101101000000011010011101011001101"
    );

    // Bit 0 of F is 1 (module A[0] dark) and bit 46 is 1 (module A[46] dark).
    let f = word.encode();
    assert_eq!((f >> 46) & 1, 1);
    assert_eq!(f & 1, 1);
    assert_eq!(f >> 47, 0);
}

#[test]
fn worked_example_2_8_decoding_with_errors() {
    let word = static_word(20, 20, 1);
    let f = word.encode();
    let read_a = flip(f, &[3, 17, 30]);
    assert_eq!(read_a, 0x59F7_480C_3ACD);
    assert_eq!(format_syndrome(read_a ^ FORMAT_MASK), 0x1_135D);

    let only_a = decode_format(&[read_a]).unwrap();
    assert_eq!(only_a.word, word);
    assert_eq!(only_a.word.data(), 0x000_8028);
    assert_eq!((only_a.errors, only_a.copy), (3, 0));

    // Copy B read without error: step 4 prefers copy B; both give the same word.
    let both = decode_format(&[read_a, f]).unwrap();
    assert_eq!(both.word, word);
    assert_eq!((both.errors, both.copy), (0, 1));
    assert_eq!(both.alternative, None);
}

#[test]
fn worked_example_annex_a_2_5() {
    let word = static_word(24, 24, 0);
    assert_eq!(word.data(), 0x001_0040);
    assert_eq!(format_parity(word.data()), 0x7_D88F);
    assert_eq!(word.codeword(), 0x0008_0207_D88F);
    assert_eq!(word.encode(), 0x51FB_6B49_7725);
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
    assert_eq!(decode_format(&[tile.encode()]).unwrap().word, tile);
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
    ];
    for bad in invalid {
        // Error-free, the invalid copy is still not decoded.
        assert_eq!(decode_format(&[sent(bad)]), Err(Error::FormatUnreadable), "{bad:#x}");
        assert_eq!(decode_format(&[sent(bad), sent(bad)]), Err(Error::FormatUnreadable));
        // The other copy is used even with more errors.
        let chosen = decode_format(&[sent(bad), flip(good.encode(), &[0, 20, 46])]).unwrap();
        assert_eq!((chosen.word, chosen.errors, chosen.copy), (good, 3, 1));
        assert_eq!(chosen.alternative, None);
    }
}

#[test]
fn rule_2_3_newer_format_version_rejects_the_symbol() {
    for version in 1..=3 {
        // Only the version field is read: invalid other fields do not make it undecoded.
        for word in [data(version, 0, 1, 1, 0, 0, 0), data(version, 1, 0, 0, 3, 3, 1)] {
            assert_eq!(decode_format(&[sent(word)]), Err(Error::FormatVersion));
            assert_eq!(decode_format(&[flip(sent(word), &[5, 6])]), Err(Error::FormatVersion));
        }
    }
    let good = static_word(20, 20, 1);
    let newer = sent(data(1, 0, 1, 1, 1, 0, 0));
    // The newer word has fewer errors, so it is chosen and rejects the symbol.
    assert_eq!(decode_format(&[flip(good.encode(), &[1]), newer]), Err(Error::FormatVersion));
    // The good word has fewer errors; the newer word is not offered as an alternative.
    let chosen = decode_format(&[good.encode(), flip(newer, &[1, 2])]).unwrap();
    assert_eq!((chosen.word, chosen.copy, chosen.alternative), (good, 0, None));
}

#[test]
fn rule_2_3_transfer_tile_and_colour_profile_are_returned() {
    let tile = sent(data(0, 1, 1, 1, 0, 0, 0));
    let decoded = decode_format(&[tile]).unwrap();
    assert_eq!(decoded.word.class(), SymbolClass::TransferTile);

    for cell in 0..=1 {
        let colour = decode_format(&[sent(data(0, 0, 2, 2, 0, 1, cell))]).unwrap();
        assert_eq!(colour.word.colour_profile(), 1);
        assert_eq!(u32::from(colour.word.chroma_cell()), cell);
    }

    // A transfer tile must have colour profile 0 (3.3, 7.3).
    assert_eq!(decode_format(&[sent(data(0, 1, 1, 1, 0, 1, 0))]), Err(Error::TileColour));
}

#[test]
fn step_4_choice_between_different_words() {
    let x = static_word(20, 20, 1);
    let y = static_word(24, 24, 0);
    // Fewer errors wins.
    let chosen = decode_format(&[flip(x.encode(), &[1, 2]), flip(y.encode(), &[3])]).unwrap();
    assert_eq!((chosen.word, chosen.errors, chosen.copy), (y, 1, 1));
    assert_eq!(chosen.alternative, Some(x));
    // Equal errors: copy A wins.
    let chosen = decode_format(&[flip(x.encode(), &[7]), flip(y.encode(), &[8])]).unwrap();
    assert_eq!((chosen.word, chosen.errors, chosen.copy), (x, 1, 0));
    assert_eq!(chosen.alternative, Some(y));
}

#[test]
fn blank_solid_and_empty_input_do_not_decode() {
    let all_dark = (1u64 << 47) - 1;
    assert_eq!(decode_format(&[0]), Err(Error::FormatUnreadable));
    assert_eq!(decode_format(&[all_dark]), Err(Error::FormatUnreadable));
    assert_eq!(decode_format(&[0, all_dark]), Err(Error::FormatUnreadable));
    assert_eq!(decode_format(&[]), Err(Error::FormatUnreadable));
}

#[test]
fn bits_above_47_are_ignored() {
    let word = static_word(20, 20, 1);
    let decoded = decode_format(&[word.encode() | (0xFFFF << 47)]).unwrap();
    assert_eq!((decoded.word, decoded.errors), (word, 0));
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

/// Up to `max` distinct bit indices of 2.4.3.
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
        let copies = [flip(word.encode(), &errors_a), flip(word.encode(), &errors_b)];
        let decoded = decode_format(&copies).unwrap();
        let (e_a, e_b) = (errors_a.len(), errors_b.len());
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!(decoded.errors as usize, e_a.min(e_b));
        prop_assert_eq!(decoded.copy, usize::from(e_b < e_a));
        prop_assert_eq!(decoded.alternative, None);
    }

    #[test]
    fn corrects_up_to_three_errors_in_one_copy(word in any_word(), errors in error_bits(0, 3)) {
        let decoded = decode_format(&[flip(word.encode(), &errors)]).unwrap();
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!(decoded.errors as usize, errors.len());
    }

    #[test]
    fn four_errors_never_decode(
        word in any_word(),
        four in error_bits(4, 4),
        errors_b in error_bits(0, 3),
    ) {
        // Every pattern of 4 bit errors is detected (2.4.1).
        prop_assert_eq!(
            decode_format(&[flip(word.encode(), &four)]),
            Err(Error::FormatUnreadable)
        );
        // With copy A lost to 4 errors, copy B decides.
        let decoded = decode_format(&[flip(word.encode(), &four), flip(word.encode(), &errors_b)])
            .unwrap();
        prop_assert_eq!(decoded.word, word);
        prop_assert_eq!(decoded.copy, 1);
        prop_assert_eq!(decoded.errors as usize, errors_b.len());
    }

    #[test]
    fn arbitrary_words_never_panic(copies in proptest::collection::vec(any::<u64>(), 0..4)) {
        if let Ok(decoded) = decode_format(&copies) {
            prop_assert!(decoded.errors <= 3);
            prop_assert!(decoded.copy < copies.len());
        }
    }
}
