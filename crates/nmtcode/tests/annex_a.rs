//! Annex A (A.2): the complete static symbol, from content to module matrix and back.

// The matrix rows are written exactly as A.2.7 prints them, without digit separators.
#![allow(clippy::unreadable_literal)]

use nmtcode::{
    CodecOptions, ContentType, DecodeOptions, EncodeOptions, ModuleGrid, Outcome, PresentAs,
    RecordForm, SizeConstraints, SizeRule, capacity, decode, encode_url,
};
use nmtcode_core::{Record, RecordContent, StaticFields, crc32c, format_parity};
use nmtcode_symbol::{Layout, SymbolCounts, Whitening, read_format_copies};

const URL: &str = "https://github.com/needmoretruth/nmtcode";

/// A.2.2 (example f of 3.10).
const CONTAINER: [u8; 28] = [
    0x03, 0x16, 0x00, 0x28, 0x02, 0xDE, 0xA7, 0x40, 0xBA, 0x96, 0xDC, 0xEE, 0xAA, 0xEE, 0x6B, 0xEF,
    0xDF, 0x35, 0x76, 0x47, 0xAE, 0xBA, 0xF7, 0x91, 0x33, 0x56, 0xF1, 0x6E,
];

/// A.2.4: the container and 6 padding bytes.
const MESSAGE: [u8; 34] = [
    0x03, 0x16, 0x00, 0x28, 0x02, 0xDE, 0xA7, 0x40, 0xBA, 0x96, 0xDC, 0xEE, 0xAA, 0xEE, 0x6B, 0xEF,
    0xDF, 0x35, 0x76, 0x47, 0xAE, 0xBA, 0xF7, 0x91, 0x33, 0x56, 0xF1, 0x6E, 0xEC, 0x11, 0xEC, 0x11,
    0xEC, 0x11,
];

/// A.2.4: parity p[0 … 7] of the single block.
const PARITY: [u8; 8] = [0x5F, 0xBF, 0xC3, 0xA7, 0xFC, 0x87, 0x2F, 0xA6];

/// A.2.7: row y as 24 bits, x = 0 the most significant.
const MATRIX_HEX: [u32; 24] = [
    0xFAA49F, 0x98D899, 0x9BF7DF, 0xF98619, 0xFB4FDF, 0x01F7C0, 0x43681A, 0xB4CBA4, 0x3BF673,
    0x7726D8, 0x0D0065, 0x8BBA54, 0x9FB0C6, 0x392308, 0xBAA2AE, 0x9275DC, 0x43FB2D, 0x671182,
    0x034D80, 0xF8FADF, 0xFB9D93, 0xF9B7D5, 0xF99F19, 0xFB8D5F,
];

/// A.2.7, the drawn form.
const MATRIX_ROWS: [&str; 24] = [
    "#####.#.#.#..#..#..#####",
    "#..##...##.##...#..##..#",
    "#..##.######.#####.#####",
    "#####..##....##....##..#",
    "#####.##.#..######.#####",
    ".......#####.#####......",
    ".#....##.##.#......##.#.",
    "#.##.#..##..#.###.#..#..",
    "..###.######.##..###..##",
    ".###.###..#..##.##.##...",
    "....##.#.........##..#.#",
    "#...#.###.###.#..#.#.#..",
    "#..######.##....##...##.",
    "..###..#..#...##....#...",
    "#.###.#.#.#...#.#.#.###.",
    "#..#..#..###.#.###.###..",
    ".#....#######.##..#.##.#",
    ".##..###...#...##.....#.",
    "......##.#..##.##.......",
    "#####...#####.#.##.#####",
    "#####.###..###.##..#..##",
    "#####..##.##.#####.#.#.#",
    "#####..##..#####...##..#",
    "#####.###...##.#.#.#####",
];

