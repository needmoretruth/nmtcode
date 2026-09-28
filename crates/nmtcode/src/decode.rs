//! The reader: the encoding order of chapter 1 (1.3) in reverse, with the checks and outcomes of
//! chapter 3 (3.9) and the error names of chapter 9 (9.8).

use core::fmt;

use nmtcode_core::{
    CODEC_COUNT_V0, ContentType, DictionaryEntry, DictionaryKind, FormatWord, MAX_CONTENT_LEN_V0,
    ModuleGrid, Outcome, PresentAs, ReaderConfig, RecordForm, ValueNotice, decode_format,
    parse_message, parse_records, safe_file_name,
};
use nmtcode_ecc::EccError;
use nmtcode_payload::CodecError;
use nmtcode_symbol::{Layout, read_format_copies};

use crate::SpecError;

/// What [`decode`] accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    /// The reader's own limit `LIMIT` on the decoded length L in bytes (chapter 6, 6.4). A
    /// container that declares more is refused with `E_TOO_LARGE` before anything is allocated.
    /// Values above `MAX_CONTENT_LEN_V0` (16 MiB) act as 16 MiB. The default is 16 MiB.
    pub limit: u32,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self { limit: MAX_CONTENT_LEN_V0 }
    }
}

/// Why [`decode`] presents nothing: one reader error of chapter 9 (9.8).
///
/// [`DecodeError::name`] is the stable name, for example `"E_CRC_MISMATCH"`, and
/// [`DecodeError::outcome`] the outcome class of chapter 3 (3.9): damaged, unsupported or
/// malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecodeError {
    error: SpecError,
}

impl DecodeError {
    /// The reader error.
    pub const fn error(self) -> SpecError {
        self.error
    }

    /// The stable name of chapter 9 (9.8).
    pub const fn name(self) -> &'static str {
        self.error.name()
    }

    /// The outcome class of chapter 3 (3.9).
    pub const fn outcome(self) -> Outcome {
        self.error.outcome()
    }
}

impl From<SpecError> for DecodeError {
    fn from(error: SpecError) -> Self {
        Self { error }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outcome = match self.outcome() {
            Outcome::Damaged => "damaged",
            Outcome::Unsupported => "unsupported",
            Outcome::Malformed => "malformed",
            Outcome::Presented | Outcome::PresentedBaseOnly | Outcome::ErrorReported => "error",
        };
        write!(f, "{} ({outcome})", self.name())
    }
}

impl std::error::Error for DecodeError {}

/// One record as the reader presents it (chapter 3, 3.4.3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DecodedRecord {
    /// The content type ID as written.
    pub content_type: ContentType,
    /// The value.
    pub value: Vec<u8>,
    /// How to present it.
    pub present_as: PresentAs,
    /// Set when the value failed its type's check and is presented otherwise.
    pub notice: Option<ValueNotice>,
}

impl DecodedRecord {
    /// The value as text, for a record presented as text, a URL or a file name.
    pub fn text(&self) -> Option<&str> {
        match self.present_as {
            PresentAs::Text | PresentAs::Url | PresentAs::FileName => {
                core::str::from_utf8(&self.value).ok()
            }
            PresentAs::Bytes | PresentAs::File | PresentAs::Psbt | PresentAs::Unknown => None,
        }
    }
}

/// The content of a symbol that passed every check of chapters 2 to 6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The base records in presentation order (chapter 3, 3.4.4 rule 4).
    pub records: Vec<DecodedRecord>,
    /// [`Outcome::Presented`], or [`Outcome::PresentedBaseOnly`] for a colour symbol whose
    /// extension records this reader does not read (chapter 3, 3.5 rule 2).
    pub outcome: Outcome,
    /// `Some(E_EXTENSION_UNREAD)` with [`Outcome::PresentedBaseOnly`]: the reader must state
    /// that the symbol holds further content it could not read.
    pub notice: Option<SpecError>,
    /// The format word the symbol was read with (chapter 2).
    pub format: FormatWord,
    /// The codec ID of the container (chapter 6, 6.2).
    pub codec: u32,
    /// The dictionary ID of the container, 0 when none (chapter 6, 6.11).
    pub dictionary: u32,
    /// The record form of the container (chapter 3, 3.2.5).
    pub record_form: RecordForm,
    /// The number of codeword bytes that Reed-Solomon decoding changed (chapter 4, 4.9).
    pub corrected: usize,
}

impl Decoded {
    /// The name to offer when saving record `index`: the value of the file name record directly
    /// before it, made safe by the rule of chapter 3 (3.4.3), when that record is presented as
    /// a file name. `None` otherwise, or when the safe name is empty.
    pub fn file_name_for(&self, index: usize) -> Option<String> {
        let previous = self.records.get(index.checked_sub(1)?)?;
        if previous.present_as != PresentAs::FileName {
            return None;
        }
        safe_file_name(&previous.value).filter(|name| !name.is_empty())
    }
}

/// The reader configuration: every codec this build decodes, the dictionaries it carries and
/// its limit.
fn reader_config(limit: u32, dictionaries: &[DictionaryEntry]) -> ReaderConfig<'_> {
    let mut codecs = [false; CODEC_COUNT_V0];
    for (id, implemented) in (0u32..).zip(codecs.iter_mut()) {
        *implemented = nmtcode_payload::is_supported(id);
    }
    ReaderConfig { codecs, limit: limit.min(MAX_CONTENT_LEN_V0), dictionaries }
}

