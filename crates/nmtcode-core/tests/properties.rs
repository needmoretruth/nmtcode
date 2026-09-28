//! Property tests: container and record parsing never panic on arbitrary bytes, and what the
//! writer produces parses back to the same fields and records.

use nmtcode_core::{
    ContainerHeader, ContentType, DictionaryEntry, DictionaryKind, ExtensionDigest, FormatWord,
    HashAlgorithm, MAX_STATIC_CONTENT_LEN_V0, ReaderConfig, Record, RecordContent, RecordForm,
    StaticFields, SymbolClass, crc32c, pad_message, parse_message, parse_records, split_container,
    write_leb128,
};
use proptest::prelude::*;

/// A container from the bytes before Lb (the lead byte, and the echo byte of a static container)
/// and a body, with a matching CRC-32C, so that the checks after the CRC-32C are reached.
fn seal(prefix: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    write_leb128(&mut out, u32::try_from(body.len()).unwrap());
    out.extend_from_slice(body);
    let crc = crc32c(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

fn any_format() -> impl Strategy<Value = FormatWord> {
    (any::<bool>(), 0u8..=1).prop_map(|(tile, colour)| {
        if tile {
            FormatWord::new(SymbolClass::TransferTile, 20, 20, 0, 0, 0).unwrap()
        } else {
            FormatWord::new(SymbolClass::Static, 20, 20, 0, colour, 0).unwrap()
        }
    })
}

const CARRIED: [DictionaryEntry; 2] = [
    DictionaryEntry { id: 1, kind: DictionaryKind::ShortTextModel },
    DictionaryEntry { id: 2, kind: DictionaryKind::BrotliPrefix },
];

fn any_config() -> impl Strategy<Value = ReaderConfig<'static>> {
    (any::<[bool; 6]>(), any::<u32>(), any::<bool>()).prop_map(|(codecs, limit, carry)| {
        ReaderConfig { codecs, limit, dictionaries: if carry { &CARRIED } else { &[] } }
    })
}

fn any_form() -> impl Strategy<Value = RecordForm> {
    prop_oneof![
        any::<u32>().prop_map(|t| RecordForm::Single(ContentType(t))),
        any::<u32>().prop_map(RecordForm::List),
    ]
}

fn any_header() -> impl Strategy<Value = ContainerHeader> {
    (any_form(), 0u32..8, any::<u32>(), 0u32..64, any::<bool>()).prop_map(
        |(form, codec, dictionary, decoded_len, colour)| ContainerHeader {
            fields: StaticFields { form, codec, dictionary, decoded_len },
            extension: colour
                .then_some(ExtensionDigest { algorithm: HashAlgorithm::Sha256, digest: [0; 32] }),
        },
    )
}

/// Records a generator may write: an optional action first, then data records, some of them
/// files with a file name before them.
fn any_records() -> impl Strategy<Value = Vec<(u32, Vec<u8>)>> {
    let value = proptest::collection::vec(any::<u8>(), 0..40);
    let data_type =
        prop_oneof![Just(0u32), Just(1), Just(3), Just(4), Just(6), Just(7), Just(8), 9u32..400];
    let item = (data_type, value.clone(), proptest::option::of(value.clone()));
    (proptest::option::of(value), proptest::collection::vec(item, 0..5)).prop_map(
        |(action, items)| {
            let mut records = Vec::new();
            if let Some(url) = action {
                records.push((2, url));
            }
            for (content_type, value, name) in items {
                if let Some(name) = name.filter(|_| ContentType(content_type).is_file_name_target())
                {
                    records.push((5, name));
                }
                records.push((content_type, value));
            }
            records
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn arbitrary_messages_never_panic(
        message in proptest::collection::vec(any::<u8>(), 0..80),
        format in any_format(),
        config in any_config(),
    ) {
        let _ = split_container(&message, format.class());
        if let Ok(container) = parse_message(&message, &format, &config) {
            let _ = parse_records(&container.header, container.coded);
        }
    }

    #[test]
    fn arbitrary_sealed_containers_never_panic(
        lead in any::<u8>(),
        echo in prop_oneof![Just(None), any::<u8>().prop_map(Some)],
        body in proptest::collection::vec(any::<u8>(), 0..80),
        padding in 0usize..8,
        format in any_format(),
        config in any_config(),
    ) {
        // The echo byte of the format word, another byte, or none as in a transfer tile.
        let prefix = match (format.class(), echo) {
            (SymbolClass::TransferTile, _) => vec![lead],
            (SymbolClass::Static, None) => vec![lead, format.echo().byte()],
            (SymbolClass::Static, Some(byte)) => vec![lead, byte],
        };
        let mut message = seal(&prefix, &body);
        let len = message.len();
        pad_message(&mut message, len + padding).unwrap();
        let raw = split_container(&message, format.class()).unwrap();
        prop_assert_eq!(raw.len, len);
        if let Ok(container) = parse_message(&message, &format, &config) {
            prop_assert_eq!(container.len, len);
            prop_assert!(container.header.fields.decoded_len <= MAX_STATIC_CONTENT_LEN_V0);
            // For codec 0 the coded field is the decoded content.
            let _ = parse_records(&container.header, container.coded);
        }
    }

    #[test]
    fn arbitrary_decoded_content_never_panics(
        header in any_header(),
        decoded in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let _ = parse_records(&header, &decoded);
        let fitted = ContainerHeader {
            fields: StaticFields {
                decoded_len: u32::try_from(decoded.len()).unwrap(),
                ..header.fields
            },
            ..header
        };
        if let Ok(records) = parse_records(&fitted, &decoded) {
            prop_assert!(!records.records.is_empty());
        }
    }

    #[test]
    fn written_records_parse_back(records in any_records(), extra in 0usize..20) {
        prop_assume!(!records.is_empty());
        let input: Vec<Record<'_>> = records
            .iter()
            .map(|(t, v)| Record { content_type: ContentType(*t), value: v })
            .collect();
        let content = RecordContent::from_records(&input).unwrap();
        let fields = StaticFields::stored(content.form, content.decoded.len()).unwrap();
        let format = FormatWord::new(SymbolClass::Static, 20, 20, 0, 0, 0).unwrap();
        let mut message = fields.write(format.echo(), &content.decoded).unwrap();
        prop_assert_eq!(fields.container_len(content.decoded.len()), Ok(message.len()));
        let len = message.len();
        pad_message(&mut message, len + extra).unwrap();

        let container = parse_message(&message, &format, &ReaderConfig::DEFAULT).unwrap();
        prop_assert_eq!(container.header.fields, fields);
        prop_assert_eq!(container.len, len);
        let parsed = parse_records(&container.header, container.coded).unwrap();
        let back: Vec<(u32, Vec<u8>)> = parsed
            .records
            .iter()
            .map(|r| (r.content_type.0, r.value.to_vec()))
            .collect();
        prop_assert_eq!(back, records);
    }

    #[test]
    fn written_fields_parse_back(
        form in any_form(),
        codec in 0u32..6,
        dictionary_is_one in any::<bool>(),
        decoded_len in 0u32..=MAX_STATIC_CONTENT_LEN_V0,
        level in 0u8..=3,
        coded in proptest::collection::vec(any::<u8>(), 0..40),
    ) {
        let form = match form {
            RecordForm::List(0) => RecordForm::List(1),
            other => other,
        };
        let takes_dictionary = codec == 3 || codec == 5;
        let dictionary = match (takes_dictionary, dictionary_is_one, codec) {
            (true, true, 3) => 1,
            (true, true, _) => 2,
            _ => 0,
        };
        let decoded_len = if codec == 0 { u32::try_from(coded.len()).unwrap() } else { decoded_len };
        let fields = StaticFields { form, codec, dictionary, decoded_len };
        let format = FormatWord::new(SymbolClass::Static, 20, 20, level, 0, 0).unwrap();
        let message = fields.write(format.echo(), &coded).unwrap();
        prop_assert_eq!(fields.container_len(coded.len()), Ok(message.len()));
        let config = ReaderConfig { dictionaries: &CARRIED, ..ReaderConfig::DEFAULT };
        let container = parse_message(&message, &format, &config).unwrap();
        prop_assert_eq!(container.header.fields, fields);
        prop_assert_eq!(container.coded, &coded[..]);
    }
}
