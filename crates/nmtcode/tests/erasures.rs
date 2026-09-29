//! `decode_with_erasures`: uncertain modules become erased format bits and erased codewords
//! (chapter 5, 5.9; chapter 4, 4.9; chapter 2, 2.7), and a failed reading falls back to the
//! plain decoder.

mod common;

use common::{codeword_modules, flip_codeword};
use nmtcode::{
    DecodeOptions, EncodeOptions, SizeRule, SpecError, Symbol, decode, decode_with_erasures,
    encode_file,
};
use nmtcode_symbol::{FormatCopy, Layout, format_module};

fn symbol(level: u8, width: u32, height: u32) -> Symbol {
    let len = u32::try_from(nmtcode::capacity(width, height, level).unwrap()).unwrap() - 24;
    let bytes: Vec<u8> = (0..len).map(|i| u8::try_from(i * 37 % 251).unwrap()).collect();
    let options = EncodeOptions {
        level: Some(level),
        size: SizeRule::Exact { width, height },
        ..EncodeOptions::default()
    };
    encode_file("data.bin", &bytes, &options).unwrap()
}

/// The modules of the codewords `indices`, all eight of each.
fn modules_of(layout: &Layout, indices: &[usize]) -> Vec<(u32, u32)> {
    let modules = codeword_modules(layout);
    indices.iter().flat_map(|&i| modules[i]).collect()
}

#[test]
fn without_uncertain_modules_it_is_decode() {
    let s = symbol(0, 48, 48);
    let options = DecodeOptions::default();
    assert_eq!(decode_with_erasures(s.grid(), &[], &options), decode(s.grid(), &options));
}

#[test]
fn erasures_correct_twice_as_many_codewords() {
    // Level 1 at 48 × 48: P parity bytes per block. Damage more codewords of every block than
    // P / 2 errors allow, but no more than P erasures: only the erasures make it decodable.
    let s = symbol(1, 48, 48);
    let split = s.block_split();
    let parity = split.parity_per_block();
    let layout = Layout::new(48, 48).unwrap();
    let modules = codeword_modules(&layout);
    let mut grid = s.grid().clone();
    let mut damaged = Vec::new();
    for block in split.blocks() {
        for byte in 0..parity * 3 / 4 {
            let index = split.stream_index(block.index, byte).unwrap();
            flip_codeword(&mut grid, &modules, index, 0xA5);
            damaged.push(index);
        }
    }
    let options = DecodeOptions::default();
    assert_eq!(decode(&grid, &options).unwrap_err().error(), SpecError::EccFailed);
    // Marking one module of each damaged codeword erases the whole codeword.
    let uncertain: Vec<(u32, u32)> = damaged.iter().map(|&i| modules[i][0]).collect();
    let decoded = decode_with_erasures(&grid, &uncertain, &options).unwrap();
    assert_eq!(decoded.records, decode(s.grid(), &options).unwrap().records);
    assert!(decoded.corrected >= damaged.len());
}

#[test]
fn wrong_marks_fall_back_to_the_plain_decoder() {
    // Marks on correct codewords beyond what any block can take: the reading with erasures
    // fails (every block has more marks than parity, the first P are used and the first half
    // tried), and the plain decoder still reads the symbol.
    let s = symbol(0, 48, 48);
    let layout = Layout::new(48, 48).unwrap();
    let all: Vec<usize> = (0..layout.codeword_count()).collect();
    let uncertain = modules_of(&layout, &all);
    let options = DecodeOptions::default();
    let decoded = decode_with_erasures(s.grid(), &uncertain, &options).unwrap();
    assert_eq!(decoded, decode(s.grid(), &options).unwrap());
}

#[test]
fn a_damaged_symbol_is_not_presented_with_or_without_erasures() {
    let s = symbol(0, 48, 48);
    let layout = Layout::new(48, 48).unwrap();
    let modules = codeword_modules(&layout);
    let mut grid = s.grid().clone();
    // Every other codeword damaged: beyond erasures and errors together.
    for index in (0..layout.codeword_count()).step_by(2) {
        flip_codeword(&mut grid, &modules, index, 0xFF);
    }
    let uncertain =
        modules_of(&layout, &(0..layout.codeword_count()).step_by(4).collect::<Vec<_>>());
    let options = DecodeOptions::default();
    let result = decode_with_erasures(&grid, &uncertain, &options);
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn format_bits_can_be_erased() {
    // Copy B covered, copy A with 4 wrong bits: beyond 3 errors, within 2e + s ≤ 7 once those
    // four are marked (chapter 2, 2.7).
    let original = symbol(0, 24, 24);
    let mut grid = original.grid().clone();
    let (width, height) = (grid.width(), grid.height());
    for bit in 0..47 {
        let (x, y) = format_module(FormatCopy::B, bit, width, height).unwrap();
        grid.set(x, y, false);
    }
    let wrong: Vec<(u32, u32)> = [0usize, 9, 20, 40]
        .iter()
        .map(|&bit| format_module(FormatCopy::A, bit, width, height).unwrap())
        .collect();
    for &(x, y) in &wrong {
        grid.toggle(x, y);
    }
    let options = DecodeOptions::default();
    assert_eq!(decode(&grid, &options).unwrap_err().error(), SpecError::FormatUnreadable);
    let decoded = decode_with_erasures(&grid, &wrong, &options).unwrap();
    assert_eq!(decoded.records, decode(original.grid(), &options).unwrap().records);
}

#[test]
fn a_size_above_the_limit_is_refused_before_the_data() {
    let s = symbol(0, 48, 48);
    let options = DecodeOptions { max_area: 48 * 44, ..DecodeOptions::default() };
    let uncertain = [(20, 20)];
    let error = decode_with_erasures(s.grid(), &uncertain, &options).unwrap_err();
    assert_eq!(error.error(), SpecError::SizeLimit);
}
