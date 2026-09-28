//! Every string the `nmtcode` command shows to a person: help, messages and errors.
//!
//! The wording lives here, and only here, so that it can be rewritten without touching the
//! logic. Machine-readable names (error names, JSON keys) are not in this module.

use nmtcode::SpecError;

/// `nmtcode --version`.
pub const VERSION: &str = concat!("nmtcode ", env!("CARGO_PKG_VERSION"), "\n");

/// `nmtcode --help`.
pub const MAIN_HELP: &str = "\
NMT Code: make a two-dimensional code as a PNG or SVG image, and read one back.

Usage:
  nmtcode make [options]        make a symbol
  nmtcode read [options] PATH   read the symbols in a PNG image
  nmtcode --help                show this help
  nmtcode --version             show the version

Run 'nmtcode make --help' or 'nmtcode read --help' for their options.
";

/// `nmtcode make --help`.
pub const MAKE_HELP: &str = "\
Make an NMT Code symbol.

Usage: nmtcode make [--text TEXT | --url URL | --file PATH] [options]

Content, one of (without any, UTF-8 text is read from standard input):
  --text TEXT          text
  --url URL            a URL
  --file PATH          a file, stored with its name

Output:
  -o, --output PATH    file to write; .png or .svg picks the format; - writes to standard
                       output (the default)
  --format png|svg     output format; needed for standard output

Symbol:
  --profile NAME       screen (the default: level 0), print (level 1, modules of at least
                       0.4 mm) or lowend (level 1, 8-pixel modules)
  --level 0-3          error-correction level; about 7.5, 15, 25 or 30% of the bytes can be
                       restored
  --size WxH           exact size in modules; each side a multiple of 4 from 20 to 4108
  --max-width N        largest width in modules
  --max-height N       largest height in modules
  --codec LIST         codecs to try, comma-separated: stored, digits, alphanumeric, token,
                       hangul, brotli, or their numbers 0-5; stored is always tried
                       (default: all)

Drawing:
  --module-px N        module size in pixels (default 4; lowend 8; print: from the DPI)
  --dpi N              pixel density written into the image; with print it also sets the
                       module size (print default: 300)
  --quiet-zone N       light margin in modules, at least 2 (default 2)
  --qr                 add a QR Code beside the symbol that links to NMT Code (the default
                       with print)
  --no-qr              leave out that QR Code

QR Code is a registered trademark of DENSO WAVE INCORPORATED.
";

/// `nmtcode read --help`.
pub const READ_HELP: &str = "\
Read the NMT Code symbols in a PNG image.

Usage: nmtcode read [options] PATH

PATH is a PNG file, or - for standard input. Text and URLs are printed on standard output;
URLs are never opened. Other data is printed in hexadecimal. Files are saved only with --out.

Options:
  --out DIR            save files into DIR, which must exist; existing files are never
                       overwritten, and names are stripped of directories, leading dots and
                       control characters
  --json               print the result as JSON
";

/// The prefix of every error line.
pub const ERROR_PREFIX: &str = "nmtcode: ";

/// A line that tells a person where the help is, after a usage error.
pub const SEE_HELP: &str = "Run 'nmtcode --help' for usage.";

