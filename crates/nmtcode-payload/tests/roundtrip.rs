//! Round trips for every codec on random applicable content.

#![allow(missing_docs)]

mod common;

use nmtcode_payload::{
    EncodeOptions, MAX_CONTENT_LEN_V0, candidates, codec0, codec1, codec2, codec3, codec4, codec5,
    decode,
};
use proptest::prelude::*;

fn len32(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.len()).unwrap()
}

/// Content assembled from tokens, URL punctuation and arbitrary bytes, so the
/// reference parse meets tokens often.
fn tokenish() -> impl Strategy<Value = Vec<u8>> {
    let piece = prop_oneof![
        (0usize..codec3::TOKEN_COUNT).prop_map(|i| codec3::TOKEN_TABLE_V0[i].to_vec()),
        "[a-z0-9/.?=&:-]{1,6}".prop_map(String::into_bytes),
        any::<u8>().prop_map(|b| vec![b]),
    ];
    prop::collection::vec(piece, 0..24).prop_map(|pieces| pieces.concat())
}

/// Korean text with spaces, punctuation, ASCII runs and escapes.
fn korean_text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        4 => "[가-힣]{1,5}",
        2 => Just(" ".to_string()),
        1 => "[.,?!] ",
        2 => "[A-Za-z0-9 ]{1,8}",
        1 => "[ㄱ-ㆎ“”…※、。「」]{1,3}",
        1 => "[\u{00A0}-\u{00FF}\u{0100}-\u{024F}\u{4E00}-\u{4E20}]{1,2}",
        1 => "[\u{1F600}-\u{1F64F}\u{10000}-\u{10010}]{1,2}",
        1 => "[\u{0}-\u{1F}\u{7F}]",
    ];
    prop::collection::vec(piece, 0..20).prop_map(|pieces| pieces.concat())
}

proptest! {
    #[test]
    fn codec0_round_trip(content in prop::collection::vec(any::<u8>(), 0..300)) {
        let coded = codec0::encode(&content);
        prop_assert_eq!(decode(0, 0, len32(&coded), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec1_round_trip(content in "[0-9]{0,300}") {
        let content = content.into_bytes();
        let coded = codec1::encode(&content).unwrap();
        prop_assert_eq!(coded.len() as u64, codec1::coded_len(content.len() as u64));
        prop_assert_eq!(decode(1, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec2_round_trip(content in "[0-9A-Z $%*+./:-]{0,300}") {
        let content = content.into_bytes();
        let coded = codec2::encode(&content).unwrap();
        prop_assert_eq!(coded.len() as u64, codec2::coded_len(content.len() as u64));
        prop_assert_eq!(decode(2, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec3_round_trip_arbitrary(content in prop::collection::vec(any::<u8>(), 0..300)) {
        let coded = codec3::encode(&content, &codec3::Model::model0());
        prop_assert!(coded.last() != Some(&0));
        prop_assert_eq!(decode(3, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec3_round_trip_tokens(content in tokenish()) {
        let coded = codec3::encode(&content, &codec3::Model::model0());
        prop_assert_eq!(decode(3, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    /// The decoder accepts any parse (6.8.6): an all-literal parse decodes to
    /// the same content.
    #[test]
    fn codec3_any_parse(content in tokenish()) {
        let literals: Vec<u16> = content.iter().map(|&b| u16::from(b)).collect();
        let coded = codec3::encode_symbols(&literals, &codec3::Model::model0()).unwrap();
        prop_assert_eq!(decode(3, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec3_round_trip_context_model(content in tokenish()) {
        let model = codec3::Model::from_dictionary(&common::order2_model()).unwrap();
        let coded = codec3::encode(&content, &model);
        prop_assert_eq!(
            codec3::decode(&model, len32(&content), &coded, MAX_CONTENT_LEN_V0),
            Ok(content)
        );
    }

    #[test]
    fn codec4_round_trip_korean(content in korean_text()) {
        let content = content.into_bytes();
        let coded = codec4::encode(&content).unwrap();
        prop_assert!(7 * content.len() <= 16 * coded.len());
        prop_assert_eq!(decode(4, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec4_round_trip_any_text(content in any::<String>()) {
        let content = content.into_bytes();
        let coded = codec4::encode(&content).unwrap();
        prop_assert_eq!(decode(4, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
    }

    #[test]
    fn codec4_rejects_invalid_utf8(content in prop::collection::vec(any::<u8>(), 0..64)) {
        prop_assert_eq!(codec4::encode(&content).is_some(), std::str::from_utf8(&content).is_ok());
    }

    #[test]
    fn every_candidate_round_trips(content in prop_oneof![
        prop::collection::vec(any::<u8>(), 0..120),
        tokenish(),
        korean_text().prop_map(String::into_bytes),
        "[0-9]{0,40}".prop_map(String::into_bytes),
    ]) {
        for coded in candidates(&content, &EncodeOptions::default()) {
            prop_assert_eq!(coded.decoded_len, len32(&content));
            let back = decode(coded.codec, coded.dictionary, coded.decoded_len, &coded.bytes, MAX_CONTENT_LEN_V0);
            prop_assert_eq!(back.as_deref(), Ok(&content[..]), "codec {}", coded.codec);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn codec5_round_trip(content in prop_oneof![
        prop::collection::vec(any::<u8>(), 0..600),
        tokenish(),
        korean_text().prop_map(String::into_bytes),
    ]) {
        if let Some(coded) = codec5::encode(&content) {
            prop_assert_eq!(decode(5, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
        } else {
            prop_assert!(!cfg!(feature = "brotli"));
        }
    }

    #[test]
    fn codec5_round_trip_any_setting(
        content in tokenish(),
        quality in 0u32..=11,
        window in 10u32..=24,
    ) {
        if let Some(coded) = codec5::encode_with(&content, quality, window) {
            prop_assert_eq!(decode(5, 0, len32(&content), &coded, MAX_CONTENT_LEN_V0), Ok(content));
        } else {
            prop_assert!(!cfg!(feature = "brotli"));
        }
    }
}

/// A model-0 dictionary in the 6.8.4 format codes exactly like model 0.
#[test]
fn model0_as_dictionary_matches_model0() {
    let model = codec3::Model::from_dictionary(&common::model0_as_dictionary()).unwrap();
    assert!(!model.is_model0());
    for content in [
        &b"https://github.com/needmoretruth/nmtcode"[..],
        b"",
        b"\x00\x00\x00",
        "안녕하세요".as_bytes(),
    ] {
        assert_eq!(
            codec3::encode(content, &model),
            codec3::encode(content, &codec3::Model::model0())
        );
    }
}

#[test]
fn large_contents_round_trip() {
    let content: Vec<u8> = (0..200_000u32)
        .map(|i| b"https://example.com/path?q=1&id=42 "[(i % 35) as usize])
        .collect();
    for coded in candidates(&content, &EncodeOptions::default()) {
        let back = decode(
            coded.codec,
            coded.dictionary,
            coded.decoded_len,
            &coded.bytes,
            MAX_CONTENT_LEN_V0,
        );
        assert_eq!(back.as_deref(), Ok(&content[..]), "codec {}", coded.codec);
    }
    let digits: Vec<u8> = (0..100_001u32).map(|i| b'0' + (i % 10) as u8).collect();
    let coded = codec1::encode(&digits).unwrap();
    assert_eq!(decode(1, 0, len32(&digits), &coded, MAX_CONTENT_LEN_V0), Ok(digits));
}
