//! The reader errors of chapter 9 (9.8) and the outcome classes of chapter 3 (3.9).

/// What a reader presents after a check (chapter 3, 3.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Nothing; a reader reading video keeps reading frames.
    Damaged,
    /// Nothing from the content; an error naming the unsupported item. The reader MAY offer the
    /// body bytes for saving, labelled as undecoded.
    Unsupported,
    /// Nothing; an error saying the symbol breaks the specification.
    Malformed,
    /// The records, per chapter 3, 3.4 and 3.5.
    Presented,
    /// The base records and a statement that further content could not be read (3.5 rule 2).
    PresentedBaseOnly,
    /// An error is reported next to any NMT Code result. Not a class of 3.9: chapter 9 (9.8)
    /// gives this outcome to [`Error::BootstrapMismatch`] only.
    ErrorReported,
}

/// Every reader error of chapter 9 (9.8), one variant per stable name.
///
/// [`Error::name`] gives the stable name and [`Error::outcome`] the outcome class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// `E_FORMAT_UNREADABLE` (2.3, 2.7): no copy of the format word decodes within 3 bit errors to
    /// valid fields.
    FormatUnreadable,
    /// `E_FORMAT_VERSION` (2.3, 9.2): the format version is 1, 2 or 3; a newer reader is needed.
    FormatVersion,
    /// `E_TRANSFER_UNSUPPORTED` (2.3, 3.3): the symbol is a transfer tile and the reader does not
    /// implement transfer.
    TransferUnsupported,
    /// `E_ECC_FAILED` (4.9): a base-layer Reed-Solomon block could not be corrected.
    EccFailed,
    /// `E_LAYER_TOO_SMALL` (4.6, 7.8.2): a layer has fewer codewords than its minimum.
    LayerTooSmall,
    /// `E_LENGTH_FIELD` (3.2.3, 3.7): the body length Lb is not a valid LEB128, or places the
    /// CRC-32C beyond the message capacity.
    LengthField,
    /// `E_CRC_MISMATCH` (3.7): the CRC-32C of the container does not match.
    CrcMismatch,
    /// `E_CONTAINER_VERSION` (3.2.2): the container version is 1, 2 or 3.
    ContainerVersion,
    /// `E_TILE_RESERVED_BITS` (3.3): a transfer tile's reserved lead-byte bits are not 0.
    TileReservedBits,
    /// `E_TILE_COLOUR` (3.3, 7.3): a transfer tile has a colour profile other than 0.
    TileColour,
    /// `E_COLOUR_FLAG` (3.2.2): the container says C = 1 while the format word's colour profile
    /// is 0.
    ColourFlag,
    /// `E_LEB128` (1.4, 3.9): a LEB128 field in the body is not minimal or is 2^32 or more.
    Leb128,
    /// `E_CODEC_ESCAPE` (3.2.4): the codec ID escape holds a value below 15.
    CodecEscape,
    /// `E_UNSUPPORTED_CODEC` (6.2): the codec ID is reserved, 16 or more, or not implemented by
    /// the reader.
    UnsupportedCodec,
    /// `E_UNKNOWN_DICTIONARY` (6.11): the dictionary ID is not 0 and the reader does not carry it,
    /// or it is in the private-use range.
    UnknownDictionary,
    /// `E_DICTIONARY_MISMATCH` (6.2, 6.11): the registered kind of the dictionary does not match
    /// the codec.
    DictionaryMismatch,
    /// `E_HASH_ID_INVALID` (3.6): the hash algorithm ID is 0 or 3 to 6.
    HashIdInvalid,
    /// `E_UNKNOWN_HASH` (3.6): the hash algorithm ID is otherwise unknown to the reader.
    UnknownHash,
    /// `E_HEADER_OVERRUN` (3.9): the header fields run past the end of the body.
    HeaderOverrun,
    /// `E_TOO_LARGE` (6.4): the decoded length L exceeds the reader's own limit.
    TooLarge,
    /// `E_MALFORMED` (6.4 to 6.10): the decoded length exceeds `MAX_CONTENT_LEN_V0`, the coded
    /// field is invalid for its codec, or decoding gives a length other than L.
    Malformed,
    /// `E_RECORD_LIST` (3.2.5): record count 0, a record that runs past the end, or bytes left
    /// over.
    RecordList,
    /// `E_NO_BASE_RECORD` (3.5): C = 1 and the base layer holds no record.
    NoBaseRecord,
    /// `E_ACTION_RULE` (3.4.4): more than one action record, or an action record that is not the
    /// first base record.
    ActionRule,
    /// `E_DIGEST_MISMATCH` (3.5, 7.9.2): the digest over base and extension records differs from
    /// the base container's digest; no record is presented, base records included.
    DigestMismatch,
    /// `E_EXTENSION_UNREAD` (3.5, 7.9): the colour layer was not read, failed error correction,
    /// failed its CRC-32C, or is unsupported or malformed. The base records are presented.
    ExtensionUnread,
    /// `E_BOOTSTRAP_MISMATCH` (8.6): a QR symbol beside the NMT Code symbol differs from every
    /// known bootstrap URL; its content is never presented.
    BootstrapMismatch,
}