/// The dictionaries of the payload crate's registry, in the terms of the container checks.
fn carried_dictionaries() -> Vec<DictionaryEntry> {
    nmtcode_payload::dictionary::REGISTRY
        .iter()
        .map(|entry| DictionaryEntry {
            id: entry.id,
            kind: match entry.kind {
                nmtcode_payload::dictionary::DictionaryKind::BrotliPrefix => {
                    DictionaryKind::BrotliPrefix
                }
                nmtcode_payload::dictionary::DictionaryKind::ShortTextModel => {
                    DictionaryKind::ShortTextModel
                }
            },
        })
        .collect()
}

const fn ecc_error(error: EccError) -> SpecError {
    match error {
        EccError::LayerTooSmall => SpecError::LayerTooSmall,
        // Every other error means the blocks could not be corrected as the format word says.
        EccError::Uncorrectable
        | EccError::InvalidLevel
        | EccError::InvalidParity
        | EccError::InvalidBlockLength
        | EccError::MessageLength
        | EccError::StreamLength
        | EccError::ErasureOutOfRange => SpecError::EccFailed,
    }
}

const fn codec_error(error: CodecError) -> SpecError {
    match error {
        CodecError::UnsupportedCodec => SpecError::UnsupportedCodec,
        CodecError::UnknownDictionary => SpecError::UnknownDictionary,
        CodecError::DictionaryMismatch => SpecError::DictionaryMismatch,
        CodecError::TooLarge => SpecError::TooLarge,
        CodecError::Malformed => SpecError::Malformed,
    }
}

/// Reads the base layer of `grid` with format word `word`: chapters 5, 4, 3 and 6 in that order.
fn decode_with(
    grid: &ModuleGrid,
    word: &FormatWord,
    config: &ReaderConfig<'_>,
) -> Result<Decoded, SpecError> {
    let layout =
        Layout::new(word.width(), word.height()).map_err(|_| SpecError::FormatUnreadable)?;
    let stream = layout.read_stream(grid).map_err(|_| SpecError::FormatUnreadable)?;
    let split = nmtcode_ecc::split(layout.codeword_count(), word.level()).map_err(ecc_error)?;
    let corrected = nmtcode_ecc::decode_stream(&split, &stream, &[]).map_err(ecc_error)?;
    let parsed = parse_message(&corrected.message, word, config)?;
    let fields = parsed.header.fields;
    let content = nmtcode_payload::decode(
        fields.codec,
        fields.dictionary,
        fields.decoded_len,
        parsed.coded,
        config.limit,
    )
    .map_err(codec_error)?;
    let records = parse_records(&parsed.header, &content)?;
    Ok(Decoded {
        outcome: records.outcome(),
        notice: records.notice(),
        records: records
            .records
            .iter()
            .map(|record| DecodedRecord {
                content_type: record.content_type,
                value: record.value.to_vec(),
                present_as: record.present_as,
                notice: record.notice,
            })
            .collect(),
        format: *word,
        codec: fields.codec,
        dictionary: fields.dictionary,
        record_form: fields.form,
        corrected: corrected.corrected,
    })
}

/// Decodes the symbol in `grid` (quiet zone excluded, dark = `true`) by the encoding order of
/// chapter 1 (1.3) in reverse.
///
/// 1. Both format copies are sampled and decoded with up to 3 bit errors each (chapter 2, 2.7).
///    The chosen word must give the grid's width and height.
/// 2. The codeword stream is read from the data modules and unwhitened (chapter 5).
/// 3. Every Reed-Solomon block is corrected (chapter 4, 4.9).
/// 4. The container is checked: body length, CRC-32C, then every header field (chapter 3, 3.9).
/// 5. The codec decodes exactly L bytes within [`DecodeOptions::limit`] (chapter 6, 6.4).
/// 6. The records are read and checked (chapter 3, 3.2.5, 3.4.4).
///
/// When the two format copies decode to different words and the base layer fails under the
/// chosen one, the other word is tried (2.7 step 4); if both fail, the error of the chosen word
/// is returned. Nothing is returned from a symbol that failed a check.
///
/// # Errors
///
/// [`DecodeError`] with the error of chapter 9 (9.8) of the first check that failed. A grid
/// whose size differs from the size in its format word gives `E_FORMAT_UNREADABLE`.
pub fn decode(grid: &ModuleGrid, options: &DecodeOptions) -> Result<Decoded, DecodeError> {
    let copies = read_format_copies(grid).map_err(|_| SpecError::FormatUnreadable)?;
    let format = decode_format(&copies)?;
    let dictionaries = carried_dictionaries();
    let config = reader_config(options.limit, &dictionaries);
    let fits = |word: &FormatWord| word.width() == grid.width() && word.height() == grid.height();
    let mut words = [Some(format.word), format.alternative].into_iter().flatten().filter(fits);
    let first = words.next().ok_or(SpecError::FormatUnreadable)?;
    match decode_with(grid, &first, &config) {
        Ok(decoded) => Ok(decoded),
        Err(error) => match words.next() {
            Some(other) => decode_with(grid, &other, &config).map_err(|_| error.into()),
            None => Err(error.into()),
        },
    }
}
