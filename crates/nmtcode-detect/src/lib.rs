//! Finds NMT Code symbols in images, from renders to camera photos, and samples their module
//! grids.
//!
//! - [`read_image`] decodes a PNG or JPEG file into a [`LumaImage`] (BT.601 luma,
//!   specification 1.4), turned upright by the JPEG EXIF orientation tag.
//!   [`LumaImage::from_rgba`] converts a camera frame or canvas buffer.
//! - [`find`] finds every symbol in a luma image and returns a [`Scan`]: a [`Found`] for each
//!   symbol whose format word decoded and whose size was confirmed, with its module grid, the
//!   modules the detector could not tell with confidence and its corners in the image; and a
//!   [`Rejected`] for each symbol that failed a check before its data was read, with the error
//!   of chapter 9 (9.8).
//! - [`read_png`], [`find_symbols`], [`detect`] and [`detect_png`] are the grid-only forms.
//!
//! How a symbol is found:
//!
//! 1. A local threshold splits the image: per block of 8 × 8 pixels, the midpoint in linear
//!    light of the darkest and lightest values within about 20 pixels, where that range stands
//!    out from the noise. Optics blur in linear light, so the midpoint of stored (gamma-encoded)
//!    values would put every edge on the dark side. Every connected dark region is measured.
//! 2. A region whose outline is a convex quadrilateral with a light border around a dark ring,
//!    and two adjacent sides facing a quiet zone, is a finder candidate (5.3). Its four edges
//!    are measured to a fraction of a pixel and its inner 3 × 3 is scored against the four
//!    finders under the eight rotations and mirror images, with templates for blur up to one
//!    module.
//! 3. Finders whose outer sides lie on one line are joined; three finders in an L, and a fourth
//!    where the far sides meet, form a candidate. Of the eight ways to lay the symbol on them,
//!    the classification picks one (5.11 steps 1 and 2). A finder the threshold missed is
//!    searched on the grey image where the others put it; the corner of a missing finder is
//!    where the symbol's outer edges, traced along the quiet zone, meet.
//! 4. A homography from the finder corners gives the finder estimates of W and H; the format
//!    copies next to TL and BR are read at the sizes that fit the module edges best and decoded
//!    with the size tolerance of 5.11 (2.7).
//! 5. With W and H known, the homography is fitted to every finder corner; the reference
//!    marks refine it on sides of 48 or more (5.6); W and H are confirmed against W ± 4 and
//!    H ± 4 on the module edges between the finders (5.11 step 4).
//! 6. Every module centre is sampled; a decision-directed equaliser takes away the part of each
//!    sample that blur brings in from its eight neighbours.
//! 7. Symbols inside another found symbol are nested (5.11 step 6).
//!
//! When the image gives no symbol this way, the finder search runs again at thresholds nearer
//! the dark and the light level, then with pairs of finders (opposite corners, or one edge, the
//! fallback of 5.11 step 2) and with single finders whose two edges are traced; every candidate
//! still needs three finders. Then the inverted image is tried (1.4, 5.11 step 1), and a large
//! image at half size.
//!
//! The work on any image is bounded: the pixel count is capped by [`MAX_PIXELS`] before
//! anything is allocated, and the numbers of regions examined, finder candidates and symbol
//! candidates are capped.

// Geometry and image code uses the usual one-letter names: x, y, u, v, w, h for coordinates
// and sizes, a, b, c, d for points and coefficients.
#![allow(clippy::many_single_char_names)]

mod binarize;
mod blobs;
mod decode;
mod finder;
mod geom;
mod group;
#[cfg(feature = "jpeg")]
mod jpeg;
mod light;
mod luma;
mod marks;
mod num;
mod sample;
mod symbol;

use core::fmt;

pub use decode::decode_png;
pub use geom::Point;
#[cfg(feature = "jpeg")]
pub use jpeg::decode_jpeg;
pub use luma::LumaImage;
use nmtcode_core::{Error, MAX_SIDE, ModuleGrid};

/// Largest image this crate accepts, in pixels (width × height). Larger images are refused
/// before their pixels are allocated; larger [`LumaImage`]s give no symbols.
pub const MAX_PIXELS: u64 = 100_000_000;

/// The area of the largest symbol, 4108 × 4108 modules: the default of
/// [`DetectOptions::max_area`].
pub const MAX_AREA: u64 = MAX_SIDE as u64 * MAX_SIDE as u64;

