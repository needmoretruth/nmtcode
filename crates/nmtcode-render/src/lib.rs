//! PNG and SVG output of NMT Code symbols, with the optional bootstrap QR Code.
//!
//! The input is a finished [`ModuleGrid`] (quiet zone excluded, dark = `true`). The output is a
//! canvas that holds the symbol, its quiet zone (specification 5.2) and, by default, the bootstrap
//! QR Code of chapter 8 beside it.
//!
//! - [`render_png`] writes a 1-bit greyscale PNG, black modules on white.
//! - [`render_svg`] writes a compact SVG: runs of dark modules merged into one path.
//! - [`render_modules`] returns the canvas as a module grid, for other output formats.
//! - [`layout`] computes the canvas and the bootstrap placement of 8.4 without drawing.
//!
//! QR Code is a registered trademark of DENSO WAVE INCORPORATED.

mod bootstrap;
mod canvas;
mod layout;
mod png_out;
mod svg_out;

use core::fmt;

pub use bootstrap::{BOOTSTRAP_URL, bootstrap_qr};
pub use canvas::{Canvas, render_modules};
pub use layout::{BootstrapLayout, Layout, layout};
use nmtcode_core::ModuleGrid;

/// Smallest quiet zone in modules on every side (specification 5.2).
pub const MIN_QUIET_ZONE: u32 = 2;
/// Default quiet zone in modules (specification 5.2).
pub const DEFAULT_QUIET_ZONE: u32 = 2;
/// Default module size in pixels: the `screen` profile of specification 1.5.
pub const DEFAULT_MODULE_PX: u32 = 4;
/// Smallest printed module of the `print` profile (specification 1.5), in millimetres.
pub const PRINT_MIN_MODULE_MM: f64 = 0.4;
/// Smallest number of printer dots per module of the `print` profile (specification 1.5).
pub const PRINT_MIN_DOTS: u32 = 4;

/// Error-correction level of the bootstrap QR Code (specification 8.3). Level L is not offered:
/// it gives no smaller symbol for the URL constant and only loses robustness (8.3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BootstrapLevel {
    /// Level M, the default: QR version 3 for the URL constant.
    #[default]
    M,
    /// Level Q: QR version 4 for the URL constant.
    Q,
    /// Level H: QR version 5 for the URL constant.
    H,
}

/// Side of the NMT Code symbol on which the bootstrap QR Code sits (specification 8.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BootstrapSide {
    /// To the left, top edges aligned. The default when W ≥ H.
    Left,
    /// Above, left edges aligned. The default when H > W.
    Above,
}

/// How a symbol is drawn.
///
/// [`RenderOptions::default`] is the `screen` profile of specification 1.5: 4 pixels per module,
/// a quiet zone of 2 modules and the bootstrap QR Code on (8.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// Side of one module in pixels (PNG pixels, SVG user units). At least 1.
    pub module_px: u32,
    /// Quiet zone in modules on every side of the symbol. At least [`MIN_QUIET_ZONE`].
    pub quiet_zone: u32,
    /// Whether to draw the bootstrap QR Code of chapter 8. On by default for static symbols
    /// (8.5); set to `false` to leave it out.
    pub bootstrap: bool,
    /// Error-correction level of the bootstrap QR Code. Default M (8.3).
    pub bootstrap_level: BootstrapLevel,
    /// Side of the bootstrap QR Code. `None` picks the default of 8.4.3: left when W ≥ H,
    /// above when H > W.
    pub bootstrap_side: Option<BootstrapSide>,
    /// Width of one bootstrap QR module in NMT Code modules (n of 8.4.1). `None` picks the
    /// smallest n allowed by 8.4.1; a value below that minimum is an error.
    pub bootstrap_scale: Option<u32>,
    /// Pixel density of the output in dots per inch. When set, the PNG carries it in a `pHYs`
    /// chunk, the SVG states its size in millimetres, and the bootstrap QR module is kept at
    /// 0.4 mm or more (8.4.1). When `None`, the output has no physical size and the bootstrap QR
    /// module is kept at 4 pixels or more.
    pub dpi: Option<u32>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            module_px: DEFAULT_MODULE_PX,
            quiet_zone: DEFAULT_QUIET_ZONE,
            bootstrap: true,
            bootstrap_level: BootstrapLevel::M,
            bootstrap_side: None,
            bootstrap_scale: None,
            dpi: None,
        }
    }
}

impl RenderOptions {
    /// The `print` profile of specification 1.5 for a printer of `dpi` dots per inch: the
    /// smallest whole number of dots that is at least 0.4 mm and at least 4 dots, with the DPI
    /// written into the output.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidDpi`] when `dpi` is 0.
    pub fn print(dpi: u32) -> Result<Self, RenderError> {
        Self::for_print(dpi, PRINT_MIN_MODULE_MM)
    }

    /// Like [`RenderOptions::print`], with a minimum module size of `min_module_mm` millimetres
    /// instead of 0.4 mm. See [`print_module_px`].
    ///
    /// # Errors
    ///
    /// The errors of [`print_module_px`].
    pub fn for_print(dpi: u32, min_module_mm: f64) -> Result<Self, RenderError> {
        Ok(Self {
            module_px: print_module_px(dpi, min_module_mm)?,
            dpi: Some(dpi),
            ..Self::default()
        })
    }
}

