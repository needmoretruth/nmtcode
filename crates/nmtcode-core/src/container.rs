//! The container of chapter 3: the static container with its format echo byte (3.2),
//! transfer-tile detection (3.3), the colour-extension fields a black-and-white reader reads
//! (3.5), hash algorithm IDs (3.6), the CRC-32C (3.7), padding (3.8) and the reader outcomes
//! (3.9).
//!
//! A generator calls [`StaticFields::container_len`] to compare codec candidates (chapter 6,
//! 6.3), [`StaticFields::write`] to serialise the chosen one and [`pad_message`] to fill the
//! message capacity K. A reader calls [`parse_message`] on the K-byte message, hands the coded
//! field to the codec, and gives the decoded content to [`crate::parse_records`].

use alloc::vec::Vec;

use crate::format::{FormatEcho, FormatWord, SymbolClass};
use crate::leb128::{Leb128Error, leb128_len, read_leb128, write_leb128};
use crate::record::{ContentType, RecordForm};
use crate::{CRC32C_LEN, Error, crc32c};

/// `MAX_CONTENT_LEN_V0` of chapter 6 (6.4): the largest decoded length L of one container in
/// format version 0 (16 MiB).
pub const MAX_CONTENT_LEN_V0: u32 = 16_777_216;
/// `MAX_STATIC_CONTENT_LEN_V0` of chapter 6 (6.4): the largest decoded length L of the
/// container of a static symbol (symbol class 0) in format version 0 (1 MiB).
pub const MAX_STATIC_CONTENT_LEN_V0: u32 = 1_048_576;
/// The container version of this chapter, bits 7–6 of the lead byte (3.2.2).
pub const CONTAINER_VERSION: u8 = 0;
/// The value of the lead byte's codec field that says the codec ID is in field 4 (3.2.2).
pub const CODEC_ESCAPE: u32 = 15;
/// The number of codec IDs, 0 to 5, that the registry of this version assigns (chapter 6, 6.2).
pub const CODEC_COUNT_V0: usize = 6;
/// The padding pattern: pad byte i is `EC` when i is even and `11` when i is odd (3.8).
pub const PADDING_PATTERN: [u8; 2] = [0xEC, 0x11];
/// The length of a SHA-256 digest, the only digest of this version (3.6).
pub const SHA256_LEN: usize = 32;

const LEAD_RECORD_LIST: u8 = 0x20;
/// X, the colour-extension bit of the lead byte (3.2.2).
const LEAD_COLOUR: u8 = 0x10;
const LEAD_CODEC: u8 = 0x0F;
const LEAD_TILE_RESERVED: u8 = 0x3F;

/// Whether codec `codec` of the registry of this version takes a dictionary ID (chapter 6, 6.2):
/// `Some(true)` for codecs 3 and 5, `Some(false)` for codecs 0, 1, 2 and 4, `None` for an ID
/// this version does not assign.
pub const fn codec_takes_dictionary(codec: u32) -> Option<bool> {
    match codec {
        0 | 1 | 2 | 4 => Some(false),
        3 | 5 => Some(true),
        _ => None,
    }
}

/// A hash algorithm of the registry of 3.6 that this version defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// ID 1: SHA-256, 32 bytes.
    Sha256,
}

impl HashAlgorithm {
    /// The registry ID.
    pub const fn id(self) -> u32 {
        match self {
            Self::Sha256 => 1,
        }
    }

    /// The digest length in bytes.
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Sha256 => SHA256_LEN,
        }
    }

    /// The algorithm of registry ID `id`.
    ///
    /// # Errors
    ///
    /// [`Error::HashIdInvalid`] for IDs 0 and 3 to 6, which are never assignable;
    /// [`Error::UnknownHash`] for ID 2 (not defined in this version) and IDs 7 and up.
    pub const fn from_id(id: u32) -> Result<Self, Error> {
        match id {
            1 => Ok(Self::Sha256),
            0 | 3..=6 => Err(Error::HashIdInvalid),
            _ => Err(Error::UnknownHash),
        }
    }
}

