//! The encoding order of chapter 1 (1.3): codec, container, error correction, placement and the
//! format word.

use core::fmt;

use nmtcode_core::{
    ContentType, FormatEcho, FormatWord, ModuleGrid, Record, RecordContent, RecordForm,
    StaticFields, SymbolClass, WriteError, pad_message,
};
use nmtcode_ecc::{BlockSplit, EccError};
use nmtcode_payload::Coded;
use nmtcode_symbol::{Layout, SymbolError};

use crate::CodecOptions;
use crate::size::{self, SizeFailure, SizeRule};

/// A named set of defaults (chapter 1, 1.5).
///
/// A reader never needs to know the profile: every value it needs is in the symbol. The module
/// size of a profile is a matter of drawing and belongs to the renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Profile {
    /// `screen`: black and white, level 0, modules of at least 0.4 mm and 4 device pixels
    /// (4 CSS pixels when the density is unknown). The default.
    #[default]
    Screen,
    /// `print`: black and white, level 1, modules of at least 0.4 mm and 4 printer dots, with
    /// the bootstrap QR Code of chapter 8 on by default.
    Print,
    /// `lowend`: black and white, level 1, large modules for cameras that see about 2 pixels
    /// per module.
    LowEnd,
    /// `color`: colour profile 1 of chapter 7. Not implemented in this version: [`encode`]
    /// refuses it with [`EncodeError::ColourNotImplemented`].
    Color,
}

impl Profile {
    /// The name used in the specification: `screen`, `print`, `lowend` or `color`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Print => "print",
            Self::LowEnd => "lowend",
            Self::Color => "color",
        }
    }

    /// The default error-correction level (1.5): 0 for `screen` and `color`, 1 for `print` and
    /// `lowend`.
    pub const fn default_level(self) -> u8 {
        match self {
            Self::Screen | Self::Color => 0,
            Self::Print | Self::LowEnd => 1,
        }
    }

    /// The colour profile of the format word (chapter 2, 2.2): 1 for `color`, else 0.
    pub const fn colour_profile(self) -> u8 {
        match self {
            Self::Color => 1,
            Self::Screen | Self::Print | Self::LowEnd => 0,
        }
    }
}

/// What [`encode`] makes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodeOptions {
    /// The profile of 1.5. Its default level applies when [`EncodeOptions::level`] is `None`.
    pub profile: Profile,
    /// The error-correction level, 0 to 3 (chapter 4, 4.5), instead of the profile's default.
    pub level: Option<u8>,
    /// How the symbol size is chosen. The default is the RECOMMENDED rule of 1.5: the smallest
    /// area with sides within a factor of 2 of each other.
    pub size: SizeRule,
    /// Which codecs are tried (chapter 6, 6.3). The default tries every codec.
    pub codecs: CodecOptions,
}

impl EncodeOptions {
    /// The error-correction level that applies: [`EncodeOptions::level`], or the profile's.
    pub fn effective_level(&self) -> u8 {
        self.level.unwrap_or(self.profile.default_level())
    }
}

/// Why [`encode`] made no symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EncodeError {
    /// The `color` profile (chapter 7) is not implemented in this version.
    ColourNotImplemented,
    /// The error-correction level is not 0 to 3.
    InvalidLevel(u8),
    /// The exact size is not a valid size: each side must be a multiple of 4 from 20 to 4108.
    InvalidSize {
        /// The requested width.
        width: u32,
        /// The requested height.
        height: u32,
    },
    /// The records break a rule of chapter 3 (no record, the action rule, a file name without a
    /// file, more than 1 MiB).
    Records(WriteError),
    /// The container does not fit the requested exact size.
    DoesNotFit {
        /// The requested width.
        width: u32,
        /// The requested height.
        height: u32,
        /// The error-correction level.
        level: u8,
        /// The container length in bytes, CRC-32C included.
        container_len: usize,
        /// The message capacity K of that size and level.
        capacity: usize,
    },
    /// No symbol size that meets the size rule holds the container.
    NoSize {
        /// The error-correction level.
        level: u8,
        /// The container length in bytes, CRC-32C included.
        container_len: usize,
    },
    /// A layer crate refused a value that this crate computed. Not expected; reported instead of
    /// a panic.
    Internal(&'static str),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ColourNotImplemented => {
                f.write_str("the color profile is not implemented in this version")
            }
            Self::InvalidLevel(level) => {
                write!(f, "error-correction level {level} is not 0 to 3")
            }
            Self::InvalidSize { width, height } => write!(
                f,
                "{width} x {height} is not a symbol size: each side is a multiple of 4 from 20 to 4108"
            ),
            Self::Records(error) => write!(f, "{error}"),
            Self::DoesNotFit { width, height, level, container_len, capacity } => write!(
                f,
                "a {container_len}-byte container does not fit {width} x {height} at level {level}, \
                 which holds {capacity} bytes"
            ),
            Self::NoSize { level, container_len } => write!(
                f,
                "no symbol size allowed by the size rule holds a {container_len}-byte container at \
                 level {level}"
            ),
            Self::Internal(what) => write!(f, "internal error: {what}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// A finished symbol and the values chosen on the way (chapter 1, 1.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    grid: ModuleGrid,
    format: FormatWord,
    profile: Profile,
    fields: StaticFields,
    coded_len: usize,
    container_len: usize,
    message: Vec<u8>,
    stream: Vec<u8>,
    split: BlockSplit,
}