/// The module size in printer dots for a printer of `dpi` dots per inch: the smallest whole
/// number of dots that is at least `min_module_mm` millimetres and at least [`PRINT_MIN_DOTS`]
/// dots (the `print` profile of specification 1.5 uses [`PRINT_MIN_MODULE_MM`] = 0.4 mm).
///
/// `min_module_mm` is rounded to the nearest micrometre first. For example 600 dpi and 0.4 mm
/// give 10 dots (0.423 mm), 300 dpi gives 5 dots and 96 dpi gives the 4-dot floor.
///
/// # Errors
///
/// [`RenderError::InvalidDpi`] when `dpi` is 0; [`RenderError::InvalidModuleMm`] when
/// `min_module_mm` is not a finite number from 0.001 to 1000.
pub fn print_module_px(dpi: u32, min_module_mm: f64) -> Result<u32, RenderError> {
    if dpi == 0 {
        return Err(RenderError::InvalidDpi);
    }
    if !min_module_mm.is_finite() || !(0.001..=1000.0).contains(&min_module_mm) {
        return Err(RenderError::InvalidModuleMm);
    }
    // In range 1 ..= 1_000_000 after the check above, so the conversion is exact and positive.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let micrometres = (min_module_mm * 1000.0).round() as u64;
    // One inch is 25 400 µm: dots ≥ µm · dpi / 25 400.
    let dots = (micrometres * u64::from(dpi)).div_ceil(25_400).max(u64::from(PRINT_MIN_DOTS));
    u32::try_from(dots).map_err(|_| RenderError::TooLarge)
}

/// Writes `grid` as a PNG: 1-bit greyscale, black modules on white, with the quiet zone and, when
/// [`RenderOptions::bootstrap`] is on, the bootstrap QR Code. When [`RenderOptions::dpi`] is set,
/// the PNG has a `pHYs` chunk with that density.
///
/// # Errors
///
/// [`RenderError`] when the grid is not a valid symbol size, an option is out of range, the image
/// would be too large, or the PNG encoder fails.
pub fn render_png(grid: &ModuleGrid, options: &RenderOptions) -> Result<Vec<u8>, RenderError> {
    let canvas = render_modules(grid, options)?;
    png_out::write(&canvas, options)
}

/// Writes `grid` as an SVG document. The drawing is one white rectangle and one black path in
/// which runs of dark modules are merged into rectangles; `shape-rendering="crispEdges"`, a
/// `<title>`, no scripts. The view box is in modules; the size is in millimetres when
/// [`RenderOptions::dpi`] is set and in pixels otherwise.
///
/// # Errors
///
/// [`RenderError`] when the grid is not a valid symbol size, an option is out of range, or the
/// canvas would be too large.
pub fn render_svg(grid: &ModuleGrid, options: &RenderOptions) -> Result<String, RenderError> {
    let canvas = render_modules(grid, options)?;
    Ok(svg_out::write(&canvas, options))
}

/// Why a symbol could not be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// The grid is not a valid symbol size: width and height must be multiples of 4 from 20 to
    /// 4108 (specification 5.2).
    InvalidSymbolSize {
        /// Width of the grid in modules.
        width: u32,
        /// Height of the grid in modules.
        height: u32,
    },
    /// [`RenderOptions::module_px`] is 0.
    ModuleSizeZero,
    /// [`RenderOptions::quiet_zone`] is below [`MIN_QUIET_ZONE`] (specification 5.2).
    QuietZoneTooSmall {
        /// The quiet zone that was asked for.
        quiet_zone: u32,
    },
    /// The DPI is 0.
    InvalidDpi,
    /// A module size in millimetres is not a finite number from 0.001 to 1000.
    InvalidModuleMm,
    /// [`RenderOptions::bootstrap_scale`] is below the smallest bootstrap module width that
    /// specification 8.4.1 allows for this module size.
    BootstrapScaleTooSmall {
        /// The scale that was asked for.
        requested: u32,
        /// The smallest allowed scale.
        minimum: u32,
    },
    /// The canvas would exceed 2^20 modules on a side, 2^26 modules in total, 2^31 − 1 pixels on
    /// a side or 2^32 pixels in total.
    TooLarge,
    /// The QR Code encoder refused the bootstrap URL (not expected for the fixed URL constant).
    Bootstrap,
    /// The PNG encoder failed; the message is the encoder's.
    Png(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSymbolSize { width, height } => write!(
                f,
                "{width} x {height} is not a symbol size: sides are multiples of 4 from 20 to 4108"
            ),
            Self::ModuleSizeZero => f.write_str("the module size must be at least 1 pixel"),
            Self::QuietZoneTooSmall { quiet_zone } => write!(
                f,
                "a quiet zone of {quiet_zone} modules is too small: at least {MIN_QUIET_ZONE} are needed"
            ),
            Self::InvalidDpi => f.write_str("the DPI must be at least 1"),
            Self::InvalidModuleMm => {
                f.write_str("the module size in millimetres must be from 0.001 to 1000")
            }
            Self::BootstrapScaleTooSmall { requested, minimum } => write!(
                f,
                "a bootstrap module of {requested} modules is too small: at least {minimum} are needed"
            ),
            Self::TooLarge => f.write_str("the image would be too large"),
            Self::Bootstrap => f.write_str("the bootstrap code could not be encoded"),
            Self::Png(message) => write!(f, "PNG encoding failed: {message}"),
        }
    }
}

impl std::error::Error for RenderError {}
