//! Round trips through the whole pipeline: encode, then decode the module grid.

use nmtcode::{
    AspectRatio, CodecOptions, ContentType, DecodeOptions, Decoded, EncodeError, EncodeOptions,
    Outcome, PresentAs, Profile, Record, RecordForm, SizeConstraints, SizeRule, Symbol, capacity,
    decode, encode, encode_file, encode_text, encode_url,
};
use proptest::prelude::*;

/// Every side from 20 to `max` in steps of 4.
fn side(max: u32) -> impl Strategy<Value = u32> {
    (5..=max / 4).prop_map(|k| 4 * k)
}

fn size_rule() -> impl Strategy<Value = SizeRule> {
    prop_oneof![
        Just(SizeRule::SmallestSquare),
        Just(SizeRule::Recommended),
        (side(160), side(160)).prop_map(|(width, height)| SizeRule::Exact { width, height }),
        (1u32..=5, 1u32..=5, proptest::option::of(side(400)), proptest::option::of(side(400)))
            .prop_map(|(a, b, max_width, max_height)| {
                SizeRule::Constrained(SizeConstraints {
                    aspect_ratio: AspectRatio::new(a, b),
                    max_width,
                    max_height,
                })
            }),
        (proptest::option::of(side(200)), proptest::option::of(side(200))).prop_map(
            |(max_width, max_height)| SizeRule::Constrained(SizeConstraints {
                aspect_ratio: None,
                max_width,
                max_height,
            })
        ),
    ]
}

fn options() -> impl Strategy<Value = EncodeOptions> {
    (0u8..=3, size_rule()).prop_map(|(level, size)| EncodeOptions {
        level: Some(level),
        size,
        ..EncodeOptions::default()
    })
}

/// Encodes and decodes, and checks what the symbol reports about itself. `None` when the size
/// rule allows no size for this content, which must then be a size error.
fn round_trip(
    result: Result<Symbol, EncodeError>,
    options: &EncodeOptions,
) -> Option<(Symbol, Decoded)> {
    let symbol = match result {
        Ok(symbol) => symbol,
        Err(EncodeError::DoesNotFit { width, height, container_len, capacity: k, .. }) => {
            assert_eq!(options.size, SizeRule::Exact { width, height });
            assert!(container_len > k);
            assert_eq!(capacity(width, height, options.effective_level()), Some(k));
            return None;
        }
        Err(EncodeError::NoSize { .. }) => {
            assert!(matches!(options.size, SizeRule::Constrained(_)));
            return None;
        }
        Err(other) => panic!("unexpected {other:?}"),
    };
    assert_eq!(symbol.level(), options.effective_level());
    assert!(symbol.container_len() <= symbol.capacity());
    assert_eq!(symbol.message().len(), symbol.capacity());
    assert_eq!(symbol.grid().width(), symbol.width());
    assert_eq!(symbol.grid().height(), symbol.height());
    match options.size {
        SizeRule::SmallestSquare => assert_eq!(symbol.width(), symbol.height()),
        SizeRule::Recommended => {
            assert!(symbol.width() <= 2 * symbol.height() && symbol.height() <= 2 * symbol.width());
        }
        SizeRule::Exact { width, height } => {
            assert_eq!((symbol.width(), symbol.height()), (width, height));
        }
        SizeRule::Constrained(constraints) => {
            assert!(constraints.max_width.is_none_or(|w| symbol.width() <= w));
            assert!(constraints.max_height.is_none_or(|h| symbol.height() <= h));
        }
    }
    let decoded = decode(symbol.grid(), &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.outcome, Outcome::Presented);
    assert_eq!(decoded.notice, None);
    assert_eq!(decoded.corrected, 0);
    assert_eq!(decoded.codec, symbol.codec());
    assert_eq!(decoded.dictionary, symbol.dictionary());
    assert_eq!(decoded.record_form, symbol.record_form());
    assert_eq!(&decoded.format, symbol.format());
    Some((symbol, decoded))
}

