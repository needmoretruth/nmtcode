//! Worked examples and transcription checks of chapter 6.

#![allow(missing_docs)]

use nmtcode_payload::{
    Coded, EncodeOptions, MAX_CONTENT_LEN_V0, candidates, codec1, codec2, codec3, codec4, codec5,
    decode, dictionary, leb128, select, takes_dictionary,
};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        write!(s, "{b:02x}").unwrap();
        s
    })
}

/// The whole container of chapter 3 (3.2) for a single-record container with
/// content type 1: lead byte, format echo byte, body length, codec escape,
/// dictionary ID, decoded length, content type, content, CRC-32C.
fn container_size(coded: &Coded) -> usize {
    let mut header = 1; // content type 1
    if coded.codec >= 15 {
        header += leb128::encoded_len(coded.codec);
    }
    if takes_dictionary(coded.codec) {
        header += leb128::encoded_len(coded.dictionary);
    }
    if coded.codec != 0 {
        header += leb128::encoded_len(coded.decoded_len);
    }
    let body = header + coded.bytes.len();
    2 + leb128::encoded_len(u32::try_from(body).unwrap()) + body + 4
}

#[test]
fn token_table_transcription_6_8_2() {
    let mut serialised = Vec::new();
    for token in codec3::TOKEN_TABLE_V0 {
        serialised.push(u8::try_from(token.len()).unwrap());
        serialised.extend_from_slice(token);
    }
    assert_eq!(serialised.len(), 1794);
    assert_eq!(
        lower_hex(&Sha256::digest(&serialised)),
        "a8e0f1be8a427a782b1c2e6179c1876a6d1690a74224f946c487e1ab803e3717"
    );
    assert!(codec3::TOKEN_TABLE_V0.iter().all(|t| t.is_ascii() && !t.is_empty()));
    let mut sorted = codec3::TOKEN_TABLE_V0.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), codec3::TOKEN_COUNT);
    assert_eq!(codec3::MAX_TOKEN_LEN, 38);
}

#[test]
fn model0_weights_6_8_3() {
    let f = codec3::model0_frequency;
    let total: u32 = (0..codec3::ALPHABET_SIZE).map(f).sum();
    assert_eq!(total, 65_536);
    let count =
        |value: u32, range: std::ops::Range<usize>| range.filter(|&s| f(s) == value).count();
    assert_eq!(count(1300, 0..256), 26);
    assert_eq!(count(700, 0..256), 10);
    assert_eq!(count(250, 0..256), 26);
    assert_eq!(count(560, 0..256), 12);
    assert_eq!(f(0x2F), 864);
    assert_eq!(f(0x20), 560);
    assert_eq!(count(60, 0..256), 23);
    assert_eq!(count(4, 0..256), 158);
    assert_eq!(count(36, 256..496), 240);
    // cum values quoted in 6.12.3.
    let cum = |s: usize| (0..s).map(f).sum::<u32>();
    assert_eq!(cum(usize::from(b'/')), 4696);
    assert_eq!(cum(usize::from(b'a')), 21_840);
    assert_eq!(cum(usize::from(b'n')), 38_740);
    assert_eq!(cum(usize::from(b'e')), 27_040);
    assert_eq!(cum(258), 56_968);
    assert_eq!(cum(300), 58_480);
}

#[test]
fn codec1_example_6_12_1() {
    let c = codec1::encode(b"0123456789").unwrap();
    assert_eq!(hex(&c), "03 15 9A 9A 40");
    assert_eq!(codec1::bit_len(10), 34);
    assert_eq!(decode(1, 0, 10, &c, MAX_CONTENT_LEN_V0).unwrap(), b"0123456789");
}

#[test]
fn codec2_example_6_12_2() {
    let content = b"HTTPS://EXAMPLE.COM/ABC";
    let c = codec2::encode(content).unwrap();
    assert_eq!(hex(&c), "63 54 CA 8C 7B A5 2E 76 23 D2 A0 46 90 24 E6 98");
    assert_eq!(codec2::bit_len(23), 127);
    assert_eq!(decode(2, 0, 23, &c, MAX_CONTENT_LEN_V0).unwrap(), content);
}