/// Most finder candidates examined per image; beyond them the image is treated as clutter.
const MAX_BLOB_CHECKS: usize = 60_000;
/// Most symbol candidates read per image.
const MAX_CANDIDATES: usize = 128;
/// Smallest side of a finder region in pixels.
const MIN_FINDER_PX: usize = 5;
/// Images with a shorter side of at least this many pixels are also searched at half size when
/// nothing is found, for symbols with very large modules.
const PYRAMID_MIN_SIDE: u32 = 1200;

/// What [`find`] looks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectOptions {
    /// The largest area W × H, in modules, that the reader decodes (specification 2.7 step 5
    /// and 5.11 step 5). A symbol whose format word gives a larger area is reported as
    /// [`Rejected`] with `E_SIZE_LIMIT` before any data module is read. The default,
    /// [`MAX_AREA`], accepts every valid size.
    pub max_area: u64,
    /// Whether to try the inverted image when the image as captured gives no symbol
    /// (specification 1.4 and 5.11 step 1). Default true.
    pub try_inverted: bool,
}

impl Default for DetectOptions {
    fn default() -> Self {
        Self { max_area: MAX_AREA, try_inverted: true }
    }
}

/// A symbol found in an image: its format word decoded and its size was confirmed.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The module grid, quiet zone excluded, dark = `true`, in the symbol's own orientation
    /// (a mirror image is undone).
    pub grid: ModuleGrid,
    /// The format and data modules whose value the detector could not tell with confidence,
    /// least confident first, as (x, y) in the grid. A reader marks the codewords that hold
    /// them as erasures (specification 5.9, 4.9).
    pub uncertain: Vec<(u32, u32)>,
    /// The symbol's outer corners in image pixels, in the symbol's own corner order: top-left,
    /// top-right, bottom-right, bottom-left.
    pub corners: [Point; 4],
    /// True when the image shows the symbol mirrored.
    pub mirrored: bool,
    /// True when the symbol was found in the inverted image (light modules on dark).
    pub inverted: bool,
    /// The number of finders found, 3 or 4.
    pub finders: u8,
}

/// A symbol that failed a check before its data was read: the reader presents nothing from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rejected {
    /// The area of the symbol in image pixels (specification 5.11 step 6): the outer corners of
    /// its finders, the fourth completing a parallelogram when only three were found, in the
    /// order top-left, top-right, bottom-right, bottom-left.
    pub corners: [Point; 4],
    /// The reader error of chapter 9 (9.8): `E_FORMAT_UNREADABLE`, `E_FORMAT_CONFLICT`,
    /// `E_FORMAT_VERSION`, `E_SIZE_LIMIT` or `E_NESTED_SYMBOL`.
    pub error: Error,
}

/// Everything [`find`] found in one image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// The symbols to decode, in the order found.
    pub found: Vec<Found>,
    /// The symbols that failed a check, including both symbols of a nested pair.
    pub rejected: Vec<Rejected>,
    /// True when the symbols were found in the inverted image.
    pub inverted: bool,
}

impl Scan {
    /// The errors a reader reports for [`Scan::rejected`], in order, each symbol once.
    ///
    /// - A candidate with `E_FORMAT_UNREADABLE` has no format copy that decodes. In a photo
    ///   most such candidates are background patterns that look like finders, so they are
    ///   reported only when nothing else in the image was read or rejected, and then once.
    /// - `E_NESTED_SYMBOL` is reported once for a nested pair, after the others: neither symbol
    ///   of the pair is presented (specification 5.11 step 6).
    /// - Every other rejection (`E_FORMAT_CONFLICT`, `E_FORMAT_VERSION`, `E_SIZE_LIMIT`) comes
    ///   from a format word that decoded, so it is always reported.
    pub fn reported_errors(&self) -> Vec<Error> {
        let mut errors = Vec::new();
        let (mut nested, mut unreadable) = (false, false);
        for rejected in &self.rejected {
            match rejected.error {
                Error::NestedSymbol => nested = true,
                Error::FormatUnreadable => unreadable = true,
                error => errors.push(error),
            }
        }
        if unreadable && self.found.is_empty() && errors.is_empty() && !nested {
            errors.push(Error::FormatUnreadable);
        }
        if nested {
            errors.push(Error::NestedSymbol);
        }
        errors
    }
}