impl Symbol {
    /// The module grid, quiet zone excluded (dark = `true`).
    pub fn grid(&self) -> &ModuleGrid {
        &self.grid
    }

    /// The module grid, by value.
    pub fn into_grid(self) -> ModuleGrid {
        self.grid
    }

    /// The format word fields (chapter 2): size, level, colour profile.
    pub fn format(&self) -> &FormatWord {
        &self.format
    }

    /// Width W in modules.
    pub fn width(&self) -> u32 {
        self.format.width()
    }

    /// Height H in modules.
    pub fn height(&self) -> u32 {
        self.format.height()
    }

    /// The error-correction level, 0 to 3.
    pub fn level(&self) -> u8 {
        self.format.level()
    }

    /// The profile the symbol was made with.
    pub fn profile(&self) -> Profile {
        self.profile
    }

    /// The chosen codec ID (chapter 6, 6.2).
    pub fn codec(&self) -> u32 {
        self.fields.codec
    }

    /// The chosen dictionary ID, 0 for the codec's built-in model or for a codec that takes
    /// none (chapter 6, 6.11).
    pub fn dictionary(&self) -> u32 {
        self.fields.dictionary
    }

    /// The decoded length L in bytes (chapter 3, 3.2.4).
    pub fn decoded_len(&self) -> u32 {
        self.fields.decoded_len
    }

    /// The length of the coded field Lc in bytes (chapter 6, 6.1).
    pub fn coded_len(&self) -> usize {
        self.coded_len
    }

    /// The record form: single-record form with its content type, or record-list form with
    /// its record count (chapter 3, 3.2.5).
    pub fn record_form(&self) -> RecordForm {
        self.fields.form
    }

    /// The container length in bytes, CRC-32C included and padding excluded: the part of the
    /// capacity that is used.
    pub fn container_len(&self) -> usize {
        self.container_len
    }

    /// The message capacity K in bytes (chapter 4, 4.6).
    pub fn capacity(&self) -> usize {
        self.message.len()
    }

    /// The number of codewords N of the data area (chapter 5, 5.7).
    pub fn codewords(&self) -> usize {
        self.stream.len()
    }

    /// The container: lead byte, body length, header fields, coded field and CRC-32C
    /// (chapter 3, 3.2, 3.7).
    pub fn container(&self) -> &[u8] {
        self.message.get(..self.container_len).unwrap_or_default()
    }

    /// The K-byte message: the container and its padding (chapter 3, 3.8).
    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// The N-byte codeword stream c\[0\] … c\[N − 1\] after Reed-Solomon encoding and
    /// interleaving (chapter 4, 4.8.2), before whitening.
    pub fn stream(&self) -> &[u8] {
        &self.stream
    }

    /// The block split of chapter 4 (4.6).
    pub fn block_split(&self) -> &BlockSplit {
        &self.split
    }
}

fn internal_symbol(_: SymbolError) -> EncodeError {
    EncodeError::Internal("the symbol layout refused a computed value")
}

fn internal_ecc(_: EccError) -> EncodeError {
    EncodeError::Internal("error correction refused a computed value")
}

