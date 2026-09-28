//! Payload codecs of NMT Code: stored, digits, upper-case alphanumeric, token
//! table with a short-text model, Hangul packing and brotli.
//!
//! This crate implements chapter 6 of the NMT Code specification 0.1
//! (`spec/06-payload-coding.md`): it turns the content bytes of a symbol into
//! the container's (`c`, `d`, `L`, `C`) and back. The container itself
//! (chapter 3) is built elsewhere.
//!
//! - Encoding: [`candidates`] computes every applicable codec, and [`select`]
//!   picks the one whose whole container is smallest (6.3).
//! - Decoding: [`decode`] applies the safety rules of 6.4: it checks `L`
//!   before allocating, produces exactly `L` bytes, bounds its work and never
//!   panics on malformed input.
//!
//! The crate is `no_std` and needs `alloc`, with or without the `brotli`
//! feature (codec 5, on by default).

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

mod bits;
pub mod codec0;
pub mod codec1;
pub mod codec2;
pub mod codec3;
pub mod codec4;
pub mod codec5;
pub mod dictionary;
pub mod leb128;

/// `MAX_CONTENT_LEN_V0` of 6.4: the absolute cap on `L` of one container in
/// format version 0 (16 MiB).
pub const MAX_CONTENT_LEN_V0: u32 = 16_777_216;

/// The output of the encoder and the input of the decoder (6.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coded {
    /// Codec ID `c`.
    pub codec: u32,
    /// Dictionary ID `d`; 0 when the codec takes none.
    pub dictionary: u32,
    /// Decoded length `L`. For codec 0 it equals `bytes.len()`.
    pub decoded_len: u32,
    /// Coded field `C`.
    pub bytes: Vec<u8>,
}

/// Decoding errors of 6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecError {
    /// `E_UNSUPPORTED_CODEC`: the codec ID is reserved or not implemented.
    UnsupportedCodec,
    /// `E_UNKNOWN_DICTIONARY`: `d` ≠ 0 is not in this reader's registry
    /// revision, or is in the private-use range.
    UnknownDictionary,
    /// `E_DICTIONARY_MISMATCH`: the registered kind of `d` does not match `c`.
    DictionaryMismatch,
    /// `E_TOO_LARGE`: `L` > the reader's limit.
    TooLarge,
    /// `E_MALFORMED`: `L` > [`MAX_CONTENT_LEN_V0`], or any other decoding
    /// failure.
    Malformed,
}

impl CodecError {
    /// The error name used by the specification.
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::UnsupportedCodec => "E_UNSUPPORTED_CODEC",
            Self::UnknownDictionary => "E_UNKNOWN_DICTIONARY",
            Self::DictionaryMismatch => "E_DICTIONARY_MISMATCH",
            Self::TooLarge => "E_TOO_LARGE",
            Self::Malformed => "E_MALFORMED",
        }
    }
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.spec_name())
    }
}

impl core::error::Error for CodecError {}

/// True when codec `codec` carries a dictionary ID in the container (6.2):
/// codecs 3 and 5. This is a property of the registry, so it holds for codec 5
/// even when the `brotli` feature is off.
pub const fn takes_dictionary(codec: u32) -> bool {
    matches!(codec, 3 | 5)
}

/// True when this build decodes codec `codec`: 0–4 always, 5 with the `brotli`
/// feature. A container reader stops with `E_UNSUPPORTED_CODEC` on any other
/// ID before it reads further fields (6.2).
pub const fn is_supported(codec: u32) -> bool {
    matches!(codec, 0..=4) || (codec == 5 && cfg!(feature = "brotli"))
}

/// Rule 1 of 6.4: `L` ≤ [`MAX_CONTENT_LEN_V0`], else `E_MALFORMED`, and then
/// `L` ≤ `limit`, else `E_TOO_LARGE`. Every decoder runs it before allocating.
///
/// # Errors
///
/// [`CodecError::Malformed`] or [`CodecError::TooLarge`] as above.
pub const fn check_decoded_len(decoded_len: u32, limit: u32) -> Result<(), CodecError> {
    if decoded_len > MAX_CONTENT_LEN_V0 {
        Err(CodecError::Malformed)
    } else if decoded_len > limit {
        Err(CodecError::TooLarge)
    } else {
        Ok(())
    }
}

/// `value` as a `usize`.
pub(crate) fn to_usize(value: u32) -> Result<usize, CodecError> {
    usize::try_from(value).map_err(|_| CodecError::Malformed)
}

/// True when a slice length equals `expected`.
pub(crate) fn len_matches(len: usize, expected: u64) -> bool {
    u64::try_from(len) == Ok(expected)
}

/// Which codecs [`candidates`] tries. Codec 0 (stored) is always a candidate
/// and has no switch.
///
/// The default tries every codec, which 6.3 requires of an encoder in its
/// default mode. Turning a codec off is the restricted mode 6.3 allows, for
/// example `EncodeOptions { brotli: false, ..EncodeOptions::default() }` (or
/// [`EncodeOptions::without_brotli`]) to skip the slowest codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::struct_excessive_bools)] // One switch per codec is the point.
pub struct EncodeOptions {
    /// Codec 1.
    pub digits: bool,
    /// Codec 2.
    pub upper_alphanumeric: bool,
    /// Codec 3, with model 0 and every registered short-text model.
    pub token_model: bool,
    /// Codec 4.
    pub hangul: bool,
    /// Codec 5. Has no effect without the `brotli` feature.
    pub brotli: bool,
}

impl EncodeOptions {
    /// Every codec: the default mode of 6.3.
    pub const ALL: Self = Self {
        digits: true,
        upper_alphanumeric: true,
        token_model: true,
        hangul: true,
        brotli: true,
    };

