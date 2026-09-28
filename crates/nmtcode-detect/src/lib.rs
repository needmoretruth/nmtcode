//! Finds NMT Code symbols in a rendered image and samples their module grids.
//!
//! The reader of this crate handles images of symbols as a generator draws them: axis-aligned,
//! possibly rotated by a multiple of 90° or mirrored (specification 5.11), at a whole number of
//! pixels per module from 2 up, or rescaled by a non-integer factor with bilinear smoothing
//! (the tests cover 2.3 to 9.3 pixels per module), with extra light margin and a uniform change
//! of brightness, and reversed (light modules on dark). Camera photos (perspective, uneven
//! light, noise) are outside its scope.
//!
//! - [`detect_png`] decodes a PNG and returns a [`Detection`]: the module grid of every symbol
//!   found, and the number of nested symbols that are left out (5.11).
//! - [`detect`] does the same on an 8-bit luminance image.
//! - [`read_png`] and [`find_symbols`] return the grids alone.
//!
//! The result is the module grid only (quiet zone excluded, dark = `true`, in the symbol's own
//! orientation). Reading the format word and the data is the job of other crates. A bootstrap QR
//! Code beside a symbol (chapter 8) is not reported.
//!
//! How a symbol is found:
//!
//! 1. The image is split into dark and light at a global threshold (Otsu's method), and every
//!    4-connected dark region is measured.
//! 2. A region whose bounding box is about square is sampled as a 7 × 7 grid of cells: a light
//!    border, a dark ring and an inner 3 × 3 that must equal one of the four finder patterns of
//!    5.3 under one of the eight rotations and mirror images.
//! 3. Three or four finders whose kinds, orientation, size and alignment agree form a symbol.
//!    Their outer edges, measured to a fraction of a pixel, give the symbol's rectangle.
//! 4. The width and height in modules are the multiples of 4 from 20 to 4108 near the rectangle
//!    size divided by the finder module size; among them, the one whose module grid best matches
//!    the dark–light edges inside the rectangle is chosen.
//! 5. Every module is sampled at its centre and compared with a threshold taken from the darkest
//!    and lightest module centres around it. The four finders and their separators are checked
//!    in the sampled grid; at least three must be exact.
//! 6. A symbol whose rectangle lies inside another symbol's rectangle is nested (5.11): neither
//!    is returned, and [`Detection::nested`] counts both, so that a reader can report
//!    `E_NESTED_SYMBOL`.
//! 7. When the image holds no symbol, steps 1 to 6 run once more on the inverted image, so a
//!    reversed symbol is read as if it had been captured dark on light (1.4, 5.11).
//!
//! The width and height chosen in step 4 agree with the finder estimate within the tolerance
//! of 5.11, max(4, W / 10) modules. A reader that checks that the format word gives the grid's
//! width and height, as `nmtcode::decode` does, therefore applies the geometry check of 5.11.

mod decode;
mod finder;
mod luma;
mod num;
mod symbol;

use core::fmt;

pub use decode::decode_png;
pub use luma::LumaImage;
use nmtcode_core::ModuleGrid;

/// Largest image this crate accepts, in pixels (width × height). Larger PNGs are refused before
/// their pixels are allocated; larger [`LumaImage`]s give no symbols.
pub const MAX_PIXELS: u64 = 100_000_000;

/// Decodes the PNG in `bytes` and returns the module grid of every NMT Code symbol in it, in the
/// order the symbols were found. An image without a symbol gives an empty list.
///
/// Every PNG colour type and bit depth is accepted. Colour is converted to luminance
/// (ITU-R BT.601 weights) and transparency is composited over white.
///
/// # Errors
///
/// [`DetectError`] when the bytes are not a PNG, the PNG is damaged, or the image has more than
/// [`MAX_PIXELS`] pixels.
pub fn read_png(bytes: &[u8]) -> Result<Vec<ModuleGrid>, DetectError> {
    Ok(detect_png(bytes)?.symbols)
}

/// Returns the module grid of every NMT Code symbol in `image`, in the order the symbols were
/// found. An image without a symbol, and an image whose pixel buffer does not hold exactly
/// `width × height` pixels or that has more than [`MAX_PIXELS`] pixels, gives an empty list.
/// Nested symbols are left out; [`detect`] also counts them.
pub fn find_symbols(image: &LumaImage) -> Vec<ModuleGrid> {
    detect(image).symbols
}

/// Decodes the PNG in `bytes` and finds every NMT Code symbol in it, as [`read_png`], with the
/// count of nested symbols.
///
/// # Errors
///
/// As [`read_png`].
pub fn detect_png(bytes: &[u8]) -> Result<Detection, DetectError> {
    let image = decode_png(bytes)?;
    Ok(detect(&image))
}

/// Finds every NMT Code symbol in `image`, as [`find_symbols`], with the count of nested
/// symbols.
pub fn detect(image: &LumaImage) -> Detection {
    symbol::find(image)
}

/// The symbols found in one image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detection {
    /// The module grid of every symbol found that is not nested, in the order found (quiet
    /// zone excluded, dark = `true`, in the symbol's own orientation).
    pub symbols: Vec<ModuleGrid>,
    /// The number of symbols that lie inside another symbol found in the same image, together
    /// with the symbols around them (specification 5.11). None of them is in
    /// [`Detection::symbols`]: a reader presents neither and reports `E_NESTED_SYMBOL`.
    pub nested: usize,
    /// True when the symbols were found in the inverted image: light modules on dark (1.4).
    pub inverted: bool,
}

/// Why an image could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DetectError {
    /// The bytes do not start with the PNG signature.
    NotPng,
    /// The PNG is damaged or truncated; the message is the decoder's.
    Malformed(String),
    /// A well-formed PNG that this reader does not handle; the message says what.
    Unsupported(String),
    /// The image has more than [`MAX_PIXELS`] pixels.
    TooLarge {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPng => f.write_str("the data is not a PNG image"),
            Self::Malformed(message) => write!(f, "the PNG image is damaged: {message}"),
            Self::Unsupported(message) => write!(f, "the PNG image is not supported: {message}"),
            Self::TooLarge { width, height } => write!(
                f,
                "the image is {width} x {height} pixels, more than the {MAX_PIXELS} pixels accepted"
            ),
        }
    }
}

impl std::error::Error for DetectError {}