fn check_text(text: &str, options: &EncodeOptions) -> Option<u32> {
    let (symbol, decoded) = round_trip(encode_text(text, options), options)?;
    assert_eq!(decoded.records.len(), 1);
    let record = &decoded.records[0];
    assert_eq!(record.content_type, ContentType::TEXT);
    assert_eq!(record.present_as, PresentAs::Text);
    assert_eq!(record.text(), Some(text));
    Some(symbol.codec())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn any_text(text in any::<String>(), options in options()) {
        check_text(&text, &options);
    }

    #[test]
    fn urls(path in "[a-z0-9./?=&_-]{0,120}", options in options()) {
        let url = format!("https://example.org/{path}");
        if let Some((_, decoded)) = round_trip(encode_url(&url, &options), &options) {
            prop_assert_eq!(decoded.records[0].present_as, PresentAs::Url);
            prop_assert_eq!(decoded.records[0].text(), Some(url.as_str()));
        }
    }

    #[test]
    fn korean_text(text in "[가-힣 ]{1,80}[ a-zA-Z0-9.,]{0,20}", options in options()) {
        check_text(&text, &options);
    }

    #[test]
    fn digits(text in "[0-9]{0,300}", options in options()) {
        check_text(&text, &options);
    }

    #[test]
    fn upper_case_alphanumerics(text in "[0-9A-Z $%*+./:-]{0,200}", options in options()) {
        check_text(&text, &options);
    }

    #[test]
    fn binary_files(
        name in "[a-zA-Z0-9_.-]{0,40}",
        bytes in proptest::collection::vec(any::<u8>(), 0..600),
        options in options(),
    ) {
        if let Some((_, decoded)) = round_trip(encode_file(&name, &bytes, &options), &options) {
            let file = decoded.records.last().unwrap();
            prop_assert_eq!(file.content_type, ContentType::FILE);
            prop_assert_eq!(file.present_as, PresentAs::File);
            prop_assert_eq!(&file.value, &bytes);
            if name.is_empty() {
                prop_assert_eq!(decoded.records.len(), 1);
            } else {
                prop_assert_eq!(decoded.records.len(), 2);
                prop_assert_eq!(decoded.record_form, RecordForm::List(2));
                prop_assert_eq!(decoded.records[0].present_as, PresentAs::FileName);
                prop_assert_eq!(decoded.records[0].text(), Some(name.as_str()));
                let safe = nmtcode::safe_file_name(name.as_bytes()).filter(|n| !n.is_empty());
                prop_assert_eq!(decoded.file_name_for(1), safe);
            }
        }
    }

    #[test]
    fn record_lists(
        texts in proptest::collection::vec(any::<String>(), 1..5),
        options in options(),
    ) {
        let records: Vec<Record<'_>> = texts
            .iter()
            .map(|text| Record { content_type: ContentType::TEXT, value: text.as_bytes() })
            .collect();
        if let Some((_, decoded)) = round_trip(encode(&records, &options), &options) {
            prop_assert_eq!(decoded.records.len(), texts.len());
            for (record, text) in decoded.records.iter().zip(&texts) {
                prop_assert_eq!(record.text(), Some(text.as_str()));
            }
        }
    }

    /// Content that exactly fills a size: the container is K bytes and no padding follows.
    #[test]
    fn content_that_exactly_fills_a_size(
        width in side(120),
        height in side(120),
        level in 0u8..=3,
        seed in any::<u8>(),
    ) {
        let k = capacity(width, height, level).unwrap();
        // Stored, single record of type 0: lead byte, echo byte, Lb, type, content, CRC-32C
        // (3.2.6).
        let header = |len: usize| 2 + nmtcode_core::leb128_len(u32::try_from(len + 1).unwrap()) + 1;
        let Some(len) = (0..k).rev().find(|&len| header(len) + len + 4 == k) else {
            // No length gives exactly K (the body length field grows by a byte at that point).
            return Ok(());
        };
        let value: Vec<u8> = (0..len).map(|i| seed.wrapping_add(u8::try_from(i % 251).unwrap())).collect();
        let stored_only = CodecOptions {
            digits: false,
            upper_alphanumeric: false,
            token_model: false,
            hangul: false,
            brotli: false,
        };
        let options = EncodeOptions {
            level: Some(level),
            size: SizeRule::Exact { width, height },
            codecs: stored_only,
            ..EncodeOptions::default()
        };
        let records = [Record { content_type: ContentType::UNSPECIFIED, value: &value }];
        let (symbol, decoded) = round_trip(encode(&records, &options), &options).unwrap();
        prop_assert_eq!(symbol.codec(), 0);
        prop_assert_eq!(symbol.container_len(), k);
        prop_assert_eq!(symbol.container(), symbol.message());
        prop_assert_eq!(&decoded.records[0].value, &value);
        // One byte more does not fit.
        let longer = [value.clone(), vec![0]].concat();
        let records = [Record { content_type: ContentType::UNSPECIFIED, value: &longer }];
        let error = encode(&records, &options).unwrap_err();
        prop_assert!(
            matches!(error, EncodeError::DoesNotFit { container_len, capacity, .. }
                if container_len > k && capacity == k),
            "{error:?}"
        );
    }
}

