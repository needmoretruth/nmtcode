//! Making a symbol for the page: encode the content, then draw it as a PNG, an SVG and a
//! preview of one value per canvas module.

use nmtcode::{EncodeError, EncodeOptions, Profile, SizeRule};
use nmtcode_core::WriteError;
use nmtcode_render::{RenderError, RenderOptions};

/// Smallest module size in pixels that [`make`] accepts.
pub const MIN_MODULE_PX: u32 = 1;
/// Largest module size in pixels that [`make`] accepts.
pub const MAX_MODULE_PX: u32 = 64;
/// Largest PNG that [`make`] draws, in pixels (8192 × 8192). A browser decodes an image to
/// 4 bytes per pixel, so a larger one would take more than 256 MiB to show.
pub const MAX_IMAGE_PIXELS: u64 = 1 << 26;

/// What the content is (chapter 3, 3.4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// UTF-8 text: one record of type 1.
    Text,
    /// A URL: one record of type 2, the action type.
    Url,
    /// A file: a file name record (type 5) and a file record (type 4), or the file record
    /// alone when the name is empty.
    File,
}

impl Kind {
    /// The kind named `text`, `url` or `file`; `None` for any other name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::Text),
            "url" => Some(Self::Url),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

/// How [`make`] encodes and draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MakeOptions {
    /// Error-correction level, 0 to 3 (chapter 4, 4.5).
    pub level: u8,
    /// Side of one module in pixels, [`MIN_MODULE_PX`] to [`MAX_MODULE_PX`].
    pub module_px: u32,
    /// Whether to draw the bootstrap QR Code of chapter 8 beside the symbol.
    pub bootstrap: bool,
}

impl Default for MakeOptions {
    /// The `screen` profile of chapter 1 (1.5) at an unknown pixel density: level 0, 4 pixels
    /// per module, no bootstrap QR Code (8.5).
    fn default() -> Self {
        Self { level: 0, module_px: nmtcode_render::DEFAULT_MODULE_PX, bootstrap: false }
    }
}

/// A symbol made by [`make`], drawn three ways, with the values the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    /// The PNG: 1-bit greyscale, black modules on white, quiet zone included.
    pub png: Vec<u8>,
    /// The SVG document.
    pub svg: String,
    /// One byte per canvas module, row by row from the top-left: 1 = dark, 0 = light. The
    /// canvas is the symbol, its quiet zone and the bootstrap QR Code when it is on.
    pub preview: Vec<u8>,
    /// Width of the canvas in modules.
    pub preview_width: u32,
    /// Height of the canvas in modules.
    pub preview_height: u32,
    /// Width of the PNG in pixels.
    pub image_width: u32,
    /// Height of the PNG in pixels.
    pub image_height: u32,
    /// Width W of the symbol in modules.
    pub width: u32,
    /// Height H of the symbol in modules.
    pub height: u32,
    /// Error-correction level, 0 to 3.
    pub level: u8,
    /// The codec ID the encoder chose (chapter 6, 6.2).
    pub codec: u32,
    /// The dictionary ID, 0 for the built-in model or none (chapter 6, 6.11).
    pub dictionary: u32,
    /// Bytes of the message that the container uses, CRC-32C included (chapter 3).
    pub bytes_used: usize,
    /// The message capacity K in bytes (chapter 4, 4.6).
    pub capacity: usize,
}

/// Why [`make`] made nothing. [`MakeError::code`] is the name the page maps to a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MakeError {
    /// The kind is not `text`, `url` or `file`.
    UnknownKind,
    /// Text or a URL that is not valid UTF-8.
    NotUtf8,
    /// The error-correction level is not 0 to 3.
    InvalidLevel,
    /// The module size is not [`MIN_MODULE_PX`] to [`MAX_MODULE_PX`] pixels.
    InvalidModuleSize,
    /// The content is larger than one symbol holds: more than 1 MiB of decoded content
    /// (chapter 6, 6.4).
    TooLarge,
    /// No symbol size holds the container at this level.
    NoSize,
    /// The PNG would have more than [`MAX_IMAGE_PIXELS`] pixels.
    ImageTooLarge,
    /// A layer crate refused a value computed here. Not expected.
    Internal,
}

