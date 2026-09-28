//! Annex A: the complete static symbol of A.2, from content to module matrix and back, the
//! nested-symbol construction of A.3 and the length-field vectors of A.4.

// The matrix rows are written exactly as A.2.7 prints them, without digit separators.
#![allow(clippy::unreadable_literal)]

use nmtcode::{
    CodecOptions, ContentType, DecodeOptions, EncodeOptions, FormatWord, ModuleGrid, Outcome,
    PresentAs, RecordForm, SizeRule, SpecError, SymbolClass, capacity, decode, encode_text,
    encode_url,
};
use nmtcode_core::{Record, RecordContent, StaticFields, crc32c, format_parity, pad_message};
use nmtcode_symbol::{Layout, ModuleClass, SymbolCounts, Whitening, read_format_copies};

const URL: &str = "https://github.com/needmoretruth/nmtcode";

/// A.2.2 (example f of 3.10).
const CONTAINER: [u8; 29] = [
    0x03, 0x00, 0x16, 0x00, 0x28, 0x02, 0xDE, 0xA7, 0x40, 0xBA, 0x96, 0xDC, 0xEE, 0xAA, 0xEE, 0x6B,
    0xEF, 0xDF, 0x35, 0x76, 0x47, 0xAE, 0xBA, 0xF7, 0x91, 0x13, 0x7D, 0xCF, 0xC4,
];

/// A.2.4: the container and 5 padding bytes.
const MESSAGE: [u8; 34] = [
    0x03, 0x00, 0x16, 0x00, 0x28, 0x02, 0xDE, 0xA7, 0x40, 0xBA, 0x96, 0xDC, 0xEE, 0xAA, 0xEE, 0x6B,
    0xEF, 0xDF, 0x35, 0x76, 0x47, 0xAE, 0xBA, 0xF7, 0x91, 0x13, 0x7D, 0xCF, 0xC4, 0xEC, 0x11, 0xEC,
    0x11, 0xEC,
];

/// A.2.4: parity p[0 … 5] of the single block.
const PARITY: [u8; 6] = [0x83, 0x8C, 0xFB, 0x28, 0xC9, 0xED];

/// A.2.5: the codeword U and the two sent words `F_A` and `F_B`.
const CODEWORD: u64 = 0x0004_0304_66EB;
const COPIES: [u64; 2] = [0x51F7_6A4A_C941, 0x3FCE_37BA_79CD];

/// A.2.7: row y as 20 bits, x = 0 the most significant.
const MATRIX_HEX: [u32; 28] = [
    0xFA89F, 0x98CD9, 0x9BA9F, 0xFBA19, 0xFB4DF, 0x01200, 0x712C2, 0xA97CB, 0x4331F, 0x154B3,
    0x1A547, 0xBA4AD, 0x9C8BC, 0x18AC9, 0x8C65D, 0xD4A79, 0x0B1B4, 0xF0EF1, 0x85C2D, 0x67E07,
    0x3D4F4, 0x7433B, 0x03FC0, 0xF959F, 0xFB4D9, 0xF8251, 0xFBFD1, 0xFBF9F,
];

