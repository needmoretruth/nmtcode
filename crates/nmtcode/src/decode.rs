//! The reader: the encoding order of chapter 1 (1.3) in reverse, with the checks and outcomes of
//! chapter 3 (3.9) and the error names of chapter 9 (9.8).

use core::fmt;

use nmtcode_core::{
    CODEC_COUNT_V0, ContentType, DictionaryEntry, DictionaryKind, FORMAT_MAX_ERASURES,
    FormatSample, FormatWord, MAX_SIDE, MAX_STATIC_CONTENT_LEN_V0, ModuleGrid, Outcome, PresentAs,
    ReaderConfig, RecordForm, ValueNotice, decode_format_with, parse_message, parse_records,
    safe_file_name,
};
use nmtcode_ecc::EccError;
use nmtcode_payload::CodecError;
use nmtcode_symbol::{
    FORMAT_BITS, FormatCopy, Layout, ModuleClass, format_module, read_format_copies,
};

use crate::SpecError;

/// The area of the largest symbol, 4108 × 4108 modules: the default of
/// [`DecodeOptions::max_area`], which accepts every valid size.
pub const MAX_AREA: u64 = MAX_SIDE as u64 * MAX_SIDE as u64;

/// What [`decode`] accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeOptions {
    /// The reader's own limit `LIMIT` on the decoded length L in bytes (chapter 6, 6.4). A
    /// container that declares more is refused with `E_TOO_LARGE` before anything is allocated.
    /// Values above `MAX_STATIC_CONTENT_LEN_V0` (1 MiB, the cap of a static symbol) act as
    /// 1 MiB. The default is 1 MiB.
    pub limit: u32,
    /// The largest area W × H, in modules, that this reader decodes (chapter 2, 2.7 step 5;
    /// chapter 5, 5.11). A format word with a larger area rejects the symbol with
    /// `E_SIZE_LIMIT` before any data module is read. The default, [`MAX_AREA`], accepts every
    /// valid size.
    pub max_area: u64,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self { limit: MAX_STATIC_CONTENT_LEN_V0, max_area: MAX_AREA }
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
            Outcome::Presented | Outcome::PresentedBaseOnly | Outcome::PresentedWithError => {
                "error"
            }
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
    ReaderConfig { codecs, limit: limit.min(MAX_STATIC_CONTENT_LEN_V0), dictionaries }
}

/// The least number of colour codewords `N_c` of a colour-profile-1 symbol (chapter 7, 7.8.2).
const COLOUR_MIN_CODEWORDS: usize = 16;

/// Reference cells per copy, times c² (chapter 7, 7.5).
const REFERENCE_MODULES_PER_COPY: usize = 32;

/// The number of colour codewords `N_c` of `layout` with chroma cells of `c` × `c` modules
/// (chapter 7, 7.5 and 7.8.2): the cells aligned to multiples of c whose modules are all data
/// modules, less the two reference copies, in whole bytes.
fn colour_codewords(layout: &Layout, c: u32) -> usize {
    let cells = (0..layout.height() / c)
        .flat_map(|j| (0..layout.width() / c).map(move |i| (i, j)))
        .filter(|&(i, j)| {
            (0..c).all(|dy| {
                (0..c).all(|dx| {
                    layout.module_class(c * i + dx, c * j + dy) == Some(ModuleClass::Data)
                })
            })
        })
        .count();
    let side = usize::try_from(c).unwrap_or(usize::MAX);
    let reference = 2 * (REFERENCE_MODULES_PER_COPY / (side * side).max(1));
    cells.saturating_sub(reference) / 8
}

/// Step 3 of 2.7 for a word that has valid fields: it must give the grid's size, and a
/// colour-profile-1 word must leave at least 16 colour codewords (chapter 7, 7.8.2).
///
/// The grid is the reader's own measurement of the symbol: a detector such as
/// `nmtcode-detect` chooses its width and height from the finders it found, within the
/// tolerance of chapter 5 (5.11). A word whose W and H differ from the grid therefore disagrees
/// with the finder geometry, and its copy is not decoded.
fn plausible(word: &FormatWord, grid: &ModuleGrid) -> bool {
    if word.width() != grid.width() || word.height() != grid.height() {
        return false;
    }
    if word.colour_profile() == 0 {
        return true;
    }
    Layout::new(word.width(), word.height()).is_ok_and(|layout| {
        colour_codewords(&layout, word.chroma_cell_side()) >= COLOUR_MIN_CODEWORDS
    })
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
        // Every allowed size has at least 20 base-layer codewords (chapter 4, 4.6), so a layer
        // that is too small, like every other error, means the blocks could not be corrected
        // as the format word says.
        EccError::LayerTooSmall
        | EccError::Uncorrectable
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
    decode_layer(grid, word, config, &[])
}