/// A short English message for each reader error of the specification (9.8).
pub const fn spec_error(error: SpecError) -> &'static str {
    match error {
        SpecError::NestedSymbol => "one symbol lies inside another, so neither is shown",
        SpecError::FormatUnreadable => "the format information of the symbol cannot be read",
        SpecError::FormatConflict => "the two copies of the format information disagree",
        SpecError::FormatVersion => "the symbol uses a newer format; a newer reader is needed",
        SpecError::SizeLimit => "the symbol is larger than this reader accepts",
        SpecError::TransferUnsupported => {
            "the symbol is one frame of a transfer, which this reader does not support"
        }
        SpecError::EccFailed => "the symbol is too damaged to correct",
        SpecError::ColourAmbiguous => "the colours of the symbol do not match its colour profile",
        SpecError::LengthField => "the symbol is damaged: its length field is invalid",
        SpecError::CrcMismatch => "the symbol is damaged: its integrity check failed",
        SpecError::ContainerVersion => {
            "the symbol uses a newer container; a newer reader is needed"
        }
        SpecError::TileReservedBits => "the transfer frame has reserved bits set",
        SpecError::FormatEcho => "the symbol's content does not match its format information",
        SpecError::ColourFlag => "the symbol claims colour content without a colour profile",
        SpecError::Leb128 => "the symbol has an invalid number field",
        SpecError::CodecEscape => "the symbol has an invalid codec field",
        SpecError::UnsupportedCodec => "the symbol uses a codec this reader does not know",
        SpecError::UnknownDictionary => "the symbol uses a dictionary this reader does not carry",
        SpecError::DictionaryMismatch => "the symbol names a dictionary of the wrong kind",
        SpecError::HashIdInvalid => "the symbol has an invalid hash algorithm field",
        SpecError::UnknownHash => "the symbol uses a hash algorithm this reader does not know",
        SpecError::HeaderOverrun => "the symbol's header is longer than its content",
        SpecError::TooLarge => "the content is larger than this reader accepts",
        SpecError::Malformed => "the symbol's content cannot be decoded",
        SpecError::RecordList => "the symbol's record list is invalid",
        SpecError::ActionRule => "the symbol holds a URL in a place that is not allowed",
        SpecError::DigestMismatch => "the symbol's colour content does not match its digest",
        SpecError::ExtensionUnread => {
            "the symbol holds more content in colour, which this reader does not read"
        }
        SpecError::BootstrapMismatch => "the QR Code beside the symbol does not link to NMT Code",
    }
}

/// The summary `make` writes to standard error.
pub fn made(
    width: u32,
    height: u32,
    level: u8,
    codec: u32,
    used: usize,
    capacity: usize,
) -> String {
    format!(
        "{width} x {height} modules, error-correction level {level}, codec {codec}, \
         {used} of {capacity} bytes used"
    )
}

/// Messages of `make` and `read`.
pub mod msg {
    use std::path::Path;

    /// No subcommand.
    pub const NO_COMMAND: &str = "no command given";
    /// Standard input is not UTF-8 text.
    pub const STDIN_NOT_UTF8: &str =
        "standard input is not UTF-8 text; use --file to encode other data";
    /// A --text value that is not UTF-8.
    pub const TEXT_NOT_UTF8: &str = "the text is not valid UTF-8";
    /// A --url value that is not UTF-8.
    pub const URL_NOT_UTF8: &str = "the URL is not valid UTF-8";
    /// A file name that is not UTF-8.
    pub const FILE_NAME_NOT_UTF8: &str = "the file name is not valid UTF-8";
    /// More than one content option.
    pub const ONE_CONTENT: &str = "give only one of --text, --url and --file";
    /// `--size` with a maximum.
    pub const SIZE_WITH_MAXIMUM: &str =
        "--size cannot be combined with --max-width or --max-height";
    /// Standard output without `--format`.
    pub const FORMAT_FOR_STDOUT: &str =
        "--format png or --format svg is needed to write to standard output";
    /// A quiet zone below 2.
    pub const QUIET_ZONE_TOO_SMALL: &str = "the quiet zone must be at least 2 modules";
    /// `--qr` with `--no-qr`.
    pub const QR_AND_NO_QR: &str = "give only one of --qr and --no-qr";
    /// `read` without a path.
    pub const READ_NEEDS_PATH: &str = "give the PNG file to read, or - for standard input";
    /// `read` with more than one path.
    pub const READ_ONE_PATH: &str = "give only one PNG file";
    /// No symbol in the image.
    pub const NOT_FOUND: &str = "no NMT Code symbol was found in the image";