/// Fields 7 and 8 of a base container with X = 1 (3.2.1, 3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtensionDigest {
    /// The hash algorithm (field 7).
    pub algorithm: HashAlgorithm,
    /// The digest over the canonical base and extension records (field 8, 3.5).
    pub digest: [u8; SHA256_LEN],
}

/// The kind of a registered dictionary, which fixes the codec it is used with (chapter 6, 6.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DictionaryKind {
    /// "brotli prefix", for codec 5.
    BrotliPrefix,
    /// "short-text model", for codec 3.
    ShortTextModel,
}

impl DictionaryKind {
    /// The codec ID this kind of dictionary is used with.
    pub const fn codec(self) -> u32 {
        match self {
            Self::BrotliPrefix => 5,
            Self::ShortTextModel => 3,
        }
    }
}

/// A registered dictionary that a reader carries, besides ID 0 (chapter 6, 6.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DictionaryEntry {
    /// The dictionary ID.
    pub id: u32,
    /// Its registered kind.
    pub kind: DictionaryKind,
}

/// What a reader implements, for the checks of 3.2.4 and 3.9 that depend on the reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReaderConfig<'a> {
    /// `codecs[i]` is true when the reader implements codec ID i. Codec IDs from
    /// [`CODEC_COUNT_V0`] up are not assigned in this version and are always unsupported.
    pub codecs: [bool; CODEC_COUNT_V0],
    /// The reader's own limit `LIMIT` on the decoded length (chapter 6, 6.4). Values above
    /// [`MAX_STATIC_CONTENT_LEN_V0`] act as that cap for a static container.
    pub limit: u32,
    /// The registered dictionaries the reader carries besides ID 0. The registry of this
    /// version has none; an application that carries a private-use dictionary lists it here.
    pub dictionaries: &'a [DictionaryEntry],
}

impl ReaderConfig<'static> {
    /// Every codec of this version, `LIMIT` = [`MAX_STATIC_CONTENT_LEN_V0`], no dictionary but
    /// ID 0.
    pub const DEFAULT: Self = Self {
        codecs: [true; CODEC_COUNT_V0],
        limit: MAX_STATIC_CONTENT_LEN_V0,
        dictionaries: &[],
    };
}

impl Default for ReaderConfig<'static> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ReaderConfig<'_> {
    /// Whether the reader implements `codec`, and if so whether that codec takes a dictionary.
    fn codec(&self, codec: u32) -> Result<bool, Error> {
        let implemented = usize::try_from(codec)
            .ok()
            .and_then(|index| self.codecs.get(index))
            .copied()
            .unwrap_or(false);
        match codec_takes_dictionary(codec) {
            Some(takes) if implemented => Ok(takes),
            _ => Err(Error::UnsupportedCodec),
        }
    }

    /// The checks of dictionary ID `id` for `codec` (3.2.4; chapter 6, 6.2 and 6.11).
    fn check_dictionary(&self, codec: u32, id: u32) -> Result<(), Error> {
        if id == 0 {
            return Ok(());
        }
        let entry = self
            .dictionaries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(Error::UnknownDictionary)?;
        if entry.kind.codec() == codec { Ok(()) } else { Err(Error::DictionaryMismatch) }
    }

    /// The checks of the decoded length of a static container before allocation (3.2.4;
    /// chapter 6, 6.4).
    const fn check_decoded_len(&self, len: u32) -> Result<(), Error> {
        if len > MAX_STATIC_CONTENT_LEN_V0 {
            Err(Error::Malformed)
        } else if len > self.limit {
            Err(Error::TooLarge)
        } else {
            Ok(())
        }
    }
}