fn matrix_from_hex() -> ModuleGrid {
    let mut grid = ModuleGrid::new(24, 24).unwrap();
    for (y, row) in (0u32..).zip(MATRIX_HEX) {
        for x in 0..24 {
            grid.set(x, y, (row >> (23 - x)) & 1 == 1);
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
    assert_eq!(size_of(0), Some(47));
    assert_eq!(size_of(1), None, "codec 1 does not apply to lower-case letters");
    assert_eq!(size_of(2), None, "codec 2 does not apply to lower-case letters");
    assert_eq!(size_of(3), Some(28));
    assert_eq!(size_of(4), Some(44));
    // Informative in A.2.1 (one brotli implementation at quality 11); this one agrees.
    assert_eq!(size_of(5), Some(49));

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
    assert_eq!(symbol.container_len(), 28);
    assert_eq!(crc32c(&CONTAINER[..24]), 0x3356_F16E);

    // A.2.3
    assert_eq!((symbol.width(), symbol.height()), (24, 24));
    assert_eq!(capacity(20, 20, 0), Some(16));
    let counts = SymbolCounts::new(24, 24).unwrap();
    assert_eq!(counts.data_modules, 338);
    assert_eq!(counts.reference_marks, 0);
    assert_eq!((counts.codewords, counts.remainder_bits), (42, 2));
    let split = symbol.block_split();
    assert_eq!(split.codewords(), 42);
    assert_eq!(split.block_count(), 1);
    assert_eq!(split.max_block_len(), 42);
    assert_eq!(split.parity_per_block(), 8);
    assert_eq!(split.capacity(), 34);
    assert_eq!(symbol.capacity(), 34);
    assert_eq!(symbol.codewords(), 42);
    assert_eq!(capacity(20, 28, 0), Some(34));
    assert_eq!(capacity(28, 20, 0), Some(34));

    // A.2.4
    assert_eq!(symbol.message(), MESSAGE);
    let parity: Vec<u8> =
        (34..42).map(|byte| symbol.stream()[split.stream_index(0, byte).unwrap()]).collect();
    assert_eq!(parity, PARITY);
    let mut stream = MESSAGE.to_vec();
    stream.extend_from_slice(&PARITY);
    assert_eq!(symbol.stream(), stream);

    // A.2.5
    let format = symbol.format();
    assert_eq!((format.width_code(), format.height_code()), (2, 2));
    assert_eq!((format.level(), format.colour_profile(), format.chroma_cell()), (0, 0, 0));
    assert_eq!(format.data(), 0x001_0040);
    assert_eq!(format_parity(format.data()), 0x7_D88F);
    assert_eq!(format.codeword(), 0x0008_0207_D88F);
    assert_eq!(format.encode(), 0x51FB_6B49_7725);

    // A.2.6: the two remainder modules and their whitening bits.
    let layout = Layout::new(24, 24).unwrap();
    let placement: Vec<(u32, u32)> = layout.placement().collect();
    assert_eq!(placement.len(), 338);
    assert_eq!(placement[336], (22, 6));
    assert_eq!(placement[337], (23, 6));
    let whitening: Vec<bool> = Whitening::new().take(338).collect();
    assert!(whitening[336]);
    assert!(!whitening[337]);
    assert_eq!(symbol.grid().get(22, 6), Some(true));
    assert_eq!(symbol.grid().get(23, 6), Some(false));

    // A.2.7
    assert_eq!(symbol.grid(), &matrix_from_hex());
}

/// The last paragraph of A.2.7: reading the matrix back gives both format copies equal to F, a
/// stream with zero syndromes, a matching CRC-32C and the 40 content bytes.
#[test]
fn a2_decoding_the_matrix_gives_the_content_back() {
    let grid = matrix_from_hex();
    assert_eq!(read_format_copies(&grid).unwrap(), [0x51FB_6B49_7725; 2]);
    let layout = Layout::new(24, 24).unwrap();
    let stream = layout.read_stream(&grid).unwrap();
    assert!(nmtcode_ecc::rs::syndromes(&stream, 8).unwrap().iter().all(|&s| s == 0));

    let decoded = decode(&grid, &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.outcome, Outcome::Presented);
    assert_eq!(decoded.notice, None);
    assert_eq!(decoded.corrected, 0);
    assert_eq!(decoded.codec, 3);
    assert_eq!(decoded.dictionary, 0);
    assert_eq!(decoded.record_form, RecordForm::Single(ContentType::URL));
    assert_eq!((decoded.format.width(), decoded.format.height()), (24, 24));
    assert_eq!(decoded.records.len(), 1);
    let record = &decoded.records[0];
    assert_eq!(record.content_type, ContentType::URL);
    assert_eq!(record.present_as, PresentAs::Url);
    assert_eq!(record.notice, None);
    assert_eq!(record.value, URL.as_bytes());
    assert_eq!(record.text(), Some(URL));
}

/// A.2.3's remark: the rectangles 20 × 28 and 28 × 20 hold the container with less area; the
/// constrained size rule of 1.5 finds them.
#[test]
fn a2_3_rectangles_hold_the_container() {
    let options = EncodeOptions {
        level: Some(0),
        size: SizeRule::Constrained(SizeConstraints::default()),
        ..EncodeOptions::default()
    };
    let symbol = encode_url(URL, &options).unwrap();
    assert_eq!(symbol.width() * symbol.height(), 560);
    assert_eq!(symbol.container(), CONTAINER);
    let decoded = decode(symbol.grid(), &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.records[0].value, URL.as_bytes());
}
