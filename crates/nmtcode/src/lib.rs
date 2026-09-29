//! NMT Code: a two-dimensional code designed to hold more than a QR Code of the same area.
//! Encode content to a symbol and decode it back.
//!
//! This crate runs the whole base-layer pipeline of the NMT Code specification 0.2 (chapter 1,
//! 1.3) on top of the layer crates:
//!
//! | Step | Encoding ([`encode`]) | Decoding ([`decode`]) | Crate |
//! |---|---|---|---|
//! | 1 | records to the decoded content, codec choice (6.3) | codec, records, reader outcome (3.9) | `nmtcode-payload`, `nmtcode-core` |
//! | 2 | container, CRC-32C, padding (3.2, 3.7, 3.8) | container checks (3.2, 3.7, 3.9) | `nmtcode-core` |
//! | 3 | block split and Reed-Solomon parity (4.6, 4.4, 4.8) | Reed-Solomon correction (4.9) | `nmtcode-ecc` |
//! | 4 | placement and whitening (5.8 to 5.10) | codeword stream from the modules | `nmtcode-symbol` |
//! | 5 | format word and function patterns (2, 5) | format word from its two copies (2.7) | `nmtcode-core`, `nmtcode-symbol` |
//!
//! The input of [`decode`] and the output of [`encode`] is a [`ModuleGrid`]: the modules of one
//! symbol, quiet zone excluded. Drawing it as an image and finding it in an image are the jobs of
//! `nmtcode-render` and `nmtcode-detect`.
//!
//! ```
//! use nmtcode::{DecodeOptions, EncodeOptions, PresentAs, decode, encode_url};
//!
//! let symbol = encode_url("https://github.com/needmoretruth/nmtcode", &EncodeOptions::default())?;
//! assert_eq!((symbol.width(), symbol.height()), (20, 28));
//!
//! let decoded = decode(symbol.grid(), &DecodeOptions::default())?;
//! assert_eq!(decoded.records[0].present_as, PresentAs::Url);
//! assert_eq!(decoded.records[0].text(), Some("https://github.com/needmoretruth/nmtcode"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! This version makes and reads black-and-white static symbols. The `color` profile of
//! chapter 7 is refused by [`encode`]; a colour symbol's base records are still decoded
//! (chapter 7, 7.9.1). Transfer tiles are reported as unsupported (chapter 3, 3.3).

mod decode;
mod encode;
mod size;

pub use decode::{
    DecodeError, DecodeOptions, Decoded, DecodedRecord, MAX_AREA, decode, decode_with_erasures,
};
pub use encode::{
    EncodeError, EncodeOptions, Profile, Symbol, encode, encode_file, encode_text, encode_url,
};
pub use size::{AspectRatio, SizeConstraints, SizeRule, capacity};

/// Which codecs the encoder tries (chapter 6, 6.3). The default tries every codec; turning one
/// off is the restricted mode that 6.3 allows. Stored (codec 0) is always a candidate.
pub use nmtcode_payload::EncodeOptions as CodecOptions;

/// The block split of chapter 4 (4.6) that a symbol uses.
pub use nmtcode_ecc::BlockSplit;

/// The reader errors of chapter 9 (9.8), with their stable names and outcome classes.
pub use nmtcode_core::Error as SpecError;

pub use nmtcode_core::{
    ContentType, FormatWord, MAX_CONTENT_LEN_V0, MAX_STATIC_CONTENT_LEN_V0, ModuleGrid, Outcome,
    PresentAs, Record, RecordForm, Role, SymbolClass, ValueNotice, safe_file_name,
};