impl MakeError {
    /// A stable lower-case name: `unknown_kind`, `not_utf8`, `invalid_level`,
    /// `invalid_module_size`, `too_large`, `no_size`, `image_too_large` or `internal`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownKind => "unknown_kind",
            Self::NotUtf8 => "not_utf8",
            Self::InvalidLevel => "invalid_level",
            Self::InvalidModuleSize => "invalid_module_size",
            Self::TooLarge => "too_large",
            Self::NoSize => "no_size",
            Self::ImageTooLarge => "image_too_large",
            Self::Internal => "internal",
        }
    }
}

fn from_encode(error: EncodeError) -> MakeError {
    match error {
        EncodeError::InvalidLevel(_) => MakeError::InvalidLevel,
        EncodeError::Records(WriteError::ContentTooLarge) => MakeError::TooLarge,
        EncodeError::DoesNotFit { .. } | EncodeError::NoSize { .. } => MakeError::NoSize,
        _ => MakeError::Internal,
    }
}

fn from_render(error: &RenderError) -> MakeError {
    match error {
        RenderError::TooLarge => MakeError::ImageTooLarge,
        _ => MakeError::Internal,
    }
}

/// Encodes `data` as `kind` with the `screen` profile at `options.level`, in the recommended
/// size of chapter 1 (1.5), and draws it at `options.module_px` pixels per module.
///
/// For [`Kind::File`], `file_name` is written as the file name record; it is ignored for the
/// other kinds.
///
/// # Errors
///
/// [`MakeError`]: invalid options, text that is not UTF-8, content too large for one symbol,
/// or an image above [`MAX_IMAGE_PIXELS`].
pub fn make(
    kind: Kind,
    data: &[u8],
    file_name: &str,
    options: MakeOptions,
) -> Result<Made, MakeError> {
    if options.level > 3 {
        return Err(MakeError::InvalidLevel);
    }
    if !(MIN_MODULE_PX..=MAX_MODULE_PX).contains(&options.module_px) {
        return Err(MakeError::InvalidModuleSize);
    }
    let encode_options = EncodeOptions {
        profile: Profile::Screen,
        level: Some(options.level),
        size: SizeRule::Recommended,
        ..EncodeOptions::default()
    };
    let text = || core::str::from_utf8(data).map_err(|_| MakeError::NotUtf8);
    let symbol = match kind {
        Kind::Text => nmtcode::encode_text(text()?, &encode_options),
        Kind::Url => nmtcode::encode_url(text()?, &encode_options),
        Kind::File => nmtcode::encode_file(file_name, data, &encode_options),
    }
    .map_err(from_encode)?;

    let render_options = RenderOptions {
        module_px: options.module_px,
        bootstrap: options.bootstrap,
        ..RenderOptions::default()
    };
    let layout = nmtcode_render::layout(symbol.width(), symbol.height(), &render_options)
        .map_err(|error| from_render(&error))?;
    let image_width = u64::from(layout.canvas_width) * u64::from(options.module_px);
    let image_height = u64::from(layout.canvas_height) * u64::from(options.module_px);
    if image_width.saturating_mul(image_height) > MAX_IMAGE_PIXELS {
        return Err(MakeError::ImageTooLarge);
    }
    let canvas = nmtcode_render::render_modules(symbol.grid(), &render_options)
        .map_err(|error| from_render(&error))?;
    let (columns, rows) = (canvas.modules.width(), canvas.modules.height());
    let preview = (0..rows)
        .flat_map(|y| (0..columns).map(move |x| (x, y)))
        .map(|(x, y)| u8::from(canvas.modules.get(x, y) == Some(true)))
        .collect();
    let png = nmtcode_render::render_png(symbol.grid(), &render_options)
        .map_err(|error| from_render(&error))?;
    let svg = nmtcode_render::render_svg(symbol.grid(), &render_options)
        .map_err(|error| from_render(&error))?;
    Ok(Made {
        png,
        svg,
        preview,
        preview_width: columns,
        preview_height: rows,
        image_width: u32::try_from(image_width).map_err(|_| MakeError::ImageTooLarge)?,
        image_height: u32::try_from(image_height).map_err(|_| MakeError::ImageTooLarge)?,
        width: symbol.width(),
        height: symbol.height(),
        level: symbol.level(),
        codec: symbol.codec(),
        dictionary: symbol.dictionary(),
        bytes_used: symbol.container_len(),
        capacity: symbol.capacity(),
    })
}
