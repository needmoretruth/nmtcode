//! Records of chapter 3: the content type registry (3.4.2), presentation by type (3.4.3), the
//! record forms (3.2.5), canonical records (3.4.1) and the ordering rules (3.4.4).
//!
//! A generator turns its records into the content a codec encodes with
//! [`RecordContent::from_records`]. A reader turns the decoded content back into records with
//! [`parse_records`].

use alloc::string::String;
use alloc::vec::Vec;

use crate::container::{ContainerHeader, Cursor, MAX_STATIC_CONTENT_LEN_V0, WriteError};
use crate::format::FormatWord;
use crate::leb128::{Leb128Error, leb128_len, write_leb128};
use crate::{Error, Outcome};

/// The first bytes of a PSBT (BIP-174): `70 73 62 74 FF` (3.4.2, 3.4.3).
pub const PSBT_MAGIC: [u8; 5] = [0x70, 0x73, 0x62, 0x74, 0xFF];

/// A content type ID, which is also a record type ID (3.4.1, 3.4.2).
///
/// Any `u32` is a valid value: an ID the registry does not assign is presented as unknown data
/// (3.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentType(pub u32);

/// The role of a content type (3.4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Shown to the user.
    Data,
    /// Shown to the user as a file.
    File,
    /// Shown, and may also be offered as an action.
    Action,
    /// Describes the next record.
    Attribute,
}

impl ContentType {
    /// 0: unspecified bytes.
    pub const UNSPECIFIED: Self = Self(0);
    /// 1: UTF-8 text.
    pub const TEXT: Self = Self(1);
    /// 2: a URI or IRI, UTF-8. The action type.
    pub const URL: Self = Self(2);
    /// 3: UTF-8 JSON text.
    pub const JSON: Self = Self(3);
    /// 4: the bytes of a file.
    pub const FILE: Self = Self(4);
    /// 5: the UTF-8 name of the file in the next record.
    pub const FILE_NAME: Self = Self(5);
    /// 6: a partially signed Bitcoin transaction.
    pub const PSBT: Self = Self(6);
    /// 7: a `COSE_Sign1` envelope (reserved: never described as verified in this version).
    pub const COSE_SIGN1: Self = Self(7);
    /// 8: an age v1 file (reserved: not decrypted in this version).
    pub const AGE: Self = Self(8);

    /// Whether the registry of this version assigns this ID (0 to 8).
    pub const fn is_known(self) -> bool {
        self.0 <= 8
    }

    /// The registry name, or `None` for an unassigned ID.
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("Unspecified bytes"),
            1 => Some("Text"),
            2 => Some("URL"),
            3 => Some("JSON"),
            4 => Some("File"),
            5 => Some("File name"),
            6 => Some("PSBT"),
            7 => Some("COSE_Sign1 envelope"),
            8 => Some("age file"),
            _ => None,
        }
    }

    /// The media type the registry gives, if any.
    pub const fn media_type(self) -> Option<&'static str> {
        match self.0 {
            1 => Some("text/plain; charset=utf-8"),
            3 => Some("application/json"),
            4 => Some("application/octet-stream"),
            7 => Some("application/cose; cose-type=\"cose-sign1\""),
            _ => None,
        }
    }

    /// The role, or `None` for an unassigned ID.
    pub const fn role(self) -> Option<Role> {
        match self.0 {
            0 | 1 | 3 => Some(Role::Data),
            2 => Some(Role::Action),
            4 | 6 | 7 | 8 => Some(Role::File),
            5 => Some(Role::Attribute),
            _ => None,
        }
    }

    /// "Never compress": a container that holds such a record uses codec 0 (3.4.2).
    pub const fn never_compress(self) -> bool {
        matches!(self.0, 7 | 8)
    }

    /// Whether a file name record may name a record of this type (3.4.4 rule 3).
    pub const fn is_file_name_target(self) -> bool {
        matches!(self.0, 0 | 4 | 6 | 7 | 8)
    }
}

/// The record form of a container (3.2.2, 3.2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecordForm {
    /// R = 0: one record, whose content type is field 9 and whose value is the decoded content.
    Single(ContentType),
    /// R = 1: the decoded content is this many canonical records (field 10, 1 or more).
    List(u32),
}

/// One record: a type ID and a value (3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Record<'a> {
    /// The content type ID.
    pub content_type: ContentType,
    /// The value.
    pub value: &'a [u8],
}