/// Why a container or record list could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WriteError {
    /// No records, or record-list form with a count of 0 (3.2.5, 3.5).
    NoRecords,
    /// More than one action record, or an action record that is not the first (3.4.4 rules 1
    /// and 2).
    ActionRule,
    /// A file name record not directly followed by a record of type 0, 4, 6, 7 or 8 (3.4.4
    /// rule 3).
    FileNameWithoutTarget,
    /// The decoded content is longer than [`MAX_STATIC_CONTENT_LEN_V0`] (3.2.4; chapter 6, 6.4).
    ContentTooLarge,
    /// The codec ID is not assigned in this version (chapter 6, 6.2).
    UnknownCodec(u32),
    /// A dictionary ID other than 0 for a codec that takes no dictionary (3.2.4).
    DictionaryNotAllowed,
    /// Codec 0 with a decoded length other than the coded length (3.2.4).
    DecodedLengthMismatch,
    /// The body length would be 2^32 or more (3.2.3).
    BodyTooLong,
    /// The container does not fit the message capacity (3.2.3).
    CapacityExceeded {
        /// The container length, CRC-32C included.
        len: usize,
        /// The message capacity K.
        capacity: usize,
    },
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoRecords => f.write_str("a container needs at least one record"),
            Self::ActionRule => {
                f.write_str("at most one action record, and only as the first record")
            }
            Self::FileNameWithoutTarget => {
                f.write_str("a file name record must be followed by a file record")
            }
            Self::ContentTooLarge => f.write_str("the decoded content exceeds 1 MiB"),
            Self::UnknownCodec(codec) => write!(f, "codec ID {codec} is not assigned"),
            Self::DictionaryNotAllowed => f.write_str("this codec takes no dictionary ID"),
            Self::DecodedLengthMismatch => {
                f.write_str("codec 0 needs a decoded length equal to the coded length")
            }
            Self::BodyTooLong => f.write_str("the body length is 2^32 or more"),
            Self::CapacityExceeded { len, capacity } => {
                write!(f, "a {len}-byte container does not fit {capacity} bytes")
            }
        }
    }
}

impl core::error::Error for WriteError {}

/// The header fields of a static container without colour extension (3.2.1): what a generator
/// writes and what a reader hands to the codec.
///
/// `codec`, `dictionary` and `decoded_len` are the values c, d and L of chapter 6 (6.1): the
/// dictionary is 0 for a codec that takes none, and for codec 0 the decoded length equals the
/// coded length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticFields {
    /// Single-record form with its content type (field 9), or record-list form with its record
    /// count (field 10).
    pub form: RecordForm,
    /// The codec ID (lead byte, or field 4 for 15 and up).
    pub codec: u32,
    /// The dictionary ID (field 5), 0 when the codec takes none.
    pub dictionary: u32,
    /// The decoded length L (field 6 when the codec is not 0).
    pub decoded_len: u32,
}

impl StaticFields {
    /// The fields of a codec 0 (stored) container whose content is `coded_len` bytes.
    ///
    /// # Errors
    ///
    /// [`WriteError::ContentTooLarge`] when `coded_len` exceeds [`MAX_STATIC_CONTENT_LEN_V0`].
    pub fn stored(form: RecordForm, coded_len: usize) -> Result<Self, WriteError> {
        let decoded_len = u32::try_from(coded_len)
            .ok()
            .filter(|&len| len <= MAX_STATIC_CONTENT_LEN_V0)
            .ok_or(WriteError::ContentTooLarge)?;
        Ok(Self { form, codec: 0, dictionary: 0, decoded_len })
    }

    /// The checks a generator applies, and whether the codec takes a dictionary.
    fn check(&self, coded_len: usize) -> Result<bool, WriteError> {
        let takes_dictionary =
            codec_takes_dictionary(self.codec).ok_or(WriteError::UnknownCodec(self.codec))?;
        if !takes_dictionary && self.dictionary != 0 {
            return Err(WriteError::DictionaryNotAllowed);
        }
        if self.codec == 0 && usize::try_from(self.decoded_len).ok() != Some(coded_len) {
            return Err(WriteError::DecodedLengthMismatch);
        }
        if self.decoded_len > MAX_STATIC_CONTENT_LEN_V0 {
            return Err(WriteError::ContentTooLarge);
        }
        if self.form == RecordForm::List(0) {
            return Err(WriteError::NoRecords);
        }
        Ok(takes_dictionary)
    }