/// Decodes a PNG or JPEG image into luma, as [`decode_png`] or [`decode_jpeg`] by the file's
/// signature. A JPEG is turned upright by its EXIF orientation tag.
///
/// # Errors
///
/// [`DetectError::UnknownFormat`] for bytes that are neither, and the errors of the two
/// decoders.
pub fn read_image(bytes: &[u8]) -> Result<LumaImage, DetectError> {
    if bytes.starts_with(&decode::PNG_SIGNATURE) {
        return decode_png(bytes);
    }
    #[cfg(feature = "jpeg")]
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return decode_jpeg(bytes);
    }
    Err(DetectError::UnknownFormat)
}

/// Finds every NMT Code symbol in `image`. An image without a symbol, and an image that is not
/// usable ([`LumaImage::new`]), gives an empty [`Scan`].
pub fn find(image: &LumaImage, options: &DetectOptions) -> Scan {
    if !image.is_usable() {
        return Scan::default();
    }
    let upright = scan_pyramid(image, options);
    if !upright.found.is_empty() || !options.try_inverted {
        return upright;
    }
    // 1.4 and 5.11: a reader MAY try the inverted image, and reads it as if captured dark on
    // light. It is tried only when the image as captured holds no symbol.
    let mut inverted = scan_pyramid(&image.inverted(), options);
    if inverted.found.is_empty() && inverted.rejected.is_empty() {
        return upright;
    }
    inverted.inverted = true;
    for f in &mut inverted.found {
        f.inverted = true;
    }
    if inverted.found.is_empty() && !upright.rejected.is_empty() {
        return upright;
    }
    inverted
}

/// [`scan`] of `image`, and of half-size copies of a large image while nothing is found.
fn scan_pyramid(image: &LumaImage, options: &DetectOptions) -> Scan {
    let mut result = scan(image, options);
    let mut level = None::<LumaImage>;
    let mut factor = 1.0;
    for _ in 0..3 {
        if !result.found.is_empty() || !result.rejected.is_empty() {
            break;
        }
        let current = level.as_ref().unwrap_or(image);
        if current.width.min(current.height) < PYRAMID_MIN_SIDE {
            break;
        }
        let Some(half) = halve(current) else { break };
        factor *= 2.0;
        let mut s = scan(&half, options);
        for f in &mut s.found {
            f.corners = f.corners.map(|p| p.scale(factor));
        }
        for r in &mut s.rejected {
            r.corners = r.corners.map(|p| p.scale(factor));
        }
        result = s;
        level = Some(half);
    }
    result
}

/// The image at half size, each pixel the mean of a 2 × 2 block.
fn halve(image: &LumaImage) -> Option<LumaImage> {
    let (w, h) = (image.width / 2, image.height / 2);
    let (wu, src_w) = (usize::try_from(w).ok()?, image.w_usize());
    let mut pixels = Vec::with_capacity(wu * usize::try_from(h).ok()?);
    for y in 0..usize::try_from(h).ok()? {
        let r0 = image.pixels.get(2 * y * src_w..(2 * y + 1) * src_w)?;
        let r1 = image.pixels.get((2 * y + 1) * src_w..(2 * y + 2) * src_w)?;
        for x in 0..wu {
            let s = u16::from(*r0.get(2 * x)?)
                + u16::from(*r0.get(2 * x + 1)?)
                + u16::from(*r1.get(2 * x)?)
                + u16::from(*r1.get(2 * x + 1)?);
            pixels.push(u8::try_from((s + 2) / 4).unwrap_or(u8::MAX));
        }
    }
    LumaImage::new(w, h, pixels)
}

/// Finder candidates of `image` under a threshold at `fraction` (see `binarize`), best first,
/// and the image's noise estimate.
fn finders(image: &LumaImage, fraction: f64) -> (Vec<finder::Finder>, f64) {
    let thresholds = binarize::thresholds(image, fraction);
    let mut out = Vec::new();
    let mut checked = 0usize;
    blobs::for_each_blob(image, &thresholds, MIN_FINDER_PX, |blob, spans| {
        checked += 1;
        if let Some(f) = finder::candidate(image, &thresholds, blob, spans) {
            out.push(f);
        }
        checked < MAX_BLOB_CHECKS
    });
    out.sort_by(|a, b| a.best_kind().1.total_cmp(&b.best_kind().1));
    out.truncate(group::MAX_FINDERS);
    (out, thresholds.noise)
}