impl Record<'_> {
    /// Appends the canonical record `type ID ‖ value length ‖ value` (3.4.1).
    ///
    /// # Errors
    ///
    /// [`WriteError::ContentTooLarge`] when the value is 2^32 bytes or longer.
    pub fn write_canonical(&self, out: &mut Vec<u8>) -> Result<(), WriteError> {
        let len = u32::try_from(self.value.len()).map_err(|_| WriteError::ContentTooLarge)?;
        write_leb128(out, self.content_type.0);
        write_leb128(out, len);
        out.extend_from_slice(self.value);
        Ok(())
    }

    /// The length of the canonical record, or `None` when the value is 2^32 bytes or longer.
    fn canonical_len(&self) -> Option<usize> {
        let len = u32::try_from(self.value.len()).ok()?;
        (leb128_len(self.content_type.0) + leb128_len(len)).checked_add(self.value.len())
    }
}

/// The content a codec encodes, built from records, with the record form for the header.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecordContent {
    /// The record form: single-record form for one record, record-list form for more.
    pub form: RecordForm,
    /// The decoded content: the value in single-record form, the canonical records one after
    /// another in record-list form.
    pub decoded: Vec<u8>,
    /// True when a record's type is "never compress" (3.4.2): the generator uses codec 0.
    pub stored_only: bool,
}

impl RecordContent {
    /// Builds the decoded content of a static container from its records, in order (3.2.5).
    ///
    /// One record gives single-record form and two or more give record-list form, as 3.2.5
    /// requires of a generator. The records are checked against the rules a generator must
    /// follow: at least one record, at most one action record and only as the first (3.4.4
    /// rules 1 and 2), every file name record directly followed by a file record (rule 3), and a
    /// decoded length of at most [`MAX_STATIC_CONTENT_LEN_V0`] (chapter 6, 6.4).
    ///
    /// # Errors
    ///
    /// [`WriteError::NoRecords`], [`WriteError::ActionRule`],
    /// [`WriteError::FileNameWithoutTarget`] or [`WriteError::ContentTooLarge`].
    pub fn from_records(records: &[Record<'_>]) -> Result<Self, WriteError> {
        let (first, _) = records.split_first().ok_or(WriteError::NoRecords)?;
        let actions =
            records.iter().filter(|record| record.content_type == ContentType::URL).count();
        if actions > 1 || (actions == 1 && first.content_type != ContentType::URL) {
            return Err(WriteError::ActionRule);
        }
        for (index, record) in records.iter().enumerate() {
            if record.content_type == ContentType::FILE_NAME {
                let target = records.get(index + 1).map(|next| next.content_type);
                if !target.is_some_and(ContentType::is_file_name_target) {
                    return Err(WriteError::FileNameWithoutTarget);
                }
            }
        }
        let stored_only = records.iter().any(|record| record.content_type.never_compress());
        let max =
            usize::try_from(MAX_STATIC_CONTENT_LEN_V0).map_err(|_| WriteError::ContentTooLarge)?;
        if let [only] = records {
            if only.value.len() > max {
                return Err(WriteError::ContentTooLarge);
            }
            return Ok(Self {
                form: RecordForm::Single(only.content_type),
                decoded: only.value.to_vec(),
                stored_only,
            });
        }
        let total = records.iter().try_fold(0usize, |sum, record| {
            record.canonical_len().and_then(|len| sum.checked_add(len)).filter(|&sum| sum <= max)
        });
        let total = total.ok_or(WriteError::ContentTooLarge)?;
        let count = u32::try_from(records.len()).map_err(|_| WriteError::ContentTooLarge)?;
        let mut decoded = Vec::with_capacity(total);
        for record in records {
            record.write_canonical(&mut decoded)?;
        }
        Ok(Self { form: RecordForm::List(count), decoded, stored_only })
    }
}

/// The bytes the digest of a colour symbol is computed over (3.5): the format data d of the
/// symbol as 4 bytes, most significant first; the number of base records as LEB128; the
/// canonical base records; the number of extension records as LEB128; the canonical extension
/// records. The digest is SHA-256 of these bytes.
///
/// # Errors
///
/// [`WriteError::ContentTooLarge`] when a value is 2^32 bytes or longer or there are 2^32
/// records or more.
pub fn digest_input(
    format: &FormatWord,
    base: &[Record<'_>],
    extension: &[Record<'_>],
) -> Result<Vec<u8>, WriteError> {
    let mut out = Vec::new();
    out.extend_from_slice(&format.data().to_be_bytes());
    for records in [base, extension] {
        let count = u32::try_from(records.len()).map_err(|_| WriteError::ContentTooLarge)?;
        write_leb128(&mut out, count);
        for record in records {
            record.write_canonical(&mut out)?;
        }
    }
    Ok(out)
}

/// How a reader presents a record (3.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresentAs {
    /// As bytes (for example hexadecimal), available to copy or save: type 0, and the fallback
    /// for a value that fails the UTF-8 check of its type.
    Bytes,
    /// As text: types 1 and 3 with valid UTF-8.
    Text,
    /// The complete URL text, which the reader MAY offer to open after a user action: type 2
    /// with valid UTF-8.
    Url,
    /// As a file offered for saving, never opened or run: types 4, 7 and 8, and type 6 without
    /// the PSBT magic.
    File,
    /// As a PSBT file, which MAY also be offered to a wallet application: type 6 starting with
    /// [`PSBT_MAGIC`].
    Psbt,
    /// As the name of the file in the next record, never used as a path: type 5 with valid
    /// UTF-8 and a file record after it.
    FileName,
    /// As unknown data (the type ID, the length and the bytes to copy or save), never acted on:
    /// an unassigned type ID, or a file name record without a target (3.4.4 rule 3).
    Unknown,
}

/// Why a record is not presented as its type says (3.4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueNotice {
    /// The value of type 1, 2, 3 or 5 is not valid UTF-8; it is presented as bytes.
    NotUtf8,
    /// The value of type 6 does not start with [`PSBT_MAGIC`]; it is presented as a file.
    NotPsbt,
}

/// One record as a reader presents it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParsedRecord<'a> {
    /// The content type ID as written.
    pub content_type: ContentType,
    /// The decoded value.
    pub value: &'a [u8],
    /// How to present it (3.4.3).
    pub present_as: PresentAs,
    /// Set when `present_as` differs from the type because the value failed its type's check.
    pub notice: Option<ValueNotice>,
}