    /// An unknown command.
    pub fn unknown_command(name: &str) -> String {
        format!("unknown command '{name}'")
    }

    /// An unknown option.
    pub fn unknown_option(name: &str) -> String {
        format!("unknown option '{name}'")
    }

    /// An option without its value.
    pub fn missing_value(name: &str) -> String {
        format!("option '{name}' needs a value")
    }

    /// A flag given a value.
    pub fn unexpected_value(name: &str) -> String {
        format!("option '{name}' takes no value")
    }

    /// An option given twice.
    pub fn repeated(name: &str) -> String {
        format!("option '{name}' is given more than once")
    }

    /// An argument where none is expected.
    pub fn unexpected_argument(value: &str) -> String {
        format!("unexpected argument '{value}'")
    }

    /// A value that does not parse.
    pub fn invalid_value(name: &str, value: &str, expected: &str) -> String {
        format!("invalid value '{value}' for '{name}': expected {expected}")
    }

    /// What `--level` expects.
    pub const EXPECT_LEVEL: &str = "0, 1, 2 or 3";
    /// What `--size` expects.
    pub const EXPECT_SIZE: &str = "WxH, for example 48x32";
    /// What a positive number option expects.
    pub const EXPECT_POSITIVE: &str = "a whole number of 1 or more";
    /// What `--quiet-zone` expects.
    pub const EXPECT_QUIET_ZONE: &str = "a whole number of 2 or more";
    /// What `--profile` expects.
    pub const EXPECT_PROFILE: &str = "screen, print or lowend";
    /// What `--format` expects.
    pub const EXPECT_FORMAT: &str = "png or svg";
    /// What `--codec` expects.
    pub const EXPECT_CODEC: &str =
        "stored, digits, alphanumeric, token, hangul, brotli or a number 0-5";

    /// An output path whose extension picks no format.
    pub fn unknown_extension(path: &Path) -> String {
        format!("cannot tell the format of '{}': use .png, .svg or --format", path.display())
    }

    /// `--format` that disagrees with the extension.
    pub fn format_conflict(path: &Path, format: &str) -> String {
        format!("'{}' does not end in .{format}, which --format asks for", path.display())
    }

    /// A file that cannot be read.
    pub fn cannot_read(path: &Path, error: &std::io::Error) -> String {
        format!("cannot read '{}': {error}", path.display())
    }

    /// Standard input that cannot be read.
    pub fn cannot_read_stdin(error: &std::io::Error) -> String {
        format!("cannot read standard input: {error}")
    }

    /// A file that cannot be written.
    pub fn cannot_write(path: &Path, error: &std::io::Error) -> String {
        format!("cannot write '{}': {error}", path.display())
    }

    /// Standard output that cannot be written.
    pub fn cannot_write_stdout(error: &std::io::Error) -> String {
        format!("cannot write to standard output: {error}")
    }

    /// `--out` that is not a directory.
    pub fn not_a_directory(path: &Path) -> String {
        format!("'{}' is not a directory", path.display())
    }

    /// A file that exists already.
    pub fn exists(path: &Path) -> String {
        format!("not saved: '{}' already exists", path.display())
    }

    /// A saved file.
    pub fn saved(path: &Path, len: usize) -> String {
        format!("saved '{}' ({len} bytes)", path.display())
    }

    /// A file record without `--out`.
    pub fn file_not_saved(name: &str, len: usize) -> String {
        format!("file '{name}' ({len} bytes) not saved; use --out DIR to save it")
    }

    /// A record of unknown type.
    pub fn unknown_type(content_type: u32, len: usize) -> String {
        format!("record of unknown type {content_type} ({len} bytes), shown in hexadecimal")
    }

    /// A value that is not UTF-8.
    pub const NOT_UTF8: &str = "a text record is not valid UTF-8; shown in hexadecimal";
    /// A PSBT record without the PSBT magic.
    pub const NOT_PSBT: &str =
        "a PSBT record does not start with the PSBT bytes; treated as a file";

