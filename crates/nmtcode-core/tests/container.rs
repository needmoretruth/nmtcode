//! Container and record tests: the worked examples of 3.10 and annex A (A.2), header sizes of
//! 3.2.6, and every reader error of chapters 2 and 3 (9.8) from a crafted input.

use nmtcode_core::{
    ContentType, DictionaryEntry, DictionaryKind, Error, ExtensionDigest, FormatWord,
    HashAlgorithm, MAX_CONTENT_LEN_V0, Outcome, ParsedRecord, PresentAs, ReaderConfig, Record,
    RecordContent, RecordForm, StaticFields, SymbolClass, ValueNotice, WriteError, crc32c,
    leb128_len, pad_message, parse_message, parse_records, safe_file_name, split_container,
    write_leb128,
};

const URL: &[u8] = b"https://github.com/needmoretruth/nmtcode";

/// Bytes from space-separated hexadecimal pairs, as the specification writes them.
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|pair| u8::from_str_radix(pair, 16).unwrap()).collect()
}

/// A container from a lead byte and a body: lead ‖ Lb ‖ body ‖ CRC-32C.
fn seal(lead: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![lead];
    write_leb128(&mut out, u32::try_from(body.len()).unwrap());
    out.extend_from_slice(body);
    let crc = crc32c(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

fn bw() -> FormatWord {
    FormatWord::new(SymbolClass::Static, 28, 28, 0, 0, 0).unwrap()
}

fn colour() -> FormatWord {
    FormatWord::new(SymbolClass::Static, 28, 28, 0, 1, 0).unwrap()
}

fn tile() -> FormatWord {
    FormatWord::new(SymbolClass::TransferTile, 28, 28, 0, 0, 0).unwrap()
}

fn parse(message: &[u8], format: &FormatWord) -> Result<(), Error> {
    let container = parse_message(message, format, &ReaderConfig::DEFAULT)?;
    parse_records(&container.header, container.coded).map(|_| ())
}

fn record(content_type: ContentType, value: &[u8]) -> Record<'_> {
    Record { content_type, value }
}

fn presented(content_type: ContentType, value: &[u8], present_as: PresentAs) -> ParsedRecord<'_> {
    ParsedRecord { content_type, value, present_as, notice: None }
}

#[test]
fn example_a_url_codec_0() {
    let content = RecordContent::from_records(&[record(ContentType::URL, URL)]).unwrap();
    assert_eq!(content.form, RecordForm::Single(ContentType::URL));
    assert_eq!(content.decoded, URL);
    assert!(!content.stored_only);
    let fields = StaticFields::stored(content.form, content.decoded.len()).unwrap();
    assert_eq!(fields.header_len(40), Ok(3));
    assert_eq!(fields.container_len(40), Ok(47));

    let mut message = fields.write(&content.decoded).unwrap();
    assert_eq!(message.len(), 47);
    assert_eq!(message[..3], [0x00, 0x29, 0x02]);
    assert_eq!(crc32c(&message[..43]), 0xFCCF_4D0A);
    assert_eq!(message[43..], [0xFC, 0xCF, 0x4D, 0x0A]);
    pad_message(&mut message, 56).unwrap();
    let expected = hex("00 29 02 68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E 65
         65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65 FC CF 4D 0A EC
         11 EC 11 EC 11 EC 11 EC");
    assert_eq!(message, expected);

    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.fields, fields);
    assert_eq!(container.header.extension, None);
    assert_eq!(container.coded, URL);
    assert_eq!(container.len, 47);
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records, [presented(ContentType::URL, URL, PresentAs::Url)]);
    assert_eq!(records.outcome(), Outcome::Presented);
    assert_eq!(records.notice(), None);
}