/// Every Reed-Solomon block of `stream` corrected (chapter 4, 4.9). `erasures[b]` lists the
/// byte positions of block b that the reader marked as unreliable, most doubtful first; each
/// block is tried with all its marks up to its parity count, then with the first half, then
/// without, and keeps the first that corrects (4.9 permits several erasure sets per block).
fn correct_blocks(
    split: &nmtcode_ecc::BlockSplit,
    stream: &[u8],
    erasures: &[Vec<usize>],
) -> Result<nmtcode_ecc::Decoded, SpecError> {
    if erasures.iter().all(Vec::is_empty) {
        return nmtcode_ecc::decode_stream(split, stream, &[]).map_err(ecc_error);
    }
    let mut message = Vec::with_capacity(split.capacity());
    let mut corrected = 0usize;
    for block in split.blocks() {
        let received: Vec<u8> = stream
            .iter()
            .skip(block.index)
            .step_by(split.block_count())
            .take(block.len)
            .copied()
            .collect();
        if received.len() != block.len {
            return Err(SpecError::EccFailed);
        }
        let marks = erasures.get(block.index).map_or(&[][..], Vec::as_slice);
        let full = marks.get(..marks.len().min(block.parity_len)).unwrap_or(&[]);
        let half = full.get(..full.len() / 2).unwrap_or(&[]);
        let mut attempts: Vec<&[usize]> = vec![full];
        if !half.is_empty() && half.len() < full.len() {
            attempts.push(half);
        }
        if !full.is_empty() {
            attempts.push(&[]);
        }
        let (codeword, changed) = attempts
            .iter()
            .find_map(|&attempt| {
                let mut codeword = received.clone();
                nmtcode_ecc::rs::decode(&mut codeword, block.parity_len, attempt)
                    .ok()
                    .map(|changed| (codeword, changed))
            })
            .ok_or(SpecError::EccFailed)?;
        corrected += changed;
        message.extend_from_slice(codeword.get(..block.message_len).ok_or(SpecError::EccFailed)?);
    }
    Ok(nmtcode_ecc::Decoded { message, corrected })
}