#[test]
fn empty_content() {
    for level in 0..=3 {
        let options = EncodeOptions { level: Some(level), ..EncodeOptions::default() };
        assert_eq!(check_text("", &options), Some(0));
        let (_, decoded) = round_trip(encode_url("", &options), &options).unwrap();
        assert_eq!(decoded.records[0].value, b"");
        assert_eq!(decoded.records[0].present_as, PresentAs::Url);
        let (_, decoded) = round_trip(encode_file("", b"", &options), &options).unwrap();
        assert_eq!(decoded.records[0].value, b"");
        assert_eq!(decoded.records[0].present_as, PresentAs::File);
        let (symbol, decoded) =
            round_trip(encode_file("empty.bin", b"", &options), &options).unwrap();
        assert_eq!(symbol.record_form(), RecordForm::List(2));
        assert_eq!(decoded.file_name_for(1).as_deref(), Some("empty.bin"));
    }
}

#[test]
fn the_selection_rule_picks_the_codec_of_each_kind_of_content() {
    let options = EncodeOptions::default();
    // 6.12.5: the chosen codec for each row.
    assert_eq!(check_text("0123456789", &options), Some(1));
    assert_eq!(check_text("HTTPS://EXAMPLE.COM/ABC", &options), Some(2));
    assert_eq!(check_text("https://github.com/needmoretruth/nmtcode", &options), Some(3));
    assert_eq!(check_text("안녕하세요 NMT Code", &options), Some(4));
    let repetitive = "abcabcabc ".repeat(40);
    assert_eq!(check_text(&repetitive, &options), Some(5));
}

#[test]
fn large_symbols_with_many_blocks() {
    // About 3 KB of text needs several Reed-Solomon blocks at every level.
    let text: String =
        (0..3000).map(|i| char::from(b'a' + u8::try_from(i * 7 % 26).unwrap())).collect();
    for level in 0..=3 {
        let options = EncodeOptions {
            level: Some(level),
            codecs: CodecOptions::without_brotli(),
            ..EncodeOptions::default()
        };
        let (symbol, _) = round_trip(encode_text(&text, &options), &options).unwrap();
        assert!(symbol.block_split().block_count() > 1);
    }
    // A non-square symbol with reference marks, near the largest width.
    let options = EncodeOptions {
        size: SizeRule::Exact { width: 4108, height: 20 },
        profile: Profile::Print,
        ..EncodeOptions::default()
    };
    let (symbol, _) = round_trip(encode_text(&text, &options), &options).unwrap();
    assert_eq!(symbol.level(), 1);
}