/// Encodes `records` into one static black-and-white symbol by the encoding order of chapter 1
/// (1.3).
///
/// 1. The records become the decoded content: one record in single-record form, more in
///    record-list form (chapter 3, 3.2.5), after the rules of 3.4.4.
/// 2. Every codec that [`EncodeOptions::codecs`] allows is tried and the one with the smallest
///    container wins (chapter 6, 6.3). A record of a "never compress" type (3.4.2) forces
///    codec 0.
/// 3. The container gets its format echo byte and CRC-32C, and the size rule picks the symbol
///    size (1.5).
/// 4. The message is padded to the capacity K (3.8), split into blocks with Reed-Solomon parity
///    and interleaved (chapter 4), placed and whitened (chapter 5), and the format word is
///    written (chapter 2).
///
/// Copy A of the format word carries `F_A` (masked with `MASK_A`) and copy B carries `F_B`
/// (masked with `MASK_B`), so either copy alone gives the format word to a reader (2.5, 2.7).
///
/// # Errors
///
/// - [`EncodeError::ColourNotImplemented`] for [`Profile::Color`].
/// - [`EncodeError::InvalidLevel`] for a level above 3.
/// - [`EncodeError::Records`] when the records break a rule of chapter 3.
/// - [`EncodeError::InvalidSize`], [`EncodeError::DoesNotFit`] or [`EncodeError::NoSize`] when
///   no size holds the container under the size rule.
pub fn encode(records: &[Record<'_>], options: &EncodeOptions) -> Result<Symbol, EncodeError> {
    if options.profile == Profile::Color {
        return Err(EncodeError::ColourNotImplemented);
    }
    let level = options.effective_level();
    if level > 3 {
        return Err(EncodeError::InvalidLevel(level));
    }
    let content = RecordContent::from_records(records).map_err(EncodeError::Records)?;
    let candidates = if content.stored_only {
        let bytes = nmtcode_payload::codec0::encode(&content.decoded);
        let decoded_len = u32::try_from(bytes.len())
            .map_err(|_| EncodeError::Records(WriteError::ContentTooLarge))?;
        vec![Coded { codec: nmtcode_payload::codec0::ID, dictionary: 0, decoded_len, bytes }]
    } else {
        nmtcode_payload::candidates(&content.decoded, &options.codecs)
    };
    let fields_of = |coded: &Coded| StaticFields {
        form: content.form,
        codec: coded.codec,
        dictionary: coded.dictionary,
        decoded_len: coded.decoded_len,
    };
    let chosen = nmtcode_payload::select(candidates, |coded| {
        fields_of(coded).container_len(coded.bytes.len()).unwrap_or(usize::MAX)
    })
    .ok_or(EncodeError::Records(WriteError::ContentTooLarge))?;
    let fields = fields_of(&chosen);
    let echo = FormatEcho::new(SymbolClass::Static, level, 0, 0)
        .map_err(|_| EncodeError::Internal("the format echo refused a computed value"))?;
    let mut message = fields.write(echo, &chosen.bytes).map_err(EncodeError::Records)?;
    let container_len = message.len();

    let (width, height) =
        size::choose(&options.size, container_len, level).map_err(|failure| match failure {
            SizeFailure::InvalidSize { width, height } => {
                EncodeError::InvalidSize { width, height }
            }
            SizeFailure::DoesNotFit { width, height, capacity } => {
                EncodeError::DoesNotFit { width, height, level, container_len, capacity }
            }
            SizeFailure::NoSize => EncodeError::NoSize { level, container_len },
        })?;

    let layout = Layout::new(width, height).map_err(internal_symbol)?;
    let split = nmtcode_ecc::split(layout.codeword_count(), level).map_err(internal_ecc)?;
    pad_message(&mut message, split.capacity()).map_err(EncodeError::Records)?;
    let stream = nmtcode_ecc::encode_stream(&split, &message).map_err(internal_ecc)?;
    let format = FormatWord::new(SymbolClass::Static, width, height, level, 0, 0)
        .map_err(|_| EncodeError::Internal("the format word refused a computed value"))?;
    let grid = layout.draw_copies(format.encode_copies(), &stream).map_err(internal_symbol)?;
    Ok(Symbol {
        grid,
        format,
        profile: options.profile,
        fields,
        coded_len: chosen.bytes.len(),
        container_len,
        message,
        stream,
        split,
    })
}

/// Encodes UTF-8 text as one record of content type 1 (chapter 3, 3.4.2).
///
/// # Errors
///
/// As [`encode`].
pub fn encode_text(text: &str, options: &EncodeOptions) -> Result<Symbol, EncodeError> {
    encode(&[Record { content_type: ContentType::TEXT, value: text.as_bytes() }], options)
}

/// Encodes a URL (a URI or IRI) as one record of content type 2, the action type
/// (chapter 3, 3.4.2). A reader shows it in full and never opens it without a user action.
///
/// # Errors
///
/// As [`encode`].
pub fn encode_url(url: &str, options: &EncodeOptions) -> Result<Symbol, EncodeError> {
    encode(&[Record { content_type: ContentType::URL, value: url.as_bytes() }], options)
}

/// Encodes a file as a file name record (type 5) followed by a file record (type 4), in
/// record-list form (chapter 3, 3.4.2, 3.4.4 rule 3). An empty `name` gives the file record
/// alone, in single-record form.
///
/// The name is written as given; a reader never uses it as a path (3.4.3).
///
/// # Errors
///
/// As [`encode`].
pub fn encode_file(
    name: &str,
    bytes: &[u8],
    options: &EncodeOptions,
) -> Result<Symbol, EncodeError> {
    let file = Record { content_type: ContentType::FILE, value: bytes };
    if name.is_empty() {
        encode(&[file], options)
    } else {
        let name = Record { content_type: ContentType::FILE_NAME, value: name.as_bytes() };
        encode(&[name, file], options)
    }
}