/// [`decode_with`] with `erasures` per block for the Reed-Solomon step.
fn decode_layer(
    grid: &ModuleGrid,
    word: &FormatWord,
    config: &ReaderConfig<'_>,
    erasures: &[Vec<usize>],
) -> Result<Decoded, SpecError> {
    let layout =
        Layout::new(word.width(), word.height()).map_err(|_| SpecError::FormatUnreadable)?;
    let stream = layout.read_stream(grid).map_err(|_| SpecError::FormatUnreadable)?;
    let split = nmtcode_ecc::split(layout.codeword_count(), word.level()).map_err(ecc_error)?;
    let corrected = correct_blocks(&split, &stream, erasures)?;
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
/// 1. Both format copies are sampled, unmasked with their own masks and decoded with up to 3
///    bit errors each (chapter 2, 2.7). A copy whose word does not give the grid's width and
///    height, or leaves a colour layer below 16 codewords, is not decoded. Two decoded copies
///    with different words reject the symbol, and so does an area above
///    [`DecodeOptions::max_area`].
/// 2. The codeword stream is read from the data modules and unwhitened (chapter 5).
/// 3. Every Reed-Solomon block is corrected (chapter 4, 4.9).
/// 4. The container is checked: body length, CRC-32C, then every header field (chapter 3, 3.9).
/// 5. The codec decodes exactly L bytes within [`DecodeOptions::limit`] (chapter 6, 6.4).
/// 6. The records are read and checked (chapter 3, 3.2.5, 3.4.4).
///
/// Nothing is returned from a symbol that failed a check.
///
/// # Errors
///
/// [`DecodeError`] with the error of chapter 9 (9.8) of the first check that failed. A grid
/// whose size differs from the size in every copy of its format word gives
/// `E_FORMAT_UNREADABLE`, and a format word whose area exceeds [`DecodeOptions::max_area`]
/// gives `E_SIZE_LIMIT`.
pub fn decode(grid: &ModuleGrid, options: &DecodeOptions) -> Result<Decoded, DecodeError> {
    let copies = read_format_copies(grid).map_err(|_| SpecError::FormatUnreadable)?;
    let samples = copies.map(|bits| Some(FormatSample::new(bits)));
    let format = decode_format_with(samples, |word| plausible(word, grid))?;
    // 2.7 step 5: the reader's largest area.
    if u64::from(format.word.width()) * u64::from(format.word.height()) > options.max_area {
        return Err(SpecError::SizeLimit.into());
    }
    let dictionaries = carried_dictionaries();
    let config = reader_config(options.limit, &dictionaries);
    decode_with(grid, &format.word, &config).map_err(DecodeError::from)
}

/// Decodes the symbol in `grid` as [`decode`] does, with `uncertain`: the modules whose value
/// the reader could not tell with confidence, as (x, y), most doubtful first, for example
/// `nmtcode_detect::Found::uncertain`.
///
/// 1. Format word (chapter 2, 2.7 step 1): the first 4 uncertain modules of each copy are
///    erased bits of that copy.
/// 2. Base layer (chapter 5, 5.9; chapter 4, 4.9): a data module at placement index k marks
///    codeword floor(k / 8) as an erasure. Each Reed-Solomon block is corrected with its marks,
///    at most its parity count, the most doubtful first; when that fails, with the first half
///    of them; then without. Every result is bounded-distance (2e + s ≤ P).
/// 3. When this reading fails a check, the grid is decoded again without any erasure, exactly
///    as [`decode`], and that result is returned. A reading with erasures in which both format
///    copies decode, to different words, rejects the symbol with `E_FORMAT_CONFLICT`, and one
///    whose chosen word has format version 1 to 3 rejects it with `E_FORMAT_VERSION`, whatever
///    the plain reading gives.
///
/// At most two messages reach the CRC-32C check, far below the 256 of chapter 4 (4.9). Nothing
/// is returned from a reading that failed a check. With an empty `uncertain` this is
/// [`decode`].
///
/// # Errors
///
/// As [`decode`].
pub fn decode_with_erasures(
    grid: &ModuleGrid,
    uncertain: &[(u32, u32)],
    options: &DecodeOptions,
) -> Result<Decoded, DecodeError> {
    if uncertain.is_empty() {
        return decode(grid, options);
    }
    match erased_reading(grid, uncertain, options) {
        Ok(decoded) => Ok(decoded),
        Err(error @ (SpecError::FormatConflict | SpecError::FormatVersion)) => Err(error.into()),
        Err(_) => decode(grid, options),
    }
}

/// The reading of [`decode_with_erasures`] with the erasures, without the fallback.
fn erased_reading(
    grid: &ModuleGrid,
    uncertain: &[(u32, u32)],
    options: &DecodeOptions,
) -> Result<Decoded, SpecError> {
    let copies = read_format_copies(grid).map_err(|_| SpecError::FormatUnreadable)?;
    let mut erased = [0u64; 2];
    let mut counts = [0u32; 2];
    for &(x, y) in uncertain {
        for (copy, (mask, count)) in
            FormatCopy::ALL.iter().zip(erased.iter_mut().zip(counts.iter_mut()))
        {
            if *count >= FORMAT_MAX_ERASURES {
                continue;
            }
            let index = (0..FORMAT_BITS)
                .find(|&i| format_module(*copy, i, grid.width(), grid.height()) == Some((x, y)));
            if let Some(i) = index {
                *mask |= 1u64 << (FORMAT_BITS - 1 - i);
                *count += 1;
            }
        }
    }
    let samples = [0, 1].map(|c| {
        Some(FormatSample::with_erasures(
            copies.get(c).copied().unwrap_or(0),
            erased.get(c).copied().unwrap_or(0),
        ))
    });
    let format = decode_format_with(samples, |word| plausible(word, grid))?;
    if u64::from(format.word.width()) * u64::from(format.word.height()) > options.max_area {
        return Err(SpecError::SizeLimit);
    }
    let layout = Layout::new(format.word.width(), format.word.height())
        .map_err(|_| SpecError::FormatUnreadable)?;
    let split =
        nmtcode_ecc::split(layout.codeword_count(), format.word.level()).map_err(ecc_error)?;
    let mut per_block: Vec<Vec<usize>> = vec![Vec::new(); split.block_count()];
    let codeword_modules = 8 * layout.codeword_count();
    for &(x, y) in uncertain {
        let Some(k) = layout.placement_index(x, y) else { continue };
        if k >= codeword_modules {
            continue;
        }
        let Some((block, byte)) = split.locate(k / 8) else { continue };
        if let Some(list) = per_block.get_mut(block)
            && !list.contains(&byte)
        {
            list.push(byte);
        }
    }
    let dictionaries = carried_dictionaries();
    let config = reader_config(options.limit, &dictionaries);
    decode_layer(grid, &format.word, &config, &per_block)
}