#[test]
fn codec3_example_6_12_3() {
    let content = b"https://github.com/needmoretruth/nmtcode";
    let symbols = codec3::reference_parse(content);
    assert_eq!(symbols.len(), 24);
    assert_eq!(&symbols[..3], &[258, 300, 47]);
    assert_eq!(symbols[16], 47);
    let literals: Vec<u8> =
        symbols[3..16].iter().chain(&symbols[17..]).map(|&s| u8::try_from(s).unwrap()).collect();
    assert_eq!(literals, b"needmoretruthnmtcode");
    let c = codec3::encode(content, &codec3::Model::model0());
    assert_eq!(hex(&c), "DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91");
    assert_eq!(decode(3, 0, 40, &c, MAX_CONTENT_LEN_V0).unwrap(), content);
}

#[test]
fn codec4_example_6_12_4() {
    let content = "안녕하세요 NMT Code".as_bytes();
    assert_eq!(content.len(), 24);
    let c = codec4::encode(content).unwrap();
    assert_eq!(hex(&c), "32 90 2A AD 2B 0A 9C 59 2F FF 54 E9 B5 10 43 DF 93 28");
    assert_eq!(decode(4, 0, 24, &c, MAX_CONTENT_LEN_V0).unwrap(), content);
}

/// 6.12.5: `Lc` / container size per codec (`None` = not applicable) and the
/// chosen codec. Codec 5 is informative: its size depends on the brotli
/// encoder, so only its applicability and that it does not win are checked.
#[test]
fn candidate_sizes_and_selection_6_12_5() {
    type Row = (&'static str, [Option<(usize, usize)>; 5], u32);
    let rows: [Row; 4] = [
        (
            "0123456789",
            [Some((10, 18)), Some((5, 14)), Some((7, 16)), Some((9, 19)), Some((9, 18))],
            1,
        ),
        (
            "HTTPS://EXAMPLE.COM/ABC",
            [Some((23, 31)), None, Some((16, 25)), Some((16, 26)), Some((21, 30))],
            2,
        ),
        (
            "https://github.com/needmoretruth/nmtcode",
            [Some((40, 48)), None, None, Some((19, 29)), Some((36, 45))],
            3,
        ),
        ("안녕하세요 NMT Code", [Some((24, 32)), None, None, Some((35, 45)), Some((18, 27))], 4),
    ];
    for (content, expected, chosen) in rows {
        let all = candidates(content.as_bytes(), &EncodeOptions::default());
        for (codec, want) in (0u32..).zip(expected) {
            let got = all
                .iter()
                .find(|c| c.codec == codec && c.dictionary == 0)
                .map(|c| (c.bytes.len(), container_size(c)));
            assert_eq!(got, want, "{content:?} codec {codec}");
        }
        let winner = select(all.clone(), container_size).unwrap();
        assert_eq!(winner.codec, chosen, "{content:?}");
        match all.iter().find(|c| c.codec == 5) {
            Some(brotli) => {
                println!(
                    "{content:?}: codec 5 gives {} / {} bytes",
                    brotli.bytes.len(),
                    container_size(brotli)
                );
                assert!(container_size(brotli) > container_size(&winner));
            }
            None => assert!(!nmtcode_payload::is_supported(5)),
        }
        for coded in all {
            let back = decode(
                coded.codec,
                coded.dictionary,
                coded.decoded_len,
                &coded.bytes,
                MAX_CONTENT_LEN_V0,
            );
            assert_eq!(back.as_deref(), Ok(content.as_bytes()));
        }
    }
}

#[test]
fn selection_tie_breaks_6_3() {
    let coded =
        |codec, dictionary, len| Coded { codec, dictionary, decoded_len: 0, bytes: vec![0; len] };
    let same_size = |_: &Coded| 10;
    let picked = select(vec![coded(3, 2, 1), coded(2, 0, 1), coded(3, 1, 1)], same_size).unwrap();
    assert_eq!((picked.codec, picked.dictionary), (2, 0));
    let picked = select(vec![coded(3, 2, 1), coded(3, 1, 1)], same_size).unwrap();
    assert_eq!((picked.codec, picked.dictionary), (3, 1));
    let by_len = |c: &Coded| c.bytes.len();
    let picked = select(vec![coded(0, 0, 5), coded(4, 0, 4), coded(1, 0, 4)], by_len).unwrap();
    assert_eq!(picked.codec, 1);
    assert_eq!(select(Vec::new(), by_len), None);
}

#[test]
fn stored_wins_when_nothing_is_shorter_6_3_rule_4() {
    let content = [0xFFu8, 0x00, 0x80];
    let all = candidates(&content, &EncodeOptions::default());
    assert_eq!(all[0].codec, 0);
    assert_eq!(select(all, container_size).unwrap().codec, 0);
}

#[test]
fn restricted_candidate_sets_6_3() {
    let content = b"0123456789";
    let codecs = |options: &EncodeOptions| -> Vec<u32> {
        candidates(content, options).iter().map(|c| c.codec).collect()
    };
    let full = codecs(&EncodeOptions::default());
    if cfg!(feature = "brotli") {
        assert_eq!(full, [0, 1, 2, 3, 4, 5]);
    } else {
        assert_eq!(full, [0, 1, 2, 3, 4]);
    }
    assert_eq!(codecs(&EncodeOptions::without_brotli()), [0, 1, 2, 3, 4]);
    let only_stored = EncodeOptions {
        digits: false,
        upper_alphanumeric: false,
        token_model: false,
        hangul: false,
        brotli: false,
    };
    assert_eq!(codecs(&only_stored), [0]);
}

#[test]
fn applicability_6_2() {
    let codecs = |content: &[u8]| -> Vec<u32> {
        candidates(content, &EncodeOptions::without_brotli()).iter().map(|c| c.codec).collect()
    };
    assert_eq!(codecs(b"123"), [0, 1, 2, 3, 4]);
    assert_eq!(codecs(b"ABC 123"), [0, 2, 3, 4]);
    assert_eq!(codecs(b"abc"), [0, 3, 4]);
    assert_eq!(codecs(&[0xC3, 0x28]), [0, 3]);
    assert_eq!(codecs(&[0xED, 0xA0, 0x80]), [0, 3]); // an encoded surrogate
    assert_eq!(codecs(&[0xC0, 0x80]), [0, 3]); // an overlong form
}

#[test]
fn codec5_settings() {
    assert_eq!(codec5::QUALITY, 11);
    if cfg!(feature = "brotli") {
        // Empty content: WBITS 16 (bit 0), ISLAST, ISLASTEMPTY, zero padding.
        assert_eq!(codec5::encode(b"").unwrap(), [0x06]);
    } else {
        assert_eq!(codec5::encode(b""), None);
    }
}

#[test]
fn registry_entries_6_11() {
    // Rule 3: every carried dictionary matches its SHA-256; IDs are in the
    // specification's range and unique. Vacuous in 0.1, binding once entries
    // are added.
    let mut ids = Vec::new();
    for entry in dictionary::REGISTRY {
        assert!(dictionary::REGISTERED_IDS.contains(&entry.id));
        assert_eq!(Sha256::digest(entry.bytes).as_slice(), entry.sha256);
        ids.push(entry.id);
        if entry.kind == dictionary::DictionaryKind::ShortTextModel {
            assert!(codec3::Model::from_dictionary(entry.bytes).is_ok());
        }
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), dictionary::REGISTRY.len());
    assert_eq!(dictionary::REGISTRY_REVISION, "0.2");
    assert!(dictionary::REGISTRY.is_empty());
}

#[test]
fn leb128_examples_3_1() {
    for (value, bytes) in [
        (0u32, &[0x00u8][..]),
        (127, &[0x7F]),
        (128, &[0x80, 0x01]),
        (300, &[0xAC, 0x02]),
        (16_384, &[0x80, 0x80, 0x01]),
        (u32::MAX, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
    ] {
        let mut out = Vec::new();
        leb128::write(value, &mut out);
        assert_eq!(out, bytes);
        assert_eq!(leb128::encoded_len(value), bytes.len());
        assert_eq!(leb128::read(bytes), Ok((value, bytes.len())));
    }
}
