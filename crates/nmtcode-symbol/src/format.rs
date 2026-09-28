//! The two format word copies and which bit goes on which module (5.5, 2.6).

use nmtcode_core::ModuleGrid;

use crate::SymbolError;

/// Number of bits of the format word, and of modules in each of its two copies (2.4.1, 5.5).
pub const FORMAT_BITS: usize = 47;

/// Smallest width and height, in modules, at which both format copies lie inside a grid.
///
/// Copy A reaches x = 9 and y = 9 (5.5). Every valid symbol size is larger; readers may read
/// the copies from any grid at least this large.
pub const FORMAT_MIN_SIDE: u32 = 10;

/// The low 47 bits of a `u64`: every bit a format codeword may use.
const CODEWORD_BITS: u64 = (1 << FORMAT_BITS) - 1;

/// One of the two copies of the format word (5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatCopy {
    /// Copy A, next to the top-left finder.
    A,
    /// Copy B: copy A turned by 180° about the symbol's centre, next to the bottom-right finder.
    B,
}

impl FormatCopy {
    /// Both copies, A first.
    pub const ALL: [Self; 2] = [Self::A, Self::B];
}

/// The modules of copy A in bit order (5.5): `FORMAT_COPY_A[i]` is A\[i\] = (x, y).
///
/// A\[0..22\] are the rows y = 0 to 5 of x = 6 to 9 without (9, 5); A\[23..46\] are the columns
/// x = 0 to 5 of y = 6 to 9.
pub const FORMAT_COPY_A: [(u32, u32); FORMAT_BITS] = copy_a();

const fn copy_a() -> [(u32, u32); FORMAT_BITS] {
    let mut out = [(0, 0); FORMAT_BITS];
    let mut i = 0;
    let mut y = 0;
    while y <= 5 {
        let mut x = 6;
        while x <= 9 {
            if !(x == 9 && y == 5) {
                out[i] = (x, y);
                i += 1;
            }
            x += 1;
        }
        y += 1;
    }
    let mut x = 0;
    while x <= 5 {
        let mut y = 6;
        while y <= 9 {
            out[i] = (x, y);
            i += 1;
            y += 1;
        }
        x += 1;
    }
    out
}

/// The module that carries bit index `index` of copy `copy` in a `width` × `height` grid
/// (5.5, 2.6). B\[i\] = (W − 1 − x, H − 1 − y) where A\[i\] = (x, y).
///
/// Returns `None` when `index` is 47 or more, or when a side is below [`FORMAT_MIN_SIDE`].
pub fn format_module(
    copy: FormatCopy,
    index: usize,
    width: u32,
    height: u32,
) -> Option<(u32, u32)> {
    let &(x, y) = FORMAT_COPY_A.get(index)?;
    if width < FORMAT_MIN_SIDE || height < FORMAT_MIN_SIDE {
        return None;
    }
    Some(match copy {
        FormatCopy::A => (x, y),
        FormatCopy::B => (width - 1 - x, height - 1 - y),
    })
}

/// The modules of both copies in bit order, copy A first, in a `width` × `height` grid.
///
/// # Errors
///
/// [`SymbolError::GridTooSmall`] when a side is below [`FORMAT_MIN_SIDE`].
pub fn format_positions(
    width: u32,
    height: u32,
) -> Result<[[(u32, u32); FORMAT_BITS]; 2], SymbolError> {
    if width < FORMAT_MIN_SIDE || height < FORMAT_MIN_SIDE {
        return Err(SymbolError::GridTooSmall { width, height });
    }
    let b = FORMAT_COPY_A.map(|(x, y)| (width - 1 - x, height - 1 - y));
    Ok([FORMAT_COPY_A, b])
}

/// Reads both format copies from `grid`, copy A first (2.6, 2.7 step 1).
///
/// Each value is the raw 47-bit word as read, still masked: module A\[i\] (or B\[i\]) becomes
/// integer bit `46 − i`, 1 for dark. The positions depend only on the grid's width and height.
///
/// # Errors
///
/// [`SymbolError::GridTooSmall`] when a side is below [`FORMAT_MIN_SIDE`].
pub fn read_format_copies(grid: &ModuleGrid) -> Result<[u64; 2], SymbolError> {
    let positions = format_positions(grid.width(), grid.height())?;
    Ok(positions.map(|copy| {
        copy.iter().fold(0, |word, &(x, y)| (word << 1) | u64::from(grid.get(x, y) == Some(true)))
    }))
}

/// Writes the two sent format words on their copies of `grid` (2.6): `copies[0]` = `F_A` on
/// copy A and `copies[1]` = `F_B` on copy B, as `nmtcode_core::FormatWord::encode_copies`
/// returns them.
///
/// Integer bit `46 − i` of `F_A` goes on module A\[i\] and that of `F_B` on module B\[i\], dark
/// for 1. Each copy has its own mask (2.5), so the two words differ; this function takes both so
/// that one word is never written into both copies. Other modules are left as they are.
///
/// # Errors
///
/// [`SymbolError::FormatCodewordTooWide`] when a word has a bit set above bit 46, and
/// [`SymbolError::GridTooSmall`] when a side of `grid` is below [`FORMAT_MIN_SIDE`].
pub fn write_format_copies(grid: &mut ModuleGrid, copies: [u64; 2]) -> Result<(), SymbolError> {
    for word in copies {
        check_codeword(word)?;
    }
    let positions = format_positions(grid.width(), grid.height())?;
    for (copy, word) in positions.into_iter().zip(copies) {
        for (i, (x, y)) in copy.into_iter().enumerate() {
            grid.set(x, y, word >> (FORMAT_BITS - 1 - i) & 1 == 1);
        }
    }
    Ok(())
}

/// [`SymbolError::FormatCodewordTooWide`] when `codeword` has a bit set above bit 46.
pub(crate) fn check_codeword(codeword: u64) -> Result<(), SymbolError> {
    if codeword & !CODEWORD_BITS == 0 { Ok(()) } else { Err(SymbolError::FormatCodewordTooWide) }
}

/// True when (`x`, `y`) is a module of copy A (5.5).
pub(crate) fn is_copy_a(x: u32, y: u32) -> bool {
    (y <= 5 && (6..=9).contains(&x) && !(x == 9 && y == 5)) || (x <= 5 && (6..=9).contains(&y))
}