/// The records of a base-layer message, in presentation order (3.4.4 rule 4).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Records<'a> {
    /// The base records.
    pub records: Vec<ParsedRecord<'a>>,
    /// True when the container has X = 1: the symbol holds extension records in its colour
    /// layer, which this crate does not read (3.5 rule 2).
    pub extension_unread: bool,
}

impl Records<'_> {
    /// The outcome of 3.9: [`Outcome::PresentedBaseOnly`] when `extension_unread`, else
    /// [`Outcome::Presented`].
    pub const fn outcome(&self) -> Outcome {
        if self.extension_unread { Outcome::PresentedBaseOnly } else { Outcome::Presented }
    }

    /// The error a reader reports next to the records: [`Error::ExtensionUnread`] when
    /// `extension_unread`. The reader MUST then state that the symbol holds further content it
    /// could not read (3.5 rule 2).
    pub const fn notice(&self) -> Option<Error> {
        if self.extension_unread { Some(Error::ExtensionUnread) } else { None }
    }
}

/// A LEB128 failure inside a record list (3.9): running past the end is a record overrun,
/// a non-minimal or too large value is `E_LEB128`.
const fn record_leb128(error: Leb128Error) -> Error {
    match error {
        Leb128Error::Truncated => Error::RecordList,
        Leb128Error::NonMinimal | Leb128Error::TooLarge => Error::Leb128,
    }
}

/// The longest file name, in bytes of UTF-8, that [`safe_file_name`] returns (3.4.3).
pub const MAX_FILE_NAME_LEN: usize = 255;

/// Windows device names that a file name may not start with (3.4.3), compared without case on
/// the part before the first `.`.
const RESERVED_DEVICE_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Whether a character is removed from a file name (3.4.3 step 2): a control character, a
/// character that changes the direction of text, or an invisible character.
const fn is_removed_from_file_name(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// Whether `stem` (the part of a name before its first `.`) is a Windows device name: `CON`,
/// `PRN`, `AUX`, `NUL`, `COM0` to `COM9` or `LPT0` to `LPT9`, in any case.
fn is_device_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    RESERVED_DEVICE_NAMES.contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
}