    /// The body length Lb for a coded field of `coded_len` bytes, and whether the codec takes a
    /// dictionary.
    fn body_len(&self, coded_len: usize) -> Result<(u32, bool), WriteError> {
        let takes_dictionary = self.check(coded_len)?;
        let dictionary_len = if takes_dictionary { leb128_len(self.dictionary) } else { 0 };
        let decoded_len_len = if self.codec == 0 { 0 } else { leb128_len(self.decoded_len) };
        let form_len = match self.form {
            RecordForm::Single(content_type) => leb128_len(content_type.0),
            RecordForm::List(count) => leb128_len(count),
        };
        let body = (dictionary_len + decoded_len_len + form_len)
            .checked_add(coded_len)
            .and_then(|len| u32::try_from(len).ok())
            .ok_or(WriteError::BodyTooLong)?;
        Ok((body, takes_dictionary))
    }

    /// The number of header bytes: the lead byte, the format echo byte, Lb and fields 4 to 10
    /// (3.2.6).
    ///
    /// # Errors
    ///
    /// As [`StaticFields::write`].
    pub fn header_len(&self, coded_len: usize) -> Result<usize, WriteError> {
        let len = self.container_len(coded_len)?;
        Ok(len.saturating_sub(coded_len).saturating_sub(CRC32C_LEN))
    }

    /// The length of the serialised container for a coded field of `coded_len` bytes, CRC-32C
    /// included and padding excluded: the size the codec selection rule compares (chapter 6,
    /// 6.3). Equal to `self.write(coded)?.len()` without building it.
    ///
    /// # Errors
    ///
    /// As [`StaticFields::write`].
    pub fn container_len(&self, coded_len: usize) -> Result<usize, WriteError> {
        let (body, _) = self.body_len(coded_len)?;
        let body_bytes = usize::try_from(body).map_err(|_| WriteError::BodyTooLong)?;
        STATIC_PREFIX_LEN
            .checked_add(leb128_len(body))
            .and_then(|len| len.checked_add(body_bytes))
            .and_then(|len| len.checked_add(CRC32C_LEN))
            .ok_or(WriteError::BodyTooLong)
    }

    /// Serialises the static container: lead byte, the format echo byte `echo`, Lb, fields 4 to
    /// 10, the coded field and the CRC-32C (3.2.1, 3.7), without padding. The colour-extension
    /// bit X is 0.
    ///
    /// # Errors
    ///
    /// - [`WriteError::UnknownCodec`] for a codec ID that this version does not assign.
    /// - [`WriteError::DictionaryNotAllowed`] for a dictionary ID other than 0 with a codec that
    ///   takes none.
    /// - [`WriteError::DecodedLengthMismatch`] for codec 0 when `decoded_len` differs from
    ///   `coded.len()`.
    /// - [`WriteError::ContentTooLarge`] when `decoded_len` exceeds
    ///   [`MAX_STATIC_CONTENT_LEN_V0`].
    /// - [`WriteError::NoRecords`] for record-list form with a count of 0.
    /// - [`WriteError::BodyTooLong`] when Lb would be 2^32 or more.
    pub fn write(&self, echo: FormatEcho, coded: &[u8]) -> Result<Vec<u8>, WriteError> {
        let (body, takes_dictionary) = self.body_len(coded.len())?;
        let len = self.container_len(coded.len())?;
        let codec_bits =
            u8::try_from(self.codec).map_err(|_| WriteError::UnknownCodec(self.codec))?;
        let form_bit = match self.form {
            RecordForm::Single(_) => 0,
            RecordForm::List(_) => LEAD_RECORD_LIST,
        };
        let mut out = Vec::with_capacity(len);
        out.push((CONTAINER_VERSION << 6) | form_bit | (codec_bits & LEAD_CODEC));
        out.push(echo.byte());
        write_leb128(&mut out, body);
        if takes_dictionary {
            write_leb128(&mut out, self.dictionary);
        }
        if self.codec != 0 {
            write_leb128(&mut out, self.decoded_len);
        }
        match self.form {
            RecordForm::Single(content_type) => write_leb128(&mut out, content_type.0),
            RecordForm::List(count) => write_leb128(&mut out, count),
        }
        out.extend_from_slice(coded);
        let crc = crc32c(&out);
        out.extend_from_slice(&crc.to_be_bytes());
        Ok(out)
    }
}