/// A.2.7, the drawn form.
const MATRIX_ROWS: [&str; 28] = [
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

fn matrix_from_hex() -> ModuleGrid {
    let mut grid = ModuleGrid::new(20, 28).unwrap();
    for (y, row) in (0u32..).zip(MATRIX_HEX) {
        for x in 0..20 {
            grid.set(x, y, (row >> (19 - x)) & 1 == 1);
        }
    }
    grid
}

fn level0() -> EncodeOptions {
    EncodeOptions { level: Some(0), ..EncodeOptions::default() }
}

#[test]
fn a2_matrix_hex_and_drawn_forms_agree() {
    assert_eq!(matrix_from_hex(), ModuleGrid::from_rows(&MATRIX_ROWS).unwrap());
}

/// A.2: the input table is the default of the `screen` profile.
#[test]
fn a2_choices_are_the_screen_defaults() {
    let defaults = EncodeOptions::default();
    assert_eq!(defaults.profile.name(), "screen");
    assert_eq!(defaults.effective_level(), 0);
    assert_eq!(defaults.size, SizeRule::Recommended);
    assert_eq!(encode_url(URL, &defaults).unwrap(), encode_url(URL, &level0()).unwrap());
}

/// A.2.1: the container size of every applicable codec, and the choice.
#[test]
fn a2_1_codec_choice() {
    let record = Record { content_type: ContentType::URL, value: URL.as_bytes() };
    let content = RecordContent::from_records(&[record]).unwrap();
    assert_eq!(content.form, RecordForm::Single(ContentType::URL));
    assert!(!content.stored_only);
    let candidates = nmtcode_payload::candidates(&content.decoded, &CodecOptions::default());
    let size_of = |codec: u32| {
        let coded = candidates.iter().find(|c| c.codec == codec)?;
        let fields = StaticFields {
            form: content.form,
            codec,
            dictionary: coded.dictionary,
            decoded_len: coded.decoded_len,
        };
        fields.container_len(coded.bytes.len()).ok()
    };
    assert_eq!(size_of(0), Some(48));
    assert_eq!(size_of(1), None, "codec 1 does not apply to lower-case letters");
    assert_eq!(size_of(2), None, "codec 2 does not apply to lower-case letters");
    assert_eq!(size_of(3), Some(29));
    assert_eq!(size_of(4), Some(45));
    // Informative in A.2.1 (one brotli implementation at quality 11); this one agrees.
    assert_eq!(size_of(5), Some(50));

    let symbol = encode_url(URL, &level0()).unwrap();
    assert_eq!(symbol.codec(), 3);
    assert_eq!(symbol.dictionary(), 0);
    assert_eq!(symbol.decoded_len(), 40);
    assert_eq!(symbol.coded_len(), 19);
    assert_eq!(symbol.record_form(), RecordForm::Single(ContentType::URL));
}

/// A.2.2 to A.2.7: every intermediate value, then the matrix.
#[test]
fn a2_encoding_gives_every_listed_value_and_the_matrix() {
    let symbol = encode_url(URL, &level0()).unwrap();

    // A.2.2
    assert_eq!(symbol.container(), CONTAINER);
    assert_eq!(symbol.container_len(), 29);
    assert_eq!(crc32c(&CONTAINER[..25]), 0x137D_CFC4);

    // A.2.3: the candidates of the size rule, then the chosen size and its split.
    for (width, height, codewords, k) in [
        (20, 20, 20, 16),
        (20, 24, 30, 24),
        (24, 20, 30, 24),
        (20, 28, 40, 34),
        (28, 20, 40, 34),
        (24, 24, 42, 34),
    ] {
        assert_eq!(SymbolCounts::new(width, height).unwrap().codewords, codewords);
        assert_eq!(capacity(width, height, 0), Some(k), "{width} × {height}");
    }
    assert_eq!((symbol.width(), symbol.height()), (20, 28));
    let counts = SymbolCounts::new(20, 28).unwrap();
    assert_eq!(counts.data_modules, 322);
    assert_eq!(counts.reference_marks, 0);
    assert_eq!((counts.codewords, counts.remainder_bits), (40, 2));
    let split = symbol.block_split();
    assert_eq!(split.codewords(), 40);
    assert_eq!(split.block_count(), 1);
    assert_eq!(split.max_block_len(), 40);
    assert_eq!(split.parity_per_block(), 6);
    assert_eq!(split.capacity(), 34);
    assert_eq!(symbol.capacity(), 34);
    assert_eq!(symbol.codewords(), 40);

    // A.2.4
    assert_eq!(symbol.message(), MESSAGE);
    let parity: Vec<u8> =
        (34..40).map(|byte| symbol.stream()[split.stream_index(0, byte).unwrap()]).collect();
    assert_eq!(parity, PARITY);
    let mut stream = MESSAGE.to_vec();
    stream.extend_from_slice(&PARITY);
    assert_eq!(symbol.stream(), stream);

    // A.2.5
    let format = symbol.format();
    assert_eq!((format.width_code(), format.height_code()), (1, 3));
    assert_eq!((format.level(), format.colour_profile(), format.chroma_cell()), (0, 0, 0));
    assert_eq!(format.data(), 0x000_8060);
    assert_eq!(format_parity(format.data()), 0x4_66EB);
    assert_eq!(format.codeword(), CODEWORD);
    assert_eq!(format.encode(), COPIES[0]);
    assert_eq!(format.encode_copies(), COPIES);
    assert_eq!(format.echo().byte(), 0x00);

    // A.2.6: the two remainder modules and their whitening bits.
    let layout = Layout::new(20, 28).unwrap();
    let placement: Vec<(u32, u32)> = layout.placement().collect();
    assert_eq!(placement.len(), 322);
    assert_eq!(placement[320..], [(18, 6), (19, 6)]);
    let whitening: Vec<bool> = Whitening::new().take(322).collect();
    assert!(whitening[320]);
    assert!(!whitening[321]);
    assert_eq!(symbol.grid().get(18, 6), Some(true));
    assert_eq!(symbol.grid().get(19, 6), Some(false));

    // A.2.7
    assert_eq!(symbol.grid(), &matrix_from_hex());
}

/// The last paragraph of A.2.7: reading the matrix back gives copy A equal to `F_A` and copy B
/// equal to `F_B`, a stream with zero syndromes, a matching CRC-32C and format echo byte, and the
/// 40 content bytes.
#[test]
fn a2_decoding_the_matrix_gives_the_content_back() {
    let grid = matrix_from_hex();
    assert_eq!(read_format_copies(&grid).unwrap(), COPIES);
    let layout = Layout::new(20, 28).unwrap();
    let stream = layout.read_stream(&grid).unwrap();
    assert!(nmtcode_ecc::rs::syndromes(&stream, 6).unwrap().iter().all(|&s| s == 0));
    assert_eq!(stream[..29], CONTAINER);

    let decoded = decode(&grid, &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.outcome, Outcome::Presented);
    assert_eq!(decoded.notice, None);
    assert_eq!(decoded.corrected, 0);
    assert_eq!(decoded.codec, 3);
    assert_eq!(decoded.dictionary, 0);
    assert_eq!(decoded.record_form, RecordForm::Single(ContentType::URL));
    assert_eq!((decoded.format.width(), decoded.format.height()), (20, 28));
    assert_eq!(decoded.format.codeword(), CODEWORD);
    assert_eq!(decoded.records.len(), 1);
    let record = &decoded.records[0];
    assert_eq!(record.content_type, ContentType::URL);
    assert_eq!(record.present_as, PresentAs::Url);
    assert_eq!(record.notice, None);
    assert_eq!(record.value, URL.as_bytes());
    assert_eq!(record.text(), Some(URL));
}

/// A.2.3's remarks: 28 × 20 holds the container in the same area, and the smallest square that
/// holds it is 24 × 24.
#[test]
fn a2_3_the_other_sizes_hold_the_container() {
    for (size, dimensions) in [
        (SizeRule::Exact { width: 28, height: 20 }, (28, 20)),
        (SizeRule::SmallestSquare, (24, 24)),
    ] {
        let options = EncodeOptions { level: Some(0), size, ..EncodeOptions::default() };
        let symbol = encode_url(URL, &options).unwrap();
        assert_eq!((symbol.width(), symbol.height()), dimensions);
        assert_eq!(symbol.container(), CONTAINER);
        let decoded = decode(symbol.grid(), &DecodeOptions::default()).unwrap();
        assert_eq!(decoded.records[0].value, URL.as_bytes());
    }
}

/// A.3: the construction of the nested-symbol vector. The reader rule itself (5.11 step 6,
/// `E_NESTED_SYMBOL`) acts on detected symbols; the command-line tests read this image.
#[test]
fn a3_nested_symbol_construction() {
    let outer_options =
        EncodeOptions { size: SizeRule::Exact { width: 96, height: 96 }, ..level0() };
    let outer = encode_text("outer content", &outer_options).unwrap();
    let inner = encode_text("inner content", &level0()).unwrap();
    assert_eq!(
        outer.container(),
        [
            0x03, 0x00, 0x0D, 0x00, 0x0D, 0x01, 0xA0, 0x2E, 0x26, 0x0D, 0x71, 0x12, 0xC9, 0xD4,
            0xBE, 0xC9, 0xF1, 0xAC, 0x44, 0xFC
        ]
    );
    assert_eq!(
        inner.container(),
        [
            0x03, 0x00, 0x0D, 0x00, 0x0D, 0x01, 0x80, 0xFE, 0xD4, 0x15, 0x91, 0x12, 0xC9, 0xD4,
            0xBE, 0xC9, 0x9D, 0x0B, 0x07, 0xFC
        ]
    );
    assert_eq!((inner.width(), inner.height()), (20, 24));

    let mut grid = outer.grid().clone();
    for y in 36..64 {
        for x in 36..60 {
            grid.set(x, y, false);
        }
    }
    for y in 0..24 {
        for x in 0..20 {
            grid.set(38 + x, 38 + y, inner.grid().get(x, y).unwrap());
        }
    }
    let changed = (0..96)
        .flat_map(|y| (0..96).map(move |x| (x, y)))
        .filter(|&(x, y)| grid.get(x, y) != outer.grid().get(x, y))
        .count();
    assert_eq!(changed, 349);
    let layout = Layout::new(96, 96).unwrap();
    assert!(layout.reference_mark_centres().any(|centre| centre == (48, 48)));
    assert_eq!(layout.module_class(48, 48), Some(ModuleClass::ReferenceMark));

    let alone = decode(inner.grid(), &DecodeOptions::default()).unwrap();
    assert_eq!(alone.records[0].text(), Some("inner content"));
    assert_eq!(decode(&grid, &DecodeOptions::default()).unwrap_err().error(), SpecError::EccFailed);
}

/// A.4: a 20 × 20 symbol at level 0 drawn from `message`.
fn a4_symbol(message: &[u8]) -> ModuleGrid {
    let split = nmtcode_ecc::split(20, 0).unwrap();
    assert_eq!((split.block_count(), split.parity_per_block(), split.capacity()), (1, 4, 16));
    let format = FormatWord::new(SymbolClass::Static, 20, 20, 0, 0, 0).unwrap();
    assert_eq!(format.data(), 0x000_8020);
    assert_eq!(format.codeword(), 0x0004_0107_AFEF);
    assert_eq!(format.encode_copies(), [0x51F7_6849_0045, 0x3FCE_35B9_B0C9]);
    let stream = nmtcode_ecc::encode_stream(&split, message).unwrap();
    Layout::new(20, 20).unwrap().draw_copies(format.encode_copies(), &stream).unwrap()
}

/// The parity bytes of the single block of an A.4 message.
fn a4_parity(message: &[u8]) -> Vec<u8> {
    let split = nmtcode_ecc::split(20, 0).unwrap();
    nmtcode_ecc::encode_stream(&split, message).unwrap()[16..].to_vec()
}

/// A.4 a: a body length of 2^32 − 1.
#[test]
fn a4_body_length_at_the_limit() {
    let mut message = vec![0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
    pad_message(&mut message, 16).unwrap();
    assert_eq!(
        message,
        [
            0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC,
            0x11, 0xEC
        ]
    );
    assert_eq!(a4_parity(&message), [0x5C, 0x90, 0xA8, 0x87]);
    // The sum of A.4 a computed in 32 bits wraps to 10.
    assert_eq!(2u32.wrapping_add(5).wrapping_add(u32::MAX).wrapping_add(4), 10);
    let error = decode(&a4_symbol(&message), &DecodeOptions::default()).unwrap_err();
    assert_eq!(error.error(), SpecError::LengthField);
    assert_eq!(error.outcome(), Outcome::Damaged);
}

/// A.4 b: a record value length of 2^32 − 1 behind a matching CRC-32C.
#[test]
fn a4_record_length_at_the_limit() {
    let body = [0x20, 0x00, 0x08, 0x01, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x41];
    assert_eq!(crc32c(&body), 0x7400_7865);
    let mut message = body.to_vec();
    message.extend_from_slice(&0x7400_7865u32.to_be_bytes());
    pad_message(&mut message, 16).unwrap();
    assert_eq!(
        message,
        [
            0x20, 0x00, 0x08, 0x01, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x41, 0x74, 0x00, 0x78,
            0x65, 0xEC
        ]
    );
    assert_eq!(a4_parity(&message), [0x05, 0x7D, 0x80, 0x1B]);
    // The end offset of A.4 b computed in 32 bits wraps to 5.
    assert_eq!(6u32.wrapping_add(u32::MAX), 5);
    let error = decode(&a4_symbol(&message), &DecodeOptions::default()).unwrap_err();
    assert_eq!(error.error(), SpecError::RecordList);
    assert_eq!(error.outcome(), Outcome::Malformed);
}
