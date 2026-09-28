//! Helpers shared by the integration tests of the `nmtcode` crate.

#![allow(dead_code)]

use nmtcode::{ModuleGrid, SymbolClass};
use nmtcode_core::{FormatWord, pad_message};
use nmtcode_symbol::Layout;

/// The modules of each codeword c[i], most significant bit first (5.9).
pub fn codeword_modules(layout: &Layout) -> Vec<[(u32, u32); 8]> {
    let placement: Vec<(u32, u32)> = layout.placement().collect();
    let (codewords, _remainder) = placement.as_chunks::<8>();
    codewords.to_vec()
}

/// Flips the modules of codeword `index` selected by the bits of `mask` (bit 7 = the codeword's
/// most significant bit). `mask` must not be 0.
pub fn flip_codeword(grid: &mut ModuleGrid, modules: &[[(u32, u32); 8]], index: usize, mask: u8) {
    assert_ne!(mask, 0);
    for (bit, &(x, y)) in modules[index].iter().enumerate() {
        if mask >> (7 - bit) & 1 == 1 {
            assert!(grid.toggle(x, y));
        }
    }
}

/// A symbol of the given format fields that carries `container` (any bytes, which are padded to
/// the capacity). The format word is written without the checks of the encoder, so tests can
/// build symbols the encoder never makes.
pub fn symbol_with_container(
    container: &[u8],
    width: u32,
    height: u32,
    level: u8,
    colour_profile: u8,
    class: SymbolClass,
) -> ModuleGrid {
    let layout = Layout::new(width, height).unwrap();
    let split = nmtcode_ecc::split(layout.codeword_count(), level).unwrap();
    let mut message = container.to_vec();
    pad_message(&mut message, split.capacity()).unwrap();
    let stream = nmtcode_ecc::encode_stream(&split, &message).unwrap();
    let word = FormatWord::new(class, width, height, level, colour_profile, 0).unwrap();
    layout.draw(word.encode(), &stream).unwrap()
}

/// A container of any lead byte and body, with a correct body length and CRC-32C (3.2.1, 3.7).
pub fn sealed_container(lead: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![lead];
    nmtcode_core::write_leb128(&mut out, u32::try_from(body.len()).unwrap());
    out.extend_from_slice(body);
    let crc = nmtcode_core::crc32c(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}
