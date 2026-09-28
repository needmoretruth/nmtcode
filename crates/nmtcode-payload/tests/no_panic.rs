//! `decode` never panics (6.4 rule 5) and, when it succeeds, returns exactly
//! `L` bytes (rule 4), for arbitrary and for mutated valid input.

#![allow(missing_docs)]

use nmtcode_payload::{EncodeOptions, MAX_CONTENT_LEN_V0, candidates, codec4, decode};
use proptest::prelude::*;

const DICTIONARIES: [u32; 9] = [0, 1, 2, 127, 128, 16_384, 32_767, 32_768, u32::MAX];

fn decoded_len() -> impl Strategy<Value = u32> {
    prop_oneof![
        8 => 0u32..=512,
        1 => 0u32..=70_000,
        1 => Just(MAX_CONTENT_LEN_V0),
        1 => any::<u32>(),
    ]
}

fn limit() -> impl Strategy<Value = u32> {
    prop_oneof![Just(MAX_CONTENT_LEN_V0), Just(u32::MAX), 0u32..=1024, any::<u32>(),]
}

fn check(codec: u32, dictionary: u32, len: u32, bytes: &[u8], limit: u32) {
    if let Ok(out) = decode(codec, dictionary, len, bytes, limit) {
        assert_eq!(out.len() as u64, u64::from(len));
        assert!(len <= MAX_CONTENT_LEN_V0 && len <= limit);
        if codec == 4 {
            assert!(std::str::from_utf8(&out).is_ok());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn arbitrary_input(
        codec in 0u32..=20,
        dictionary in prop::sample::select(DICTIONARIES.to_vec()),
        len in decoded_len(),
        bytes in prop::collection::vec(any::<u8>(), 0..96),
        limit in limit(),
    ) {
        check(codec, dictionary, len, &bytes, limit);
    }

    /// Codec IDs near and far, dictionary 0: every codec's inner decoder runs.
    #[test]
    fn arbitrary_input_dictionary_0(
        codec in prop_oneof![0u32..=20, any::<u32>()],
        len in 0u32..=256,
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        check(codec, 0, len, &bytes, MAX_CONTENT_LEN_V0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Valid coded fields with bytes flipped, cut or added, decoded under every
    /// codec with the true L and L ± 1.
    #[test]
    fn mutated_valid_input(
        content in prop_oneof![
            prop::collection::vec(any::<u8>(), 0..80),
            "[가-힣 A-Za-z0-9.,?!/:]{0,30}".prop_map(String::into_bytes),
            "[0-9]{0,40}".prop_map(String::into_bytes),
        ],
        edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>(), 0u8..4), 1..4),
    ) {
        for coded in candidates(&content, &EncodeOptions::default()) {
            let mut bytes = coded.bytes.clone();
            for (index, value, kind) in &edits {
                match kind {
                    0 if !bytes.is_empty() => {
                        let i = index.index(bytes.len());
                        bytes[i] ^= value | 1;
                    }
                    1 if !bytes.is_empty() => {
                        bytes.truncate(index.index(bytes.len()));
                    }
                    2 => bytes.push(*value),
                    _ => bytes.insert(index.index(bytes.len() + 1), *value),
                }
            }
            for len in [coded.decoded_len.saturating_sub(1), coded.decoded_len, coded.decoded_len + 1] {
                for codec in 0..=6 {
                    check(codec, 0, len, &bytes, MAX_CONTENT_LEN_V0);
                }
            }
        }
    }
}

#[test]
fn codec4_output_is_always_utf8() {
    // Every 14-bit H unit alone, with L from 1 to 4.
    for unit in 0u32..(1 << 14) {
        // Mode bit 0, then the unit, then 1 padding bit.
        let bits = u16::try_from(unit << 1).unwrap();
        let bytes = bits.to_be_bytes();
        for len in 1..=4 {
            if let Ok(out) = codec4::decode(len, &bytes, MAX_CONTENT_LEN_V0) {
                assert!(std::str::from_utf8(&out).is_ok());
            }
        }
    }
}