/// Appends the padding of 3.8 to a serialised container until it is `capacity` bytes long.
///
/// # Errors
///
/// [`WriteError::CapacityExceeded`] when `message` is already longer than `capacity`; `message`
/// is then unchanged.
pub fn pad_message(message: &mut Vec<u8>, capacity: usize) -> Result<(), WriteError> {
    let len = message.len();
    if len > capacity {
        return Err(WriteError::CapacityExceeded { len, capacity });
    }
    message.extend((0..capacity - len).map(|i| if i % 2 == 0 { 0xEC } else { 0x11 }));
    Ok(())
}

/// A container whose body length and CRC-32C have passed, of any container version (the layout
/// rule of 3.2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RawContainer<'a> {
    /// The lead byte.
    pub lead: u8,
    /// The format echo byte, in the layout of a static container; `None` for a transfer tile.
    pub echo: Option<u8>,
    /// The body: fields 4 to 11 of a static container. Not yet interpreted.
    pub body: &'a [u8],
    /// The container length, CRC-32C included: `2 + size(Lb) + Lb + 4` for a static container,
    /// `1 + size(Lb) + Lb + 4` for a transfer tile. The bytes after it are padding.
    pub len: usize,
}

impl RawContainer<'_> {
    /// The container version, bits 7–6 of the lead byte.
    pub const fn version(&self) -> u8 {
        self.lead >> 6
    }
}

/// The number of bytes before Lb in a static container: the lead byte and the format echo byte
/// (3.2.1).
pub const STATIC_PREFIX_LEN: usize = 2;
/// The number of bytes before Lb in a transfer tile: the lead byte (3.3).
pub const TILE_PREFIX_LEN: usize = 1;

/// Reads the lead byte, the format echo byte of a static container and Lb, and checks the
/// CRC-32C of a message (3.2.1, 3.2.3, 3.3, 3.7).
///
/// `class` is the symbol class of the chosen format word, which selects the layout: a static
/// container has the format echo byte after its lead byte, a transfer tile has none. This works
/// for every container version, so a reader can offer the body bytes of a container that is
/// otherwise unsupported (3.9). The message is the whole K-byte message; padding after the
/// CRC-32C is ignored.
///
/// # Errors
///
/// - [`Error::LengthField`] when Lb is not a valid LEB128 (truncated, not minimal, 2^32 or
///   more) or places the CRC-32C beyond the end of `message`. Every comparison is done without
///   overflow, so Lb = 2^32 − 1 is refused on every platform.
/// - [`Error::CrcMismatch`] when the CRC-32C does not match.
pub fn split_container(message: &[u8], class: SymbolClass) -> Result<RawContainer<'_>, Error> {
    let prefix = match class {
        SymbolClass::Static => STATIC_PREFIX_LEN,
        SymbolClass::TransferTile => TILE_PREFIX_LEN,
    };
    let (head, rest) = message.split_at_checked(prefix).ok_or(Error::LengthField)?;
    let (body_len, body_len_size) = read_leb128(rest).map_err(|_| Error::LengthField)?;
    let body_start = prefix + body_len_size;
    let body_end = usize::try_from(body_len)
        .ok()
        .and_then(|len| body_start.checked_add(len))
        .ok_or(Error::LengthField)?;
    let crc_end = body_end.checked_add(CRC32C_LEN).ok_or(Error::LengthField)?;
    let covered = message.get(..body_end).ok_or(Error::LengthField)?;
    let crc: [u8; CRC32C_LEN] = message
        .get(body_end..crc_end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::LengthField)?;
    if crc32c(covered) != u32::from_be_bytes(crc) {
        return Err(Error::CrcMismatch);
    }
    let body = covered.get(body_start..).ok_or(Error::LengthField)?;
    let lead = *head.first().ok_or(Error::LengthField)?;
    let echo = head.get(1).copied();
    Ok(RawContainer { lead, echo, body, len: crc_end })
}