#[test]
fn example_b_utf8_text_codec_0() {
    let text = "안녕하세요 NMT Code".as_bytes();
    assert_eq!(
        text,
        hex("EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 20 4E 4D 54 20 43 6F 64 65")
    );
    let content = RecordContent::from_records(&[record(ContentType::TEXT, text)]).unwrap();
    let fields = StaticFields::stored(content.form, content.decoded.len()).unwrap();
    assert_eq!(fields.header_len(24), Ok(3));
    let message = fields.write(&content.decoded).unwrap();
    let mut expected = hex("00 19 01");
    expected.extend_from_slice(text);
    expected.extend(hex("FF F6 05 7C"));
    assert_eq!(message, expected);
    assert_eq!(crc32c(&message[..27]), 0xFFF6_057C);

    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records, [presented(ContentType::TEXT, text, PresentAs::Text)]);
}

/// Example c of 3.10: the provisional transfer-tile container.
fn example_c() -> Vec<u8> {
    hex("00 1B 00 00 12 34 00 03 07 00 00 00 1F
         00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
         AC 39 AE 69")
}

#[test]
fn example_c_transfer_tile_is_detected_and_unsupported() {
    let message = example_c();
    assert_eq!(crc32c(&message[..29]), 0xAC39_AE69);
    let raw = split_container(&message).unwrap();
    assert_eq!((raw.lead, raw.body.len(), raw.len), (0, 27, 33));
    let config = ReaderConfig::DEFAULT;
    assert_eq!(parse_message(&message, &tile(), &config), Err(Error::TransferUnsupported));
    assert_eq!(Error::TransferUnsupported.outcome(), Outcome::Unsupported);

    // Reserved lead-byte bits of a tile (3.3), with a matching CRC-32C.
    for lead in [0x01, 0x10, 0x20, 0x3F] {
        let reserved = seal(lead, raw.body);
        assert_eq!(parse_message(&reserved, &tile(), &config), Err(Error::TileReservedBits));
    }
    // A damaged tile is damaged before it is unsupported.
    let mut damaged = message.clone();
    damaged[10] ^= 1;
    assert_eq!(parse_message(&damaged, &tile(), &config), Err(Error::CrcMismatch));
}

#[test]
fn example_d_record_list() {
    let records = [
        record(ContentType::URL, URL),
        record(ContentType::FILE_NAME, b"hello.txt"),
        record(ContentType::FILE, b"hello\n"),
    ];
    let content = RecordContent::from_records(&records).unwrap();
    assert_eq!(content.form, RecordForm::List(3));
    assert_eq!(content.decoded.len(), 61);
    let fields = StaticFields::stored(content.form, 61).unwrap();
    assert_eq!(fields.container_len(61), Ok(68));
    let message = fields.write(&content.decoded).unwrap();
    let mut expected = hex("20 3E 03 02 28");
    expected.extend_from_slice(URL);
    expected.extend(hex("05 09 68 65 6C 6C 6F 2E 74 78 74 04 06 68 65 6C 6C 6F 0A"));
    expected.extend(hex("38 55 D9 1B"));
    assert_eq!(message, expected);
    assert_eq!(crc32c(&message[..64]), 0x3855_D91B);

    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    let parsed = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(
        parsed.records,
        [
            presented(ContentType::URL, URL, PresentAs::Url),
            presented(ContentType::FILE_NAME, b"hello.txt", PresentAs::FileName),
            presented(ContentType::FILE, b"hello\n", PresentAs::File),
        ]
    );
}

const DIGEST_E: &str = "53 14 09 A7 3A CC A0 2F A0 78 07 F5 45 BA B6 B8 F0 40 3D 7F 96 CA 2A A1
                        F8 39 A6 73 FD DB 53 31";

#[test]
fn example_e_colour_symbol_read_by_a_black_and_white_reader() {
    let mut base = hex("10 2A 01");
    base.extend(hex(DIGEST_E));
    base.extend(hex("01 4E 4D 54 20 43 6F 64 65 05 06 D2 A1"));
    assert_eq!(base.len(), 48);
    assert_eq!(crc32c(&base[..44]), 0x0506_D2A1);
    let mut message = base.clone();
    pad_message(&mut message, 60).unwrap();

    let container = parse_message(&message, &colour(), &ReaderConfig::DEFAULT).unwrap();
    let digest: [u8; 32] = hex(DIGEST_E).try_into().unwrap();
    assert_eq!(
        container.header.extension,
        Some(ExtensionDigest { algorithm: HashAlgorithm::Sha256, digest })
    );
    assert_eq!(
        container.header.fields,
        StaticFields::stored(RecordForm::Single(ContentType::TEXT), 8).unwrap()
    );
    // Header: 3 bytes plus 33 for the colour extension (3.2.6).
    assert_eq!(container.len - container.coded.len() - 4, 3 + 33);
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records, [presented(ContentType::TEXT, b"NMT Code", PresentAs::Text)]);
    assert!(records.extension_unread);
    assert_eq!(records.outcome(), Outcome::PresentedBaseOnly);
    assert_eq!(records.notice(), Some(Error::ExtensionUnread));
    assert_eq!(Error::ExtensionUnread.outcome(), Outcome::PresentedBaseOnly);

    // The same container in a black-and-white symbol breaks 3.2.2.
    assert_eq!(parse(&message, &bw()), Err(Error::ColourFlag));

    // The extension message is a static container with C = 0.
    let extension = hex("00 10 01 EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 A2 EC CD A1");
    assert_eq!(crc32c(&extension[..18]), 0xA2EC_CDA1);
    let container = parse_message(&extension, &colour(), &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.extension, None);
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(
        records.records,
        [presented(ContentType::TEXT, "안녕하세요".as_bytes(), PresentAs::Text)]
    );

    // The digest input is the canonical base records then the canonical extension records.
    let both = [
        record(ContentType::TEXT, b"NMT Code"),
        record(ContentType::TEXT, "안녕하세요".as_bytes()),
    ];
    let canonical = RecordContent::from_records(&both).unwrap().decoded;
    assert_eq!(
        canonical,
        hex("01 08 4E 4D 54 20 43 6F 64 65 01 0F EC 95 88 EB 85 95 ED 95 98 EC 84 B8
             EC 9A 94")
    );
}

#[test]
fn example_f_codec_3_and_annex_a_2_message() {
    let coded = hex("DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91");
    let fields = StaticFields {
        form: RecordForm::Single(ContentType::URL),
        codec: 3,
        dictionary: 0,
        decoded_len: 40,
    };
    assert_eq!(fields.header_len(19), Ok(5));
    assert_eq!(fields.container_len(19), Ok(28));
    let mut message = fields.write(&coded).unwrap();
    assert_eq!(
        message,
        hex("03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91 33 56 F1 6E")
    );
    assert_eq!(crc32c(&message[..24]), 0x3356_F16E);
    // Annex A (A.2.4): the 34-byte message of the 24 × 24 symbol.
    pad_message(&mut message, 34).unwrap();
    assert_eq!(
        message,
        hex("03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF
             DF 35 76 47 AE BA F7 91 33 56 F1 6E EC 11 EC 11
             EC 11")
    );

    let format = FormatWord::new(SymbolClass::Static, 24, 24, 0, 0, 0).unwrap();
    let container = parse_message(&message, &format, &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.fields, fields);
    assert_eq!(container.coded, coded);
    assert_eq!(container.len, 28);
    // The codec's output is the 40 URL bytes.
    let records = parse_records(&container.header, URL).unwrap();
    assert_eq!(records.records, [presented(ContentType::URL, URL, PresentAs::Url)]);
    // Any other length breaks 3.2.4.
    assert_eq!(parse_records(&container.header, &URL[..39]), Err(Error::Malformed));
}

#[test]
fn header_sizes_of_3_2_6() {
    let single = RecordForm::Single(ContentType::TEXT);
    let stored = |len| StaticFields::stored(single, len).unwrap();
    assert_eq!(stored(126).header_len(126), Ok(3));
    assert_eq!(stored(127).header_len(127), Ok(4));
    assert_eq!(stored(200).header_len(200), Ok(4));
    assert_eq!(stored(16_382).header_len(16_382), Ok(4));
    assert_eq!(stored(200).container_len(200), Ok(208));
    let coded = |codec, dictionary, decoded_len| StaticFields {
        form: single,
        codec,
        dictionary,
        decoded_len,
    };
    for codec in [1, 2, 4] {
        assert_eq!(coded(codec, 0, 127).header_len(10), Ok(3 + 1));
        assert_eq!(coded(codec, 0, 16_383).header_len(10), Ok(3 + 2));
    }
    for codec in [3, 5] {
        assert_eq!(coded(codec, 0, 127).header_len(10), Ok(3 + 1 + 1));
        assert_eq!(coded(codec, 127, 16_383).header_len(10), Ok(3 + 2 + 1));
    }
    // Record-list form: the count replaces the content type in the header.
    let list = StaticFields::stored(RecordForm::List(2), 20).unwrap();
    assert_eq!(list.header_len(20), Ok(3));
    // The container length always equals the written length.
    for (fields, len) in [(coded(5, 300, 70_000), 200), (stored(300), 300), (list, 20)] {
        assert_eq!(fields.container_len(len), Ok(fields.write(&vec![0; len]).unwrap().len()));
    }
}

#[test]
fn writer_rejects_what_a_generator_must_not_write() {
    let single = RecordForm::Single(ContentType::TEXT);
    let fields = |codec, dictionary, decoded_len| StaticFields {
        form: single,
        codec,
        dictionary,
        decoded_len,
    };
    assert_eq!(fields(6, 0, 1).write(&[0]), Err(WriteError::UnknownCodec(6)));
    assert_eq!(fields(15, 0, 1).container_len(1), Err(WriteError::UnknownCodec(15)));
    assert_eq!(fields(1, 1, 1).write(&[0]), Err(WriteError::DictionaryNotAllowed));
    assert_eq!(fields(0, 0, 2).write(&[0]), Err(WriteError::DecodedLengthMismatch));
    assert_eq!(fields(1, 0, MAX_CONTENT_LEN_V0 + 1).write(&[0]), Err(WriteError::ContentTooLarge));
    let empty_list = StaticFields::stored(RecordForm::List(0), 0).unwrap();
    assert_eq!(empty_list.write(&[]), Err(WriteError::NoRecords));

    assert_eq!(RecordContent::from_records(&[]), Err(WriteError::NoRecords));
    let text = record(ContentType::TEXT, b"a");
    let url = record(ContentType::URL, b"b");
    assert_eq!(RecordContent::from_records(&[text, url]), Err(WriteError::ActionRule));
    assert_eq!(RecordContent::from_records(&[url, url]), Err(WriteError::ActionRule));
    let name = record(ContentType::FILE_NAME, b"a.txt");
    assert_eq!(RecordContent::from_records(&[name]), Err(WriteError::FileNameWithoutTarget));
    assert_eq!(RecordContent::from_records(&[name, text]), Err(WriteError::FileNameWithoutTarget));
    let signed = record(ContentType::COSE_SIGN1, b"x");
    assert!(RecordContent::from_records(&[name, signed]).unwrap().stored_only);
}

#[test]
fn damaged_length_field() {
    let good = seal(0x00, &hex("01 41"));
    let cases: [&[u8]; 6] = [
        &[],
        &[0x00],
        &[0x00, 0x80],
        // Lb not minimal (1.4): 02 written as 82 00.
        &[0x00, 0x82, 0x00, 0x01, 0x41, 0, 0, 0, 0],
        // Lb of 2^32.
        &[0x00, 0x80, 0x80, 0x80, 0x80, 0x10, 0, 0, 0, 0],
        // The CRC-32C would lie beyond the capacity: the message ends one byte early.
        &good[..good.len() - 1],
    ];
    for message in cases {
        assert_eq!(parse(message, &bw()), Err(Error::LengthField), "{message:02X?}");
    }
    assert_eq!(parse(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F], &bw()), Err(Error::LengthField));
    assert_eq!(Error::LengthField.outcome(), Outcome::Damaged);
}

#[test]
fn damaged_crc() {
    let mut message = seal(0x00, &hex("01 41"));
    pad_message(&mut message, 16).unwrap();
    assert_eq!(parse(&message, &bw()), Ok(()));
    // Padding is not covered and is ignored (3.7, 3.8).
    message[12] = 0x00;
    assert_eq!(parse(&message, &bw()), Ok(()));
    for index in 0..7 {
        let mut damaged = message.clone();
        damaged[index] ^= 0x04;
        let result = parse(&damaged, &bw());
        assert!(
            matches!(result, Err(Error::CrcMismatch | Error::LengthField)),
            "{index}: {result:?}"
        );
    }
    message[3] ^= 0x01;
    assert_eq!(parse(&message, &bw()), Err(Error::CrcMismatch));
    assert_eq!(Error::CrcMismatch.outcome(), Outcome::Damaged);
}

#[test]
fn unsupported_container_version() {
    for lead in [0x40, 0x80, 0xC0, 0x7F] {
        let message = seal(lead, &hex("01 41"));
        assert_eq!(parse(&message, &bw()), Err(Error::ContainerVersion));
        assert_eq!(parse(&message, &tile()), Err(Error::ContainerVersion));
        // The body can still be offered, labelled as undecoded (3.9).
        assert_eq!(split_container(&message).unwrap().body, [0x01, 0x41]);
    }
}

#[test]
fn colour_flag_needs_a_colour_profile() {
    let mut body = vec![0x01];
    body.extend([0; 32]);
    body.extend([0x01, 0x41]);
    let message = seal(0x10, &body);
    assert_eq!(parse(&message, &bw()), Err(Error::ColourFlag));
    assert_eq!(parse(&message, &colour()), Ok(()));
}

#[test]
fn leb128_fields_in_the_body() {
    let cases: [(u8, &[u8]); 6] = [
        // Content type ID 2 written as 82 00.
        (0x00, &[0x82, 0x00, 0x41]),
        // Content type ID of 2^32.
        (0x00, &[0x80, 0x80, 0x80, 0x80, 0x10]),
        // Record count not minimal.
        (0x20, &[0x81, 0x00, 0x01, 0x01, 0x41]),
        // Decoded length of codec 1 not minimal.
        (0x01, &[0x83, 0x00, 0x01, 0x00]),
        // Dictionary ID of codec 3 not minimal.
        (0x03, &[0x80, 0x00, 0x01, 0x01, 0x00]),
        // Codec ID escape not minimal.
        (0x0F, &[0x90, 0x00, 0x01, 0x00]),
    ];
    for (lead, body) in cases {
        assert_eq!(parse(&seal(lead, body), &bw()), Err(Error::Leb128), "{lead:02X} {body:02X?}");
    }
    // In a record of a record list.
    let message = seal(0x20, &hex("02 81 00 01 41 01 01 42"));
    assert_eq!(parse(&message, &bw()), Err(Error::Leb128));
    let message = seal(0x20, &hex("01 01 80 00 41"));
    assert_eq!(parse(&message, &bw()), Err(Error::Leb128));
    assert_eq!(Error::Leb128.outcome(), Outcome::Malformed);
}

#[test]
fn codec_escape_and_unsupported_codecs() {
    for escaped in [0, 1, 5, 14] {
        let message = seal(0x0F, &[escaped, 0x01, 0x00]);
        assert_eq!(parse(&message, &bw()), Err(Error::CodecEscape));
    }
    // Escaped 15 and 16: valid escapes, but no such codec in this version.
    for escaped in [0x0F, 0x10, 0x7F] {
        let message = seal(0x0F, &[escaped, 0x01, 0x00]);
        assert_eq!(parse(&message, &bw()), Err(Error::UnsupportedCodec));
    }
    for codec in 6..=14 {
        let message = seal(codec, &[0x01, 0x01, 0x00]);
        assert_eq!(parse(&message, &bw()), Err(Error::UnsupportedCodec));
    }
    // A codec the reader does not implement stops before any further field is read (6.2):
    // the malformed dictionary ID after it is not reported.
    let mut config = ReaderConfig::DEFAULT;
    config.codecs[5] = false;
    let message = seal(0x05, &[0x80, 0x00, 0x01, 0x01]);
    assert_eq!(parse_message(&message, &bw(), &config).map(|_| ()), Err(Error::UnsupportedCodec));
    assert_eq!(parse(&message, &bw()), Err(Error::Leb128));
    assert_eq!(Error::UnsupportedCodec.outcome(), Outcome::Unsupported);
}

#[test]
fn dictionaries() {
    // Codec 3, dictionary d, L = 1, content type 1, one coded byte.
    let message = |d: u32| {
        let mut body = Vec::new();
        write_leb128(&mut body, d);
        body.extend([0x01, 0x01, 0x00]);
        seal(0x03, &body)
    };
    let default = ReaderConfig::DEFAULT;
    assert!(parse_message(&message(0), &bw(), &default).is_ok());
    for d in [1, 127, 128, 16_384, 32_767, 32_768] {
        assert_eq!(
            parse_message(&message(d), &bw(), &default).map(|_| ()),
            Err(Error::UnknownDictionary)
        );
    }
    let carried = [
        DictionaryEntry { id: 1, kind: DictionaryKind::ShortTextModel },
        DictionaryEntry { id: 2, kind: DictionaryKind::BrotliPrefix },
    ];
    let config = ReaderConfig { dictionaries: &carried, ..ReaderConfig::DEFAULT };
    let one = message(1);
    let container = parse_message(&one, &bw(), &config).unwrap();
    assert_eq!(container.header.fields.dictionary, 1);
    assert_eq!(
        parse_message(&message(2), &bw(), &config).map(|_| ()),
        Err(Error::DictionaryMismatch)
    );
    assert_eq!(Error::UnknownDictionary.outcome(), Outcome::Unsupported);
    assert_eq!(Error::DictionaryMismatch.outcome(), Outcome::Malformed);
}

#[test]
fn hash_algorithm_ids() {
    let message = |id: u8| {
        let mut body = vec![id];
        body.extend([0xAA; 32]);
        body.extend([0x01, 0x41]);
        seal(0x10, &body)
    };
    assert_eq!(parse(&message(1), &colour()), Ok(()));
    for id in [0, 3, 4, 5, 6] {
        assert_eq!(parse(&message(id), &colour()), Err(Error::HashIdInvalid));
    }
    for id in [2, 7, 0x7F] {
        assert_eq!(parse(&message(id), &colour()), Err(Error::UnknownHash));
    }
    assert_eq!(Error::HashIdInvalid.outcome(), Outcome::Malformed);
    assert_eq!(Error::UnknownHash.outcome(), Outcome::Unsupported);
}

#[test]
fn header_overrun() {
    let mut short_digest = vec![0x01];
    short_digest.extend([0; 31]);
    let cases: [(u8, &[u8], FormatWord); 6] = [
        // No content type ID.
        (0x00, &[], bw()),
        // Content type ID runs past the body.
        (0x00, &[0x80], bw()),
        // Codec 3: no dictionary ID.
        (0x03, &[], bw()),
        // Codec 1: no decoded length.
        (0x01, &[], bw()),
        // Codec escape runs past the body.
        (0x0F, &[0xFF], bw()),
        // Digest shorter than 32 bytes.
        (0x10, &short_digest, colour()),
    ];
    for (lead, body, format) in cases {
        assert_eq!(parse(&seal(lead, body), &format), Err(Error::HeaderOverrun), "{lead:02X}");
    }
    assert_eq!(Error::HeaderOverrun.outcome(), Outcome::Malformed);
}

#[test]
fn decoded_length_limits() {
    // Codec 1 with decoded length L and one coded byte.
    let message = |len: u32| {
        let mut body = Vec::new();
        write_leb128(&mut body, len);
        body.extend([0x01, 0x00]);
        seal(0x01, &body)
    };
    let default = ReaderConfig::DEFAULT;
    let largest = message(MAX_CONTENT_LEN_V0);
    let container = parse_message(&largest, &bw(), &default).unwrap();
    assert_eq!(container.header.fields.decoded_len, MAX_CONTENT_LEN_V0);
    for len in [MAX_CONTENT_LEN_V0 + 1, u32::MAX] {
        assert_eq!(
            parse_message(&message(len), &bw(), &default).map(|_| ()),
            Err(Error::Malformed)
        );
    }
    let small = ReaderConfig { limit: 10, ..ReaderConfig::DEFAULT };
    assert!(parse_message(&message(10), &bw(), &small).is_ok());
    assert_eq!(parse_message(&message(11), &bw(), &small).map(|_| ()), Err(Error::TooLarge));
    // Codec 0: L = Lc is checked the same way.
    let stored = seal(0x00, &[0x01; 12]);
    assert_eq!(parse_message(&stored, &bw(), &small).map(|_| ()), Err(Error::TooLarge));
    assert_eq!(Error::TooLarge.outcome(), Outcome::Unsupported);
    assert_eq!(Error::Malformed.outcome(), Outcome::Malformed);
}

#[test]
fn record_list_errors() {
    let cases: [&[u8]; 5] = [
        // Record count 0.
        &[0x00],
        // Count 2, one record.
        &[0x02, 0x01, 0x01, 0x41],
        // Value runs past the end.
        &[0x01, 0x01, 0x05, 0x41],
        // Value length missing.
        &[0x01, 0x01],
        // Bytes left over.
        &[0x01, 0x01, 0x01, 0x41, 0x00],
    ];
    for body in cases {
        assert_eq!(parse(&seal(0x20, body), &bw()), Err(Error::RecordList), "{body:02X?}");
    }
    assert_eq!(Error::RecordList.outcome(), Outcome::Malformed);
}

#[test]
fn colour_symbol_without_base_record() {
    let mut body = vec![0x01];
    body.extend([0; 32]);
    body.push(0x00);
    assert_eq!(parse(&seal(0x30, &body), &colour()), Err(Error::NoBaseRecord));
    assert_eq!(Error::NoBaseRecord.outcome(), Outcome::Malformed);
}

#[test]
fn ordering_rules_of_3_4_4() {
    // Content: canonical records written by hand.
    let list = |records: &[(u8, &[u8])]| {
        let mut body = Vec::new();
        write_leb128(&mut body, u32::try_from(records.len()).unwrap());
        for &(content_type, value) in records {
            body.push(content_type);
            write_leb128(&mut body, u32::try_from(value.len()).unwrap());
            body.extend_from_slice(value);
        }
        seal(0x20, &body)
    };
    // Rule 1: at most one action record.
    assert_eq!(parse(&list(&[(2, b"a"), (2, b"b")]), &bw()), Err(Error::ActionRule));
    // Rule 2: the action record comes first.
    assert_eq!(parse(&list(&[(1, b"a"), (2, b"b")]), &bw()), Err(Error::ActionRule));
    assert_eq!(parse(&list(&[(2, b"b"), (1, b"a")]), &bw()), Ok(()));
    assert_eq!(Error::ActionRule.outcome(), Outcome::Malformed);

    // Rule 3: a file name without a target is presented as unknown data.
    let message = list(&[(5, b"a"), (5, b"b"), (8, b"c"), (5, b"d")]);
    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    let records = parse_records(&container.header, container.coded).unwrap();
    let kinds: Vec<PresentAs> = records.records.iter().map(|r| r.present_as).collect();
    assert_eq!(
        kinds,
        [PresentAs::Unknown, PresentAs::FileName, PresentAs::File, PresentAs::Unknown]
    );
    let single = seal(0x00, &hex("05 61"));
    let container = parse_message(&single, &bw(), &ReaderConfig::DEFAULT).unwrap();
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records[0].present_as, PresentAs::Unknown);
}