/// The file name a reader may offer when saving the file that follows a file name record,
/// from that record's value, by the steps of 3.4.3:
///
/// 1. keep the part after the last `/` or `\`;
/// 2. remove control characters, characters that change the direction of text (U+061C,
///    U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069) and invisible characters (U+00AD,
///    U+180E, U+200B to U+200D, U+2060 to U+2064, U+FEFF);
/// 3. replace each of `:` `*` `?` `"` `<` `>` `|` by `_`;
/// 4. remove leading dots, then trailing dots and spaces;
/// 5. put `_` before a name whose part before the first `.` is a Windows device name;
/// 6. cut the name to at most [`MAX_FILE_NAME_LEN`] bytes, at a character boundary.
///
/// An empty result means the reader chooses a name of its own. `None` when the value is not
/// valid UTF-8; the record is then presented as bytes.
pub fn safe_file_name(value: &[u8]) -> Option<String> {
    let name = core::str::from_utf8(value).ok()?;
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .filter(|&c| !is_removed_from_file_name(c))
        .map(|c| if matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim_start_matches('.').trim_end_matches(['.', ' ']);
    let stem = trimmed.split('.').next().unwrap_or_default();
    let mut out = String::with_capacity(trimmed.len() + 1);
    if is_device_name(stem) {
        out.push('_');
    }
    out.push_str(trimmed);
    let mut end = out.len().min(MAX_FILE_NAME_LEN);
    while !out.is_char_boundary(end) {
        end -= 1;
    }
    out.truncate(end);
    Some(out)
}

/// How a record of `content_type` with `value` is presented, given the type of the next record.
fn presentation(
    content_type: ContentType,
    value: &[u8],
    next: Option<ContentType>,
) -> (PresentAs, Option<ValueNotice>) {
    let utf8 = core::str::from_utf8(value).is_ok();
    let checked = |kind: PresentAs| {
        if utf8 { (kind, None) } else { (PresentAs::Bytes, Some(ValueNotice::NotUtf8)) }
    };
    match content_type.0 {
        0 => (PresentAs::Bytes, None),
        1 | 3 => checked(PresentAs::Text),
        2 => checked(PresentAs::Url),
        4 | 7 | 8 => (PresentAs::File, None),
        5 if next.is_some_and(ContentType::is_file_name_target) => checked(PresentAs::FileName),
        6 if value.starts_with(&PSBT_MAGIC) => (PresentAs::Psbt, None),
        6 => (PresentAs::File, Some(ValueNotice::NotPsbt)),
        _ => (PresentAs::Unknown, None),
    }
}

/// Reads the records of a base-layer message from its decoded content (3.2.5, 3.4, 3.5).
///
/// `header` comes from [`crate::parse_message`]; `decoded` is the output of the codec named in
/// it. In record-list form the decoded content must be exactly the record count's canonical
/// records. The ordering rules of 3.4.4 are applied: more than one action record, or an action
/// record that is not the first, rejects the symbol; a file name record without a file record
/// after it is presented as unknown data. When the container has X = 1 the result says that
/// the extension records were not read.
///
/// # Errors
///
/// - [`Error::Malformed`] when `decoded` is not `header.fields.decoded_len` bytes (3.2.4).
/// - [`Error::RecordList`] for a record count of 0, a record that runs past the end, or bytes
///   left over (3.2.5).
/// - [`Error::Leb128`] for a type ID or value length that is not minimal or is 2^32 or more.
/// - [`Error::ActionRule`] when rule 1 or 2 of 3.4.4 is broken.
pub fn parse_records<'a>(
    header: &ContainerHeader,
    decoded: &'a [u8],
) -> Result<Records<'a>, Error> {
    if usize::try_from(header.fields.decoded_len).ok() != Some(decoded.len()) {
        return Err(Error::Malformed);
    }
    let extension_unread = header.extension.is_some();
    let raw: Vec<Record<'a>> = match header.fields.form {
        RecordForm::Single(content_type) => alloc::vec![Record { content_type, value: decoded }],
        RecordForm::List(0) => return Err(Error::RecordList),
        RecordForm::List(count) => {
            // Every canonical record takes at least 2 bytes, so this bounds the allocation.
            let capacity = usize::try_from(count).map_or(0, |count| count.min(decoded.len() / 2));
            let mut records = Vec::with_capacity(capacity);
            let mut cursor = Cursor::new(decoded);
            for _ in 0..count {
                let content_type = ContentType(cursor.leb128().map_err(record_leb128)?);
                let len = cursor.leb128().map_err(record_leb128)?;
                let value = usize::try_from(len)
                    .ok()
                    .and_then(|len| cursor.take(len))
                    .ok_or(Error::RecordList)?;
                records.push(Record { content_type, value });
            }
            if !cursor.rest().is_empty() {
                return Err(Error::RecordList);
            }
            records
        }
    };
    let actions = raw.iter().filter(|record| record.content_type == ContentType::URL).count();
    let first_is_action = raw.first().is_some_and(|record| record.content_type == ContentType::URL);
    if actions > 1 || (actions == 1 && !first_is_action) {
        return Err(Error::ActionRule);
    }
    let records = raw
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let next = raw.get(index + 1).map(|next| next.content_type);
            let (present_as, notice) = presentation(record.content_type, record.value, next);
            ParsedRecord {
                content_type: record.content_type,
                value: record.value,
                present_as,
                notice,
            }
        })
        .collect();
    Ok(Records { records, extension_unread })
}
