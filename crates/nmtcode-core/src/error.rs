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
    /// The records with an error next to them; the action record is not offered until the user
    /// has seen the error (chapter 8, 8.6). Chapter 9 (9.8) gives this outcome to
    /// [`Error::BootstrapMismatch`] only.
    PresentedWithError,
}

/// Every reader error of chapter 9 (9.8), one variant per stable name, in the order of that
/// table.
///
/// [`Error::name`] gives the stable name and [`Error::outcome`] the outcome class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// `E_NESTED_SYMBOL` (5.11): a detected symbol lies inside the area of another detected
    /// symbol; neither is presented.
    NestedSymbol,
    /// `E_FORMAT_UNREADABLE` (2.3, 2.7): no copy of the format word decodes to a valid word, or
    /// the one decoded copy has format version 1 to 3 with 2e + s > 4.
    FormatUnreadable,
    /// `E_FORMAT_CONFLICT` (2.7): both copies of the format word decode, to different words.
    FormatConflict,
    /// `E_FORMAT_VERSION` (2.3, 9.2): the format version is 1, 2 or 3; a newer reader is needed.
    FormatVersion,
    /// `E_SIZE_LIMIT` (2.7, 5.11): W × H of the chosen format word exceeds the largest area this
    /// reader accepts.
    SizeLimit,
    /// `E_ECC_FAILED` (4.9): a base-layer Reed-Solomon block could not be corrected.
    EccFailed,
    /// `E_COLOUR_AMBIGUOUS` (7.9): more modules than chapter 7 allows have a colour that does
    /// not fit their luminance class and the colour profile.
    ColourAmbiguous,
    /// `E_LENGTH_FIELD` (3.2.3, 3.7): the body length Lb is not a valid LEB128, or places the
    /// CRC-32C beyond the message capacity.
    LengthField,
    /// `E_CRC_MISMATCH` (3.7): the CRC-32C of the container does not match.
    CrcMismatch,
    /// `E_CONTAINER_VERSION` (3.2.2): the container version is 1, 2 or 3.
    ContainerVersion,
    /// `E_TILE_RESERVED_BITS` (3.3): a transfer tile's reserved lead-byte bits are not 0.
    TileReservedBits,
    /// `E_TRANSFER_UNSUPPORTED` (2.3, 3.3): the symbol is a transfer tile and the reader does not
    /// implement transfer.
    TransferUnsupported,
    /// `E_FORMAT_ECHO` (3.2.2): the format echo byte of a static container differs from the
    /// byte built from the chosen format word, reserved bits included.
    FormatEcho,
    /// `E_COLOUR_FLAG` (3.2.2): the container says X = 1 while the format word's colour profile
    /// is 0.
    ColourFlag,
    /// `E_LEB128` (1.4, 3.9): a LEB128 field in the body is not minimal or is 2^32 or more.
    Leb128,
    /// `E_CODEC_ESCAPE` (3.2.4): the codec ID escape holds a value below 15.
    CodecEscape,
    /// `E_UNSUPPORTED_CODEC` (6.2): the codec ID is reserved, 16 or more, or not implemented by
    /// the reader.
    UnsupportedCodec,
    /// `E_UNKNOWN_DICTIONARY` (6.11): the dictionary ID is not 0 and the reader does not carry
    /// it, including IDs of the reserved and private-use ranges.
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
    /// `E_MALFORMED` (6.4 to 6.10): the decoded length exceeds its cap, the coded field breaks
    /// its codec's rules or expansion bound, or decoding gives a length other than L.
    Malformed,
    /// `E_RECORD_LIST` (3.2.5): record count 0, a record that runs past the end, or bytes left
    /// over.
    RecordList,
    /// `E_ACTION_RULE` (3.4.4): more than one action record in the base records, or an action
    /// record that is not the first base record.
    ActionRule,
    /// `E_DIGEST_MISMATCH` (3.5, 7.9): the digest over base and extension records differs from
    /// the base container's digest; no record is presented, base records included.
    DigestMismatch,
    /// `E_EXTENSION_UNREAD` (3.5, 7.9): the colour layer was not read, or it or its records
    /// failed a check. The base records are presented.
    ExtensionUnread,
    /// `E_BOOTSTRAP_MISMATCH` (8.6): a QR symbol beside the NMT Code symbol differs from every
    /// known bootstrap URL and does not have the shape of one; its content is never presented.
    BootstrapMismatch,
}

impl Error {
    /// Every variant, in the order of the table of chapter 9 (9.8).
    pub const ALL: [Self; 29] = [
        Self::NestedSymbol,
        Self::FormatUnreadable,
        Self::FormatConflict,
        Self::FormatVersion,
        Self::SizeLimit,
        Self::EccFailed,
        Self::ColourAmbiguous,
        Self::LengthField,
        Self::CrcMismatch,
        Self::ContainerVersion,
        Self::TileReservedBits,
        Self::TransferUnsupported,
        Self::FormatEcho,
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
        Self::ActionRule,
        Self::DigestMismatch,
        Self::ExtensionUnread,
        Self::BootstrapMismatch,
    ];

    /// The stable name of chapter 9 (9.8), for example `"E_FORMAT_UNREADABLE"`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NestedSymbol => "E_NESTED_SYMBOL",
            Self::FormatUnreadable => "E_FORMAT_UNREADABLE",
            Self::FormatConflict => "E_FORMAT_CONFLICT",
            Self::FormatVersion => "E_FORMAT_VERSION",
            Self::SizeLimit => "E_SIZE_LIMIT",
            Self::EccFailed => "E_ECC_FAILED",
            Self::ColourAmbiguous => "E_COLOUR_AMBIGUOUS",
            Self::LengthField => "E_LENGTH_FIELD",
            Self::CrcMismatch => "E_CRC_MISMATCH",
            Self::ContainerVersion => "E_CONTAINER_VERSION",
            Self::TileReservedBits => "E_TILE_RESERVED_BITS",
            Self::TransferUnsupported => "E_TRANSFER_UNSUPPORTED",
            Self::FormatEcho => "E_FORMAT_ECHO",
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
            Self::ActionRule => "E_ACTION_RULE",
            Self::DigestMismatch => "E_DIGEST_MISMATCH",
            Self::ExtensionUnread => "E_EXTENSION_UNREAD",
            Self::BootstrapMismatch => "E_BOOTSTRAP_MISMATCH",
        }
    }

    /// The outcome class that chapter 9 (9.8) gives this error, in the terms of chapter 3 (3.9).
    pub const fn outcome(self) -> Outcome {
        match self {
            Self::FormatUnreadable
            | Self::FormatConflict
            | Self::EccFailed
            | Self::LengthField
            | Self::CrcMismatch => Outcome::Damaged,
            Self::FormatVersion
            | Self::SizeLimit
            | Self::TransferUnsupported
            | Self::ContainerVersion
            | Self::UnsupportedCodec
            | Self::UnknownDictionary
            | Self::UnknownHash
            | Self::TooLarge => Outcome::Unsupported,
            Self::NestedSymbol
            | Self::ColourAmbiguous
            | Self::TileReservedBits
            | Self::FormatEcho
            | Self::ColourFlag
            | Self::Leb128
            | Self::CodecEscape
            | Self::DictionaryMismatch
            | Self::HashIdInvalid
            | Self::HeaderOverrun
            | Self::Malformed
            | Self::RecordList
            | Self::ActionRule
            | Self::DigestMismatch => Outcome::Malformed,
            Self::ExtensionUnread => Outcome::PresentedBaseOnly,
            Self::BootstrapMismatch => Outcome::PresentedWithError,
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

impl core::error::Error for Error {}
