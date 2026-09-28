//! Geometry, finder patterns, reference marks, module placement and whitening of NMT Code symbols.
//!
//! This crate implements chapter 5 of the NMT Code specification (version 0.1, draft) and
//! section 2.6, which says which bit of the format word goes on which module. It works on the
//! grid of modules of one symbol, [`ModuleGrid`], with the quiet zone excluded.
//!
//! - [`Layout`] holds everything that depends only on the symbol size: the class of every module
//!   (5.2 to 5.7), the counts D, N and R (5.7) and the placement order (5.8), which
//!   [`Layout::placement`] yields as a [`Placement`] iterator.
//!   [`Layout::draw`] turns an already-masked format codeword and the codeword stream into a
//!   grid; [`Layout::read_stream`] and [`read_format_copies`] read them back.
//! - [`SymbolCounts`] gives D, N, R and the number of reference marks from the formula of 5.7,
//!   without building a [`Layout`].
//! - [`Corner`] is the finder pattern of each corner (5.3).
//! - [`Whitening`] is the whitening sequence (5.10).
//!
//! The quiet zone (5.2) is not part of the grid: a renderer surrounds the grid with at least
//! [`QUIET_ZONE_MIN`] light modules on every side.
//!
//! # Format codeword bits
//!
//! A format codeword is the low [`FORMAT_BITS`] bits of a `u64`. Bit index `i` of 2.4.3, the
//! coefficient of x^(46 − i), is integer bit `46 − i`, so index 0 is the most significant of the
//! 47 bits. Module A\[i\] of copy A and module B\[i\] of copy B carry bit index `i` (2.6), dark
//! for 1.
//!
//! # Example
//!
//! ```
//! use nmtcode_symbol::{Layout, read_format_copies};
//!
//! let layout = Layout::new(24, 24)?;
//! assert_eq!(layout.codeword_count(), 42);
//! let stream = vec![0xA5; layout.codeword_count()];
//! let grid = layout.draw(0x51FB_6B49_7725, &stream)?;
//! assert_eq!(layout.read_stream(&grid)?, stream);
//! assert_eq!(read_format_copies(&grid)?, [0x51FB_6B49_7725; 2]);
//! # Ok::<(), nmtcode_symbol::SymbolError>(())
//! ```

#![no_std]

extern crate alloc;

mod finder;
mod format;
mod layout;
mod marks;
mod whitening;

pub use finder::Corner;
pub use format::{
    FORMAT_BITS, FORMAT_COPY_A, FORMAT_MIN_SIDE, FormatCopy, format_module, format_positions,
    read_format_copies, write_format_copies,
};
pub use layout::{Layout, ModuleClass, Placement, SymbolCounts};
pub use marks::reference_mark_lines;
pub use nmtcode_core::ModuleGrid;
pub use whitening::{WHITENING_SEED, Whitening};

/// Smallest quiet zone, in modules, on each of the four sides (5.2). All light.
pub const QUIET_ZONE_MIN: u32 = 2;
/// Default quiet zone, in modules, on each of the four sides (5.2).
pub const QUIET_ZONE_DEFAULT: u32 = 2;
/// Side of a finder pattern, in modules (5.3).
pub const FINDER_SIZE: u32 = 5;
/// Width of the light separator on the two inner sides of each finder, in modules (5.4).
///
/// The format word positions of 5.5 and the count D of 5.7 assume this value.
pub const SEPARATOR_WIDTH: u32 = 1;
/// Side of a reference mark, in modules (5.6).
pub const REFERENCE_MARK_SIZE: u32 = 3;

/// Why a function of this crate refused its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolError {
    /// The width or the height is not a multiple of 4 from 20 to 4108 (5.2).
    InvalidSize {
        /// The width that was asked for.
        width: u32,
        /// The height that was asked for.
        height: u32,
    },
    /// The symbol has more modules than this platform can address or allocate.
    TooLarge,
    /// The codeword stream does not have exactly N bytes (5.7, 5.9).
    StreamLength {
        /// N, the codeword count of the layout.
        expected: usize,
        /// The length of the stream that was given.
        actual: usize,
    },
    /// The format codeword has a bit set above its 47 bits (2.4.1).
    FormatCodewordTooWide,
    /// The grid does not have the layout's width and height.
    GridSizeMismatch {
        /// The layout's width.
        expected_width: u32,
        /// The layout's height.
        expected_height: u32,
        /// The grid's width.
        width: u32,
        /// The grid's height.
        height: u32,
    },
    /// The grid is narrower or lower than [`FORMAT_MIN_SIDE`] modules, so a format copy does
    /// not fit inside it (5.5).
    GridTooSmall {
        /// The grid's width.
        width: u32,
        /// The grid's height.
        height: u32,
    },
}

impl core::fmt::Display for SymbolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::InvalidSize { width, height } => write!(
                f,
                "{width} x {height} is not a symbol size: each side must be a multiple of 4 \
                 from 20 to 4108"
            ),
            Self::TooLarge => f.write_str("the symbol is too large for this platform"),
            Self::StreamLength { expected, actual } => {
                write!(f, "the codeword stream has {actual} bytes; this size takes {expected}")
            }
            Self::FormatCodewordTooWide => {
                f.write_str("the format codeword has a bit set above its 47 bits")
            }
            Self::GridSizeMismatch { expected_width, expected_height, width, height } => write!(
                f,
                "the grid is {width} x {height} modules; the layout is \
                 {expected_width} x {expected_height}"
            ),
            Self::GridTooSmall { width, height } => write!(
                f,
                "the grid is {width} x {height} modules; a format copy needs at least \
                 {FORMAT_MIN_SIDE} x {FORMAT_MIN_SIDE}"
            ),
        }
    }
}

impl core::error::Error for SymbolError {}