#[test]
fn presentation_by_type_of_3_4_3() {
    let psbt = [0x70, 0x73, 0x62, 0x74, 0xFF, 0x01];
    let bad_utf8: &[u8] = &[0xC3, 0x28];
    let cases: [(u32, &[u8], PresentAs, Option<ValueNotice>); 13] = [
        (0, b"x", PresentAs::Bytes, None),
        (1, b"x", PresentAs::Text, None),
        (1, bad_utf8, PresentAs::Bytes, Some(ValueNotice::NotUtf8)),
        (2, bad_utf8, PresentAs::Bytes, Some(ValueNotice::NotUtf8)),
        (3, b"{}", PresentAs::Text, None),
        (3, bad_utf8, PresentAs::Bytes, Some(ValueNotice::NotUtf8)),
        (4, &psbt, PresentAs::File, None),
        (6, &psbt, PresentAs::Psbt, None),
        (6, b"x", PresentAs::File, Some(ValueNotice::NotPsbt)),
        (7, b"x", PresentAs::File, None),
        (8, b"x", PresentAs::File, None),
        (9, b"x", PresentAs::Unknown, None),
        (300, b"", PresentAs::Unknown, None),
    ];
    for (content_type, value, present_as, notice) in cases {
        let mut body = Vec::new();
        write_leb128(&mut body, content_type);
        body.extend_from_slice(value);
        let message = seal(0x00, &body);
        let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
        let records = parse_records(&container.header, container.coded).unwrap();
        let expected =
            ParsedRecord { content_type: ContentType(content_type), value, present_as, notice };
        assert_eq!(records.records, [expected], "type {content_type}");
    }
    // A file name that is not UTF-8 is presented as bytes.
    let message = seal(0x20, &hex("02 05 01 FF 04 00"));
    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records[0].present_as, PresentAs::Bytes);
    assert_eq!(records.records[1].present_as, PresentAs::File);
}

