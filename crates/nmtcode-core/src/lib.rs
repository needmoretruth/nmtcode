//! Common types of NMT Code: module grid, format word, container and records, CRC-32C and the reader errors.
//!
//! - [`FormatWord`], [`decode_format`], [`decode_format_with`]: the format word of chapter 2 and
//!   its two copies as 47-bit integers.
//! - [`RecordContent`], [`StaticFields`], [`pad_message`]: a generator's records, static
//!   container and padding (chapter 3).
//! - [`parse_message`], [`parse_records`]: a reader's container and records (chapter 3), with
//!   the outcomes of 3.9; [`digest_input`]: the bytes of the colour digest (3.5).
//! - [`Error`]: the reader errors of chapter 9 (9.8).
//! - [`crc32c`], [`read_leb128`], [`write_leb128`]: the conventions of chapter 1 (1.4).

#![no_std]

extern crate alloc;

mod container;
mod crc32c;
mod error;
mod format;
mod grid;
mod leb128;
mod record;

pub use container::{
    CODEC_COUNT_V0, CODEC_ESCAPE, CONTAINER_VERSION, ContainerHeader, DictionaryEntry,
    DictionaryKind, ExtensionDigest, HashAlgorithm, MAX_CONTENT_LEN_V0, MAX_STATIC_CONTENT_LEN_V0,
    PADDING_PATTERN, ParsedContainer, RawContainer, ReaderConfig, SHA256_LEN, STATIC_PREFIX_LEN,
    StaticFields, TILE_PREFIX_LEN, WriteError, codec_takes_dictionary, pad_message, parse_message,
    split_container,
};
pub use crc32c::{CRC32C_CHECK, CRC32C_LEN, crc32c};
pub use error::{Error, Outcome};
pub use format::{
    FORMAT_CODEWORD_BITS, FORMAT_DATA_BITS, FORMAT_DECODING_BOUND, FORMAT_GENERATOR, FORMAT_MASK_A,
    FORMAT_MASK_B, FORMAT_MASKS, FORMAT_MAX_ERASURES, FORMAT_MAX_ERRORS, FORMAT_PARITY_BITS,
    FORMAT_VERSION, FORMAT_VERSION_REPORT_BOUND, FormatDecoded, FormatEcho, FormatSample,
    FormatWord, FormatWordError, SymbolClass, decode_format, decode_format_with, format_codeword,
    format_parity, format_syndrome,
};
pub use grid::{GridTextError, MAX_SIDE, MIN_SIDE, ModuleGrid, is_valid_side};
pub use leb128::{LEB128_MAX_LEN, Leb128Error, leb128_len, read_leb128, write_leb128};
pub use record::{
    ContentType, MAX_FILE_NAME_LEN, PSBT_MAGIC, ParsedRecord, PresentAs, Record, RecordContent,
    RecordForm, Records, Role, ValueNotice, digest_input, parse_records, safe_file_name,
};