impl Error {
    /// Every variant, in the order of the table of chapter 9 (9.8).
    pub const ALL: [Self; 27] = [
        Self::FormatUnreadable,
        Self::FormatVersion,
        Self::TransferUnsupported,
        Self::EccFailed,
        Self::LayerTooSmall,
        Self::LengthField,
        Self::CrcMismatch,
        Self::ContainerVersion,
        Self::TileReservedBits,
        Self::TileColour,
        Self::ColourFlag,
        Self::Leb128,
        Self::CodecEscape,
        Self::UnsupportedCodec,
        Self::UnknownDictionary,
        Self::DictionaryMismatch,
        Self::HashIdInvalid,
        Self::UnknownHash,
        Self::HeaderOverrun,
        Self::TooLarge,
        Self::Malformed,
        Self::RecordList,
        Self::NoBaseRecord,
        Self::ActionRule,
        Self::DigestMismatch,
        Self::ExtensionUnread,
        Self::BootstrapMismatch,
    ];

    /// The stable name of chapter 9 (9.8), for example `"E_FORMAT_UNREADABLE"`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::FormatUnreadable => "E_FORMAT_UNREADABLE",
            Self::FormatVersion => "E_FORMAT_VERSION",
            Self::TransferUnsupported => "E_TRANSFER_UNSUPPORTED",
            Self::EccFailed => "E_ECC_FAILED",
            Self::LayerTooSmall => "E_LAYER_TOO_SMALL",
            Self::LengthField => "E_LENGTH_FIELD",
            Self::CrcMismatch => "E_CRC_MISMATCH",
            Self::ContainerVersion => "E_CONTAINER_VERSION",
            Self::TileReservedBits => "E_TILE_RESERVED_BITS",
            Self::TileColour => "E_TILE_COLOUR",
            Self::ColourFlag => "E_COLOUR_FLAG",
            Self::Leb128 => "E_LEB128",
            Self::CodecEscape => "E_CODEC_ESCAPE",
            Self::UnsupportedCodec => "E_UNSUPPORTED_CODEC",
            Self::UnknownDictionary => "E_UNKNOWN_DICTIONARY",
            Self::DictionaryMismatch => "E_DICTIONARY_MISMATCH",
            Self::HashIdInvalid => "E_HASH_ID_INVALID",
            Self::UnknownHash => "E_UNKNOWN_HASH",
            Self::HeaderOverrun => "E_HEADER_OVERRUN",
            Self::TooLarge => "E_TOO_LARGE",
            Self::Malformed => "E_MALFORMED",
            Self::RecordList => "E_RECORD_LIST",
            Self::NoBaseRecord => "E_NO_BASE_RECORD",
            Self::ActionRule => "E_ACTION_RULE",
            Self::DigestMismatch => "E_DIGEST_MISMATCH",
            Self::ExtensionUnread => "E_EXTENSION_UNREAD",
            Self::BootstrapMismatch => "E_BOOTSTRAP_MISMATCH",
        }
    }

    /// The outcome class that chapter 9 (9.8) gives this error, in the terms of chapter 3 (3.9).
    pub const fn outcome(self) -> Outcome {
        match self {
            Self::FormatUnreadable | Self::EccFailed | Self::LengthField | Self::CrcMismatch => {
                Outcome::Damaged
            }
            Self::FormatVersion
            | Self::TransferUnsupported
            | Self::ContainerVersion
            | Self::UnsupportedCodec
            | Self::UnknownDictionary
            | Self::UnknownHash
            | Self::TooLarge => Outcome::Unsupported,
            Self::LayerTooSmall
            | Self::TileReservedBits
            | Self::TileColour
            | Self::ColourFlag
            | Self::Leb128
            | Self::CodecEscape
            | Self::DictionaryMismatch
            | Self::HashIdInvalid
            | Self::HeaderOverrun
            | Self::Malformed
            | Self::RecordList
            | Self::NoBaseRecord
            | Self::ActionRule
            | Self::DigestMismatch => Outcome::Malformed,
            Self::ExtensionUnread => Outcome::PresentedBaseOnly,
            Self::BootstrapMismatch => Outcome::ErrorReported,
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

impl core::error::Error for Error {}