#[test]
fn file_names_are_never_paths() {
    let cases: [(&[u8], Option<&str>); 7] = [
        (b"hello.txt", Some("hello.txt")),
        (b"../../etc/passwd", Some("passwd")),
        (b"C:\\Users\\x\\report.pdf", Some("report.pdf")),
        (b"a\x00b\nc\x7F.txt", Some("abc.txt")),
        (b"dir/..", Some("")),
        (b"dir/", Some("")),
        (&[0x66, 0xFF], None),
    ];
    for (value, expected) in cases {
        assert_eq!(safe_file_name(value).as_deref(), expected, "{value:02X?}");
    }
}

#[test]
fn error_names_and_outcomes_match_the_table_of_9_8() {
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/09-versions-and-registries.md"
    ))
    .unwrap();
    let table: Vec<(String, String)> = spec
        .lines()
        .filter(|line| line.starts_with("| `E_"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            (cells[1].trim_matches('`').to_owned(), cells[3].to_owned())
        })
        .collect();
    assert_eq!(table.len(), Error::ALL.len());
    for ((name, outcome), error) in table.iter().zip(Error::ALL) {
        assert_eq!(error.name(), name);
        let expected = match outcome.as_str() {
            "Damaged" => Outcome::Damaged,
            "Unsupported" => Outcome::Unsupported,
            "Malformed" => Outcome::Malformed,
            "Presented, base only" => Outcome::PresentedBaseOnly,
            "Error reported" => Outcome::ErrorReported,
            other => panic!("unknown outcome {other}"),
        };
        assert_eq!(error.outcome(), expected, "{name}");
        assert_eq!(error.to_string(), *name);
    }
    assert_eq!(Error::FormatUnreadable.name(), "E_FORMAT_UNREADABLE");
}

#[test]
fn lengths_are_leb128() {
    // The body length field grows with the body (3.2.3).
    for len in [126usize, 127, 16_382, 16_383] {
        let fields =
            StaticFields::stored(RecordForm::Single(ContentType::UNSPECIFIED), len).unwrap();
        let body = u32::try_from(len + 1).unwrap();
        assert_eq!(fields.container_len(len), Ok(1 + leb128_len(body) + len + 1 + 4));
    }
}