    /// The name a record is saved under when it has no usable file name: record `record` of the
    /// symbol, and symbol `symbol` when the image has several (both counted from 0).
    pub fn fallback_file_name(symbol: Option<usize>, record: usize) -> String {
        match symbol {
            Some(symbol) => format!("nmtcode-{}-{}.bin", symbol + 1, record + 1),
            None => format!("nmtcode-{}.bin", record + 1),
        }
    }

    /// Symbol `index` of an image that has several.
    pub fn symbol_prefix(index: usize) -> String {
        format!("symbol {}: ", index + 1)
    }
}

/// Why `make` could not encode the content.
pub fn encode_error(error: &nmtcode::EncodeError) -> String {
    use nmtcode::EncodeError;
    match *error {
        EncodeError::ColourNotImplemented => {
            "the color profile is not implemented in this version".to_owned()
        }
        EncodeError::InvalidLevel(level) => format!("level {level} is not 0 to 3"),
        EncodeError::InvalidSize { width, height } => format!(
            "{width}x{height} is not a symbol size: each side is a multiple of 4 from 20 to 4108"
        ),
        EncodeError::Records(error) => write_error(error).to_owned(),
        EncodeError::DoesNotFit { width, height, level, container_len, capacity } => format!(
            "the content needs {container_len} bytes; {width}x{height} at level {level} holds \
             {capacity}"
        ),
        EncodeError::NoSize { level, container_len } => format!(
            "the content needs {container_len} bytes, more than any allowed size holds at level \
             {level}"
        ),
        EncodeError::Internal(what) => format!("internal error: {what}"),
        _ => "the content cannot be encoded".to_owned(),
    }
}

/// Why the records break a rule of the specification.
const fn write_error(error: nmtcode_core::WriteError) -> &'static str {
    use nmtcode_core::WriteError;
    match error {
        WriteError::NoRecords => "there is nothing to encode",
        WriteError::ActionRule => "a symbol holds at most one URL, as its first record",
        WriteError::FileNameWithoutTarget => "a file name must be followed by a file",
        WriteError::ContentTooLarge | WriteError::BodyTooLong => "the content is larger than 1 MiB",
        WriteError::UnknownCodec(_)
        | WriteError::DictionaryNotAllowed
        | WriteError::DecodedLengthMismatch
        | WriteError::CapacityExceeded { .. } => "internal error: the container was refused",
    }
}

/// Why `make` could not draw the symbol.
pub fn render_error(error: &nmtcode_render::RenderError) -> String {
    use nmtcode_render::RenderError;
    match error {
        RenderError::ModuleSizeZero => "the module size must be at least 1 pixel".to_owned(),
        RenderError::QuietZoneTooSmall { .. } => msg::QUIET_ZONE_TOO_SMALL.to_owned(),
        RenderError::InvalidDpi => "the DPI must be at least 1".to_owned(),
        RenderError::TooLarge => {
            "the image would be too large; use a smaller module size".to_owned()
        }
        RenderError::Png(message) => format!("the PNG image could not be written: {message}"),
        _ => "the image could not be drawn".to_owned(),
    }
}

/// Why `read` could not decode the image.
pub fn detect_error(error: &nmtcode_detect::DetectError) -> String {
    use nmtcode_detect::DetectError;
    match error {
        DetectError::NotPng => "the input is not a PNG image".to_owned(),
        DetectError::Malformed(message) => format!("the PNG image is damaged: {message}"),
        DetectError::Unsupported(message) => format!("the PNG image is not supported: {message}"),
        DetectError::TooLarge { width, height } => format!(
            "the image is {width} x {height} pixels, more than the {} pixels accepted",
            nmtcode_detect::MAX_PIXELS
        ),
        _ => "the image could not be read".to_owned(),
    }
}