/// The header of a parsed static container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContainerHeader {
    /// The record form, the codec ID, the dictionary ID (0 when absent) and the decoded length
    /// (equal to the coded length for codec 0).
    pub fields: StaticFields,
    /// Fields 7 and 8 when the lead byte has X = 1: a colour extension message exists (3.5).
    pub extension: Option<ExtensionDigest>,
}

/// A static container read from a message by [`parse_message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParsedContainer<'a> {
    /// The header fields.
    pub header: ContainerHeader,
    /// The content, called the coded field in chapter 6: the bytes the codec decodes.
    pub coded: &'a [u8],
    /// The container length, CRC-32C included; the bytes after it are padding.
    pub len: usize,
}

/// A read position in a byte string.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'a> {
    rest: &'a [u8],
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    /// Reads one LEB128 value.
    pub(crate) fn leb128(&mut self) -> Result<u32, Leb128Error> {
        let (value, len) = read_leb128(self.rest)?;
        self.rest = self.rest.get(len..).unwrap_or_default();
        Ok(value)
    }

    /// Takes the next `len` bytes, or `None` when fewer remain.
    pub(crate) fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.rest.split_at_checked(len)?;
        self.rest = rest;
        Some(taken)
    }

    /// The bytes not yet read.
    pub(crate) const fn rest(&self) -> &'a [u8] {
        self.rest
    }
}

/// A LEB128 failure inside the body (3.9): running past the end of the body is a header overrun,
/// a non-minimal or too large value is `E_LEB128`.
const fn header_leb128(error: Leb128Error) -> Error {
    match error {
        Leb128Error::Truncated => Error::HeaderOverrun,
        Leb128Error::NonMinimal | Leb128Error::TooLarge => Error::Leb128,
    }
}

/// Reads the container at the start of a base-layer message, in the order of 3.9.
///
/// `message` is the K-byte message after error correction (chapter 4); `format` is the format
/// word chosen by [`crate::decode_format`]. The checks run in this order: Lb and the CRC-32C
/// ([`split_container`]); the container version; for a transfer tile, its reserved lead-byte
/// bits and then the unsupported outcome; for a static container, the format echo byte against
/// `format`, the colour-extension bit X against the format word's colour profile, then the
/// fields 4 to 10 in order, each checked as it is read. Nothing of fields 4 to 11 is read before
/// the CRC-32C has passed.
///
/// A container with X = 1 is returned with its digest in `header.extension`; the records it
/// holds are the base records (3.5). This crate does not read the colour layer.
///
/// # Errors
///
/// Every error of 3.9 that the container header can produce: [`Error::LengthField`],
/// [`Error::CrcMismatch`], [`Error::ContainerVersion`], [`Error::TileReservedBits`],
/// [`Error::TransferUnsupported`], [`Error::FormatEcho`], [`Error::ColourFlag`],
/// [`Error::Leb128`], [`Error::HeaderOverrun`], [`Error::CodecEscape`],
/// [`Error::UnsupportedCodec`], [`Error::UnknownDictionary`], [`Error::DictionaryMismatch`],
/// [`Error::Malformed`] (L above [`MAX_STATIC_CONTENT_LEN_V0`]), [`Error::TooLarge`] (L above
/// the reader's limit), [`Error::HashIdInvalid`], [`Error::UnknownHash`] and
/// [`Error::RecordList`] (record count 0).
pub fn parse_message<'a>(
    message: &'a [u8],
    format: &FormatWord,
    config: &ReaderConfig<'_>,
) -> Result<ParsedContainer<'a>, Error> {
    let raw = split_container(message, format.class())?;
    if raw.version() != CONTAINER_VERSION {
        return Err(Error::ContainerVersion);
    }
    match format.class() {
        SymbolClass::TransferTile => {
            if raw.lead & LEAD_TILE_RESERVED != 0 {
                return Err(Error::TileReservedBits);
            }
            Err(Error::TransferUnsupported)
        }
        SymbolClass::Static => {
            if raw.echo != Some(format.echo().byte()) {
                return Err(Error::FormatEcho);
            }
            let (header, coded) = parse_static_header(raw.lead, raw.body, format, config)?;
            Ok(ParsedContainer { header, coded, len: raw.len })
        }
    }
}