/// Threshold positions of the finder search: the midpoint, then, when that finds no symbol, a
/// threshold nearer the dark level, which keeps the one-module separators of a blurred symbol
/// light where the midpoint joins its finders to the data, and one nearer the light level,
/// which keeps a blurred one-module ring dark where the midpoint breaks it.
const FRACTIONS: [f64; 3] = [0.5, 0.33, 0.67];

/// One pass over `image` as captured: [`scan_with`] at the midpoint threshold and, when that
/// finds no symbol, again with the finders of both thresholds.
fn scan(image: &LumaImage, options: &DetectOptions) -> Scan {
    let (mut finders, noise) = finders(image, FRACTIONS[0]);
    let first = scan_with(image, options, &finders, noise);
    if !first.found.is_empty() {
        return first;
    }
    let before = finders.len();
    for &fraction in FRACTIONS.iter().skip(1) {
        let (more, _) = self::finders(image, fraction);
        for f in more {
            let duplicate =
                finders.iter().any(|g| g.centre().dist(f.centre()) < 1.5 * g.pitch.min(f.pitch));
            if !duplicate && finders.len() < group::MAX_FINDERS * 2 {
                finders.push(f);
            }
        }
    }
    if finders.len() == before {
        return first;
    }
    let second = scan_with(image, options, &finders, noise);
    if second.found.is_empty() && second.rejected.is_empty() { first } else { second }
}

/// A symbol read from a candidate, with its nesting area and the finders it used.
type FoundEntry = (Found, [Point; 4], Vec<usize>);

/// What one pass has read so far.
struct Reading {
    /// Finders used by a symbol found.
    used: Vec<bool>,
    found: Vec<FoundEntry>,
    rejected: Vec<(Rejected, Vec<usize>)>,
}

/// Reads `list` in order, skipping candidates that share a finder with a symbol already found.
fn read_all(
    image: &LumaImage,
    options: &DetectOptions,
    (finders, noise): (&[finder::Finder], f64),
    list: &[group::Candidate],
    reading: &mut Reading,
) {
    let Reading { used, found, rejected } = reading;
    for candidate in list.iter().take(MAX_CANDIDATES) {
        let members: Vec<usize> = candidate.finders().collect();
        if members.iter().any(|&i| used.get(i).copied().unwrap_or(true)) {
            continue;
        }
        match symbol::read(image, noise, finders, candidate, options) {
            symbol::Outcome::Found(f, area) => {
                for &i in &members {
                    if let Some(u) = used.get_mut(i) {
                        *u = true;
                    }
                }
                found.push((f, area, members));
            }
            symbol::Outcome::Rejected(r) => {
                if !rejected.iter().any(|(_, m)| m.iter().any(|i| members.contains(i))) {
                    rejected.push((r, members));
                }
            }
            symbol::Outcome::Nothing => {}
        }
    }
}

/// Groups `finders` into candidates and reads them: groups of three or four finders, then,
/// when no group gave a symbol, pairs of finders at opposite corners or on one edge, then
/// single solid finders with the two edges traced from them. A candidate becomes a symbol only
/// with at least three finders (see `symbol::read`).
fn scan_with(
    image: &LumaImage,
    options: &DetectOptions,
    finders: &[finder::Finder],
    noise: f64,
) -> Scan {
    let mut reading =
        Reading { used: vec![false; finders.len()], found: Vec::new(), rejected: Vec::new() };
    let groups = group::candidates(finders);
    read_all(image, options, (finders, noise), &groups, &mut reading);
    if reading.found.is_empty() {
        let mut pairs = group::pairs(finders, &reading.used);
        pairs.extend(group::edge_pairs(image, finders, &reading.used));
        read_all(image, options, (finders, noise), &pairs, &mut reading);
    }
    if reading.found.is_empty() {
        let singles = group::singles(image, finders, &reading.used);
        read_all(image, options, (finders, noise), &singles, &mut reading);
    }
    let Reading { used, found, rejected } = reading;
    // A rejected candidate that shares a finder with a found symbol was a wrong grouping.
    let rejected: Vec<Rejected> = rejected
        .into_iter()
        .filter(|(_, m)| !m.iter().any(|i| used.get(*i).copied().unwrap_or(false)))
        .map(|(r, _)| r)
        .collect();
    let (found, nested) = split_nested(found);
    let mut scan = Scan { found, rejected, inverted: false };
    scan.rejected.extend(nested);
    scan
}

