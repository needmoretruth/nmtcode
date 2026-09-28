//! The dictionary registry of 6.11.
//!
//! A symbol names a dictionary by its ID; every reader of a format version
//! carries the registered bytes. In specification 0.1 only ID 0 ("none")
//! exists, so [`REGISTRY`] is empty and every other ID fails with
//! [`CodecError::UnknownDictionary`].
//!
//! Adding a dictionary later means adding one [`Dictionary`] entry to
//! [`REGISTRY`]; [`resolve`] already checks that its kind matches the codec
//! (6.2) and the test suite checks its ID range and SHA-256 (6.11 rule 3).

use core::ops::RangeInclusive;

use crate::CodecError;

/// What a registered dictionary is for (6.11, field "Kind").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DictionaryKind {
    /// Bytes that a codec-5 (brotli) decoder treats as already output (6.10).
    BrotliPrefix,
    /// A codec-3 context model in the byte format of 6.8.4.
    ShortTextModel,
}

/// One entry of the registry (6.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dictionary {
    /// The dictionary ID, 1–127 for dictionaries registered by the specification.
    pub id: u32,
    /// Which codec may use it.
    pub kind: DictionaryKind,
    /// The exact bytes.
    pub bytes: &'static [u8],
    /// SHA-256 of `bytes`.
    pub sha256: [u8; 32],
    /// The specification version that registered it.
    pub added_in: &'static str,
}

/// The specification version whose registry this crate carries (6.11 rule 4).
pub const REGISTRY_REVISION: &str = "0.1";

/// Every dictionary registered up to [`REGISTRY_REVISION`]. Empty in 0.1.
pub const REGISTRY: &[Dictionary] = &[];

/// IDs registered by the specification.
pub const REGISTERED_IDS: RangeInclusive<u32> = 1..=127;

/// IDs for private use by a closed application; a general reader fails on them.
pub const PRIVATE_USE_IDS: RangeInclusive<u32> = 16_384..=32_767;

/// The kind a codec's dictionary ID must name, or `None` for a codec that
/// takes no dictionary.
pub const fn kind_for_codec(codec: u32) -> Option<DictionaryKind> {
    match codec {
        3 => Some(DictionaryKind::ShortTextModel),
        5 => Some(DictionaryKind::BrotliPrefix),
        _ => None,
    }
}

/// Resolves dictionary ID `id` for `codec` against [`REGISTRY`].
///
/// Returns `Ok(None)` for ID 0 (the codec's built-in model or dictionary) and
/// `Ok(Some(entry))` for a registered dictionary of the kind `codec` uses.
///
/// # Errors
///
/// - [`CodecError::UnknownDictionary`]: `id` is in the private-use range, or
///   is not in this reader's registry revision.
/// - [`CodecError::DictionaryMismatch`]: `id` is registered with a kind that
///   does not match `codec`, or `codec` takes no dictionary at all.
pub fn resolve(codec: u32, id: u32) -> Result<Option<&'static Dictionary>, CodecError> {
    resolve_in(REGISTRY, codec, id)
}

/// [`resolve`] against a given registry.
fn resolve_in(
    registry: &'static [Dictionary],
    codec: u32,
    id: u32,
) -> Result<Option<&'static Dictionary>, CodecError> {
    if id == 0 {
        return Ok(None);
    }
    if PRIVATE_USE_IDS.contains(&id) {
        return Err(CodecError::UnknownDictionary);
    }
    let entry =
        registry.iter().find(|entry| entry.id == id).ok_or(CodecError::UnknownDictionary)?;
    if kind_for_codec(codec) == Some(entry.kind) {
        Ok(Some(entry))
    } else {
        Err(CodecError::DictionaryMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::{Dictionary, DictionaryKind, resolve, resolve_in};
    use crate::CodecError;

    const TEST_REGISTRY: &[Dictionary] = &[
        Dictionary {
            id: 1,
            kind: DictionaryKind::BrotliPrefix,
            bytes: b"https://",
            sha256: [0; 32],
            added_in: "test",
        },
        Dictionary {
            id: 2,
            kind: DictionaryKind::ShortTextModel,
            bytes: b"",
            sha256: [0; 32],
            added_in: "test",
        },
        // A private-use ID must fail even if a registry listed it.
        Dictionary {
            id: 16_384,
            kind: DictionaryKind::ShortTextModel,
            bytes: b"",
            sha256: [0; 32],
            added_in: "test",
        },
    ];

    #[test]
    fn private_use_ids_fail_even_when_listed() {
        assert_eq!(resolve_in(TEST_REGISTRY, 3, 16_384), Err(CodecError::UnknownDictionary));
    }

    #[test]
    fn kind_check_with_registered_entries() {
        assert_eq!(resolve_in(TEST_REGISTRY, 5, 0), Ok(None));
        assert_eq!(resolve_in(TEST_REGISTRY, 5, 1), Ok(Some(&TEST_REGISTRY[0])));
        assert_eq!(resolve_in(TEST_REGISTRY, 3, 2), Ok(Some(&TEST_REGISTRY[1])));
        assert_eq!(resolve_in(TEST_REGISTRY, 3, 1), Err(CodecError::DictionaryMismatch));
        assert_eq!(resolve_in(TEST_REGISTRY, 5, 2), Err(CodecError::DictionaryMismatch));
        assert_eq!(resolve_in(TEST_REGISTRY, 0, 1), Err(CodecError::DictionaryMismatch));
        assert_eq!(resolve_in(TEST_REGISTRY, 5, 3), Err(CodecError::UnknownDictionary));
    }

    #[test]
    fn empty_registry_of_0_1() {
        for codec in [3, 5] {
            assert_eq!(resolve(codec, 0), Ok(None));
            for id in [1, 2, 127, 128, 16_383, 16_384, 20_000, 32_767, 32_768, u32::MAX] {
                assert_eq!(resolve(codec, id), Err(CodecError::UnknownDictionary));
            }
        }
    }
}