/// Fields 4 to 10 of a static container (3.2.1 to 3.2.5), and the coded field.
fn parse_static_header<'a>(
    lead: u8,
    body: &'a [u8],
    format: &FormatWord,
    config: &ReaderConfig<'_>,
) -> Result<(ContainerHeader, &'a [u8]), Error> {
    let record_list = lead & LEAD_RECORD_LIST != 0;
    let colour = lead & LEAD_COLOUR != 0;
    if colour && format.colour_profile() == 0 {
        return Err(Error::ColourFlag);
    }
    let mut cursor = Cursor::new(body);
    let codec = match u32::from(lead & LEAD_CODEC) {
        CODEC_ESCAPE => {
            let escaped = cursor.leb128().map_err(header_leb128)?;
            if escaped < CODEC_ESCAPE {
                return Err(Error::CodecEscape);
            }
            escaped
        }
        codec => codec,
    };
    let takes_dictionary = config.codec(codec)?;
    let dictionary = if takes_dictionary {
        let id = cursor.leb128().map_err(header_leb128)?;
        config.check_dictionary(codec, id)?;
        id
    } else {
        0
    };
    let decoded_len = if codec == 0 {
        None
    } else {
        let len = cursor.leb128().map_err(header_leb128)?;
        config.check_decoded_len(len)?;
        Some(len)
    };
    let extension = if colour {
        let algorithm = HashAlgorithm::from_id(cursor.leb128().map_err(header_leb128)?)?;
        let digest: [u8; SHA256_LEN] = cursor
            .take(algorithm.digest_len())
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::HeaderOverrun)?;
        Some(ExtensionDigest { algorithm, digest })
    } else {
        None
    };
    let form = if record_list {
        let count = cursor.leb128().map_err(header_leb128)?;
        if count == 0 {
            return Err(Error::RecordList);
        }
        RecordForm::List(count)
    } else {
        RecordForm::Single(ContentType(cursor.leb128().map_err(header_leb128)?))
    };
    let content = cursor.rest();
    let decoded_len = if let Some(len) = decoded_len {
        len
    } else {
        // Codec 0: L = Lc, with the same checks before allocation (3.2.4).
        let len = u32::try_from(content.len()).map_err(|_| Error::Malformed)?;
        config.check_decoded_len(len)?;
        len
    };
    let header = ContainerHeader {
        fields: StaticFields { form, codec, dictionary, decoded_len },
        extension,
    };
    Ok((header, content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn padding_alternates_from_ec() {
        let mut message = vec![1, 2, 3];
        pad_message(&mut message, 8).unwrap();
        assert_eq!(message, [1, 2, 3, 0xEC, 0x11, 0xEC, 0x11, 0xEC]);
        assert_eq!(
            pad_message(&mut message, 7),
            Err(WriteError::CapacityExceeded { len: 8, capacity: 7 })
        );
        assert_eq!(message.len(), 8);
    }

    #[test]
    fn hash_ids() {
        assert_eq!(HashAlgorithm::from_id(1), Ok(HashAlgorithm::Sha256));
        for id in [0, 3, 4, 5, 6] {
            assert_eq!(HashAlgorithm::from_id(id), Err(Error::HashIdInvalid));
        }
        for id in [2, 7, 8, u32::MAX] {
            assert_eq!(HashAlgorithm::from_id(id), Err(Error::UnknownHash));
        }
        assert_eq!(HashAlgorithm::Sha256.id(), 1);
        assert_eq!(HashAlgorithm::Sha256.digest_len(), 32);
    }
}