/// The nesting rule of 5.11 step 6: a symbol whose area lies inside the area of another found
/// symbol, with all four corners, and that other symbol, are both left out and reported.
fn split_nested(found: Vec<FoundEntry>) -> (Vec<Found>, Vec<Rejected>) {
    let n = found.len();
    let mut nested = vec![false; n];
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let (Some((_, inner, _)), Some((_, outer, _))) = (found.get(i), found.get(j)) else {
                continue;
            };
            if inner.iter().all(|&p| geom::inside_convex(outer, p)) {
                if let Some(f) = nested.get_mut(i) {
                    *f = true;
                }
                if let Some(f) = nested.get_mut(j) {
                    *f = true;
                }
            }
        }
    }
    let mut kept = Vec::new();
    let mut out = Vec::new();
    for ((f, area, _), is_nested) in found.into_iter().zip(nested) {
        if is_nested {
            out.push(Rejected { corners: area, error: Error::NestedSymbol });
        } else {
            kept.push(f);
        }
    }
    (kept, out)
}

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
/// found: [`find`] with the default options, grids only. An image without a symbol, and an
/// image whose pixel buffer does not hold exactly `width × height` pixels or that has more than
/// [`MAX_PIXELS`] pixels, gives an empty list. Nested symbols are left out; [`detect`] also
/// counts them.
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
    let scan = find(image, &DetectOptions::default());
    Detection {
        nested: scan.rejected.iter().filter(|r| r.error == Error::NestedSymbol).count(),
        symbols: scan.found.into_iter().map(|f| f.grid).collect(),
        inverted: scan.inverted,
    }
}

/// The symbols found in one image, grids only.
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

/// One finder candidate, for measurement and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinderCandidate {
    /// The candidate's corners in the image, in increasing angle.
    pub corners: [Point; 4],
    /// Its centre in the image.
    pub centre: Point,
    /// For each kind in the order TL, TR, BL, BR: the smallest sum of squared differences of
    /// the inner 3 × 3 against that finder over the eight transforms, in units of the contrast.
    /// The smallest entry is the kind the candidate is classified as.
    pub scores: [f64; 4],
}

/// Every finder candidate of `image` as captured (no inversion, full size), best first: the
/// dark squares with a light border that step 2 of [`find`] keeps.
pub fn finder_candidates(image: &LumaImage) -> Vec<FinderCandidate> {
    if !image.is_usable() {
        return Vec::new();
    }
    finders(image, FRACTIONS[0])
        .0
        .iter()
        .map(|f| FinderCandidate {
            corners: f.corners,
            centre: f.centre(),
            scores: [f.kind_score(0), f.kind_score(1), f.kind_score(2), f.kind_score(3)],
        })
        .collect()
}

/// Why an image could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DetectError {
    /// The bytes do not start with the PNG signature ([`read_png`], [`decode_png`]).
    NotPng,
    /// The bytes are neither a PNG nor a JPEG ([`read_image`]).
    UnknownFormat,
    /// The PNG is damaged or truncated; the message is the decoder's.
    Malformed(String),
    /// A well-formed PNG that this reader does not handle; the message says what.
    Unsupported(String),
    /// The JPEG is damaged, truncated or of a kind the decoder does not handle; the message is
    /// the decoder's.
    Jpeg(String),
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
            Self::UnknownFormat => f.write_str("the data is not a PNG or JPEG image"),
            Self::Malformed(message) => write!(f, "the PNG image is damaged: {message}"),
            Self::Unsupported(message) => write!(f, "the PNG image is not supported: {message}"),
            Self::Jpeg(message) => write!(f, "the JPEG image cannot be read: {message}"),
            Self::TooLarge { width, height } => write!(
                f,
                "the image is {width} x {height} pixels, more than the {MAX_PIXELS} pixels accepted"
            ),
        }
    }
}

impl std::error::Error for DetectError {}

#[cfg(test)]
mod tests {
    use super::symbol::within_tolerance;

    #[test]
    fn size_tolerance_of_5_11() {
        // max(4, W / 10): 4 modules up to W = 40, then 10%.
        assert!(within_tolerance(20, 24.0) && within_tolerance(20, 16.0));
        assert!(!within_tolerance(20, 24.1) && !within_tolerance(20, 15.9));
        assert!(within_tolerance(40, 44.0) && !within_tolerance(40, 44.1));
        assert!(within_tolerance(100, 110.0) && !within_tolerance(100, 110.1));
        assert!(within_tolerance(100, 90.0) && !within_tolerance(100, 89.9));
        assert!(within_tolerance(4108, 3697.25) && !within_tolerance(4108, 3697.15));
    }
}