    /// Every codec except brotli.
    pub const fn without_brotli() -> Self {
        Self { brotli: false, ..Self::ALL }
    }
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self::ALL
    }
}

/// Every applicable candidate for `content` (6.3 rule 1), codec 0 first.
///
/// For codec 3 the candidates are model 0 and every registered short-text
/// model; for codec 5, dictionary 0 (no brotli prefix dictionary is registered
/// in 0.1). Returns an empty vector when `content` is longer than
/// [`MAX_CONTENT_LEN_V0`], because no container can carry it.
pub fn candidates(content: &[u8], options: &EncodeOptions) -> Vec<Coded> {
    let Ok(decoded_len) = u32::try_from(content.len()) else {
        return Vec::new();
    };
    if decoded_len > MAX_CONTENT_LEN_V0 {
        return Vec::new();
    }
    let coded = |codec, dictionary, bytes| Coded { codec, dictionary, decoded_len, bytes };
    let mut out = Vec::new();
    out.push(coded(codec0::ID, 0, codec0::encode(content)));
    if options.digits
        && let Some(bytes) = codec1::encode(content)
    {
        out.push(coded(codec1::ID, 0, bytes));
    }
    if options.upper_alphanumeric
        && let Some(bytes) = codec2::encode(content)
    {
        out.push(coded(codec2::ID, 0, bytes));
    }
    if options.token_model {
        out.push(coded(codec3::ID, 0, codec3::encode(content, &codec3::Model::model0())));
        for entry in dictionary::REGISTRY
            .iter()
            .filter(|entry| entry.kind == dictionary::DictionaryKind::ShortTextModel)
        {
            if let Ok(model) = codec3::Model::from_dictionary(entry.bytes) {
                out.push(coded(codec3::ID, entry.id, codec3::encode(content, &model)));
            }
        }
    }
    if options.hangul
        && let Some(bytes) = codec4::encode(content)
    {
        out.push(coded(codec4::ID, 0, bytes));
    }
    if options.brotli
        && let Some(bytes) = codec5::encode(content)
    {
        out.push(coded(codec5::ID, 0, bytes));
    }
    out
}

/// Picks the candidate whose serialised container is smallest (6.3 rules 2–4).
///
/// `container_size` returns the container size in bytes for a candidate,
/// CRC-32C included and padding excluded. Ties go to the lowest codec ID, then
/// to the lowest dictionary ID. Returns `None` only for an empty `candidates`.
/// Rule 4 (stored is always a candidate) holds when `candidates` comes from
/// [`candidates`].
pub fn select<F: Fn(&Coded) -> usize>(candidates: Vec<Coded>, container_size: F) -> Option<Coded> {
    candidates
        .into_iter()
        .map(|candidate| (container_size(&candidate), candidate))
        .min_by(|(size_a, a), (size_b, b)| {
            size_a.cmp(size_b).then(a.codec.cmp(&b.codec)).then(a.dictionary.cmp(&b.dictionary))
        })
        .map(|(_, candidate)| candidate)
}

/// Decodes coded field `bytes` with codec `codec`, dictionary `dictionary` and
/// decoded length `decoded_len` into exactly `decoded_len` bytes, applying
/// every rule of 6.4 with reader limit `limit`.
///
/// Checks run in container order: the codec ID, then the dictionary ID, then
/// `L` against [`MAX_CONTENT_LEN_V0`] and `limit`, then the codec's own length
/// check, and only then is the output allocated. For codec 0 pass
/// `decoded_len` = `bytes.len()`. For a codec that takes no dictionary pass
/// `dictionary` = 0.
///
/// # Errors
///
/// - [`CodecError::UnsupportedCodec`]: `codec` is reserved (6–15, 16 or more)
///   or is 5 without the `brotli` feature.
/// - [`CodecError::UnknownDictionary`]: `dictionary` ≠ 0 is not registered, or
///   is private-use.
/// - [`CodecError::DictionaryMismatch`]: `dictionary` is registered with a
///   kind that does not match `codec`.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
/// - [`CodecError::Malformed`]: `decoded_len` > [`MAX_CONTENT_LEN_V0`], a
///   non-zero `dictionary` for a codec that takes none, or any decoding failure.
pub fn decode(
    codec: u32,
    dictionary: u32,
    decoded_len: u32,
    bytes: &[u8],
    limit: u32,
) -> Result<Vec<u8>, CodecError> {
    if !is_supported(codec) {
        return Err(CodecError::UnsupportedCodec);
    }
    if !takes_dictionary(codec) && dictionary != 0 {
        return Err(CodecError::Malformed);
    }
    let entry =
        if takes_dictionary(codec) { dictionary::resolve(codec, dictionary)? } else { None };
    check_decoded_len(decoded_len, limit)?;
    match codec {
        codec0::ID => codec0::decode(decoded_len, bytes, limit),
        codec1::ID => codec1::decode(decoded_len, bytes, limit),
        codec2::ID => codec2::decode(decoded_len, bytes, limit),
        codec3::ID => match entry {
            None => codec3::decode(&codec3::Model::model0(), decoded_len, bytes, limit),
            Some(entry) => {
                let model = codec3::Model::from_dictionary(entry.bytes)?;
                codec3::decode(&model, decoded_len, bytes, limit)
            }
        },
        codec4::ID => codec4::decode(decoded_len, bytes, limit),
        codec5::ID => match entry {
            None => codec5::decode(decoded_len, bytes, limit),
            // No brotli prefix dictionary is registered in 0.1; decoding with
            // one is added together with its registration.
            Some(_) => Err(CodecError::UnknownDictionary),
        },
        _ => Err(CodecError::UnsupportedCodec),
    }
}
