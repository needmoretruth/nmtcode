//! Container and record tests: the worked examples of 3.10, header sizes of 3.2.6, and every
//! reader error of chapter 3 (9.8) from a crafted input.

use nmtcode_core::{
    ContentType, DictionaryEntry, DictionaryKind, Error, ExtensionDigest, FormatEcho, FormatWord,
    HashAlgorithm, MAX_CONTENT_LEN_V0, MAX_STATIC_CONTENT_LEN_V0, Outcome, ParsedRecord, PresentAs,
    ReaderConfig, Record, RecordContent, RecordForm, StaticFields, SymbolClass, ValueNotice,
    WriteError, crc32c, digest_input, leb128_len, pad_message, parse_message, parse_records,
    safe_file_name, split_container, write_leb128,
};
use sha2::{Digest, Sha256};

const URL: &[u8] = b"https://github.com/needmoretruth/nmtcode";

/// Bytes from space-separated hexadecimal pairs, as the specification writes them.
fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|pair| u8::from_str_radix(pair, 16).unwrap()).collect()
}

/// A container from its bytes before Lb and a body: prefix ‖ Lb ‖ body ‖ CRC-32C.
fn seal_with(prefix: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    write_leb128(&mut out, u32::try_from(body.len()).unwrap());
    out.extend_from_slice(body);
    let crc = crc32c(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

/// A static container of a level-0 symbol: lead ‖ echo `00` (colour profile 0) or `10`
/// (colour profile 1, when the lead byte has X = 1) ‖ Lb ‖ body ‖ CRC-32C.
fn seal(lead: u8, body: &[u8]) -> Vec<u8> {
    let echo = if lead & 0x10 != 0 { colour().echo() } else { bw().echo() };
    seal_with(&[lead, echo.byte()], body)
}

/// A transfer tile: lead ‖ Lb ‖ body ‖ CRC-32C, with no echo byte (3.3).
fn seal_tile(lead: u8, body: &[u8]) -> Vec<u8> {
    seal_with(&[lead], body)
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
    assert_eq!(fields.header_len(40), Ok(4));
    assert_eq!(fields.container_len(40), Ok(48));

    assert_eq!(bw().echo().byte(), 0x00);
    let mut message = fields.write(bw().echo(), &content.decoded).unwrap();
    assert_eq!(message.len(), 48);
    assert_eq!(message[..4], [0x00, 0x00, 0x29, 0x02]);
    assert_eq!(crc32c(&message[..44]), 0x0578_960C);
    assert_eq!(message[44..], [0x05, 0x78, 0x96, 0x0C]);
    pad_message(&mut message, 56).unwrap();
    let expected = hex("00 00 29 02 68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E
         65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65 05 78 96 0C
         EC 11 EC 11 EC 11 EC 11");
    assert_eq!(message, expected);

    let container = parse_message(&message, &bw(), &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.fields, fields);
    assert_eq!(container.header.extension, None);
    assert_eq!(container.coded, URL);
    assert_eq!(container.len, 48);
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
    assert_eq!(fields.header_len(24), Ok(4));
    // Level 1: the echo byte is 40.
    let format = FormatWord::new(SymbolClass::Static, 20, 20, 1, 0, 0).unwrap();
    assert_eq!(format.echo().byte(), 0x40);
    let message = fields.write(format.echo(), &content.decoded).unwrap();
    let mut expected = hex("00 40 19 01");
    expected.extend_from_slice(text);
    expected.extend(hex("26 7D 9B A5"));
    assert_eq!(message, expected);
    assert_eq!(crc32c(&message[..28]), 0x267D_9BA5);

    let container = parse_message(&message, &format, &ReaderConfig::DEFAULT).unwrap();
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
    let raw = split_container(&message, SymbolClass::TransferTile).unwrap();
    assert_eq!((raw.lead, raw.echo, raw.body.len(), raw.len), (0, None, 27, 33));
    let config = ReaderConfig::DEFAULT;
    assert_eq!(parse_message(&message, &tile(), &config), Err(Error::TransferUnsupported));
    assert_eq!(Error::TransferUnsupported.outcome(), Outcome::Unsupported);

    // Reserved lead-byte bits of a tile (3.3), with a matching CRC-32C.
    for lead in [0x01, 0x10, 0x20, 0x3F] {
        let reserved = seal_tile(lead, raw.body);
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
    assert_eq!(fields.container_len(61), Ok(69));
    let message = fields.write(bw().echo(), &content.decoded).unwrap();
    let mut expected = hex("20 00 3E 03 02 28");
    expected.extend_from_slice(URL);
    expected.extend(hex("05 09 68 65 6C 6C 6F 2E 74 78 74 04 06 68 65 6C 6C 6F 0A"));
    expected.extend(hex("21 C2 FF FD"));
    assert_eq!(message, expected);
    assert_eq!(crc32c(&message[..65]), 0x21C2_FFFD);

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

const DIGEST_E: &str = "88 42 74 7E 65 E1 E1 BB CF 64 6D 36 FC 53 53 49 0E B4 4D 5E EC 5A FF C7
                        BD 1F 41 03 BE F9 EA D5";

/// Example e of 3.10: a 32 × 32 symbol at level 0 with colour profile 1 and 1 × 1 cells.
fn format_e() -> FormatWord {
    FormatWord::new(SymbolClass::Static, 32, 32, 0, 1, 0).unwrap()
}

#[test]
fn example_e_colour_symbol_read_by_a_black_and_white_reader() {
    let format = format_e();
    assert_eq!(format.data(), 0x002_0082);
    assert_eq!(format.echo().byte(), 0x10);

    // The digest input (3.5): d, the base record count, the base records, the extension record
    // count, the extension records.
    let base_records = [record(ContentType::TEXT, b"NMT Code")];
    let extension_records = [record(ContentType::TEXT, "안녕하세요".as_bytes())];
    let input = digest_input(&format, &base_records, &extension_records).unwrap();
    assert_eq!(
        input,
        hex("00 02 00 82 01 01 08 4E 4D 54 20 43 6F 64 65 01 01 0F EC 95 88 EB 85 95 ED 95 98
             EC 84 B8 EC 9A 94")
    );
    assert_eq!(Sha256::digest(&input).as_slice(), hex(DIGEST_E));

    let mut base = hex("10 10 2A 01");
    base.extend(hex(DIGEST_E));
    base.extend(hex("01 4E 4D 54 20 43 6F 64 65 03 DB 0A A4"));
    assert_eq!(base.len(), 49);
    assert_eq!(crc32c(&base[..45]), 0x03DB_0AA4);
    let mut message = base.clone();
    pad_message(&mut message, 60).unwrap();

    let container = parse_message(&message, &format, &ReaderConfig::DEFAULT).unwrap();
    let digest: [u8; 32] = hex(DIGEST_E).try_into().unwrap();
    assert_eq!(
        container.header.extension,
        Some(ExtensionDigest { algorithm: HashAlgorithm::Sha256, digest })
    );
    assert_eq!(
        container.header.fields,
        StaticFields::stored(RecordForm::Single(ContentType::TEXT), 8).unwrap()
    );
    // Header: 4 bytes plus 33 for the colour extension (3.2.6).
    assert_eq!(container.len - container.coded.len() - 4, 4 + 33);
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(records.records, [presented(ContentType::TEXT, b"NMT Code", PresentAs::Text)]);
    assert!(records.extension_unread);
    assert_eq!(records.outcome(), Outcome::PresentedBaseOnly);
    assert_eq!(records.notice(), Some(Error::ExtensionUnread));
    assert_eq!(Error::ExtensionUnread.outcome(), Outcome::PresentedBaseOnly);

    // The same container in a black-and-white symbol: the echo byte says colour profile 1.
    let bw_32 = FormatWord::new(SymbolClass::Static, 32, 32, 0, 0, 0).unwrap();
    assert_eq!(parse(&message, &bw_32), Err(Error::FormatEcho));

    // The extension message is a static container with X = 0 and the same echo byte.
    let content = RecordContent::from_records(&extension_records).unwrap();
    let fields = StaticFields::stored(content.form, content.decoded.len()).unwrap();
    let extension = fields.write(format.echo(), &content.decoded).unwrap();
    assert_eq!(
        extension,
        hex("00 10 10 01 EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 28 CA 4D 68")
    );
    assert_eq!(crc32c(&extension[..19]), 0x28CA_4D68);
    let container = parse_message(&extension, &format, &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.extension, None);
    let records = parse_records(&container.header, container.coded).unwrap();
    assert_eq!(
        records.records,
        [presented(ContentType::TEXT, "안녕하세요".as_bytes(), PresentAs::Text)]
    );
}

#[test]
fn digest_input_binds_the_format_word_and_the_split() {
    let format = format_e();
    let a = record(ContentType::TEXT, b"a");
    let b = record(ContentType::TEXT, b"b");
    // Moving a record from the base to the extension changes the digest input (3.5).
    assert_ne!(
        digest_input(&format, &[a, b], &[]).unwrap(),
        digest_input(&format, &[a], &[b]).unwrap()
    );
    // So does another chroma cell size.
    let c2 = FormatWord::new(SymbolClass::Static, 32, 32, 0, 1, 1).unwrap();
    assert_ne!(digest_input(&format, &[a], &[b]).unwrap(), digest_input(&c2, &[a], &[b]).unwrap());
}

#[test]
fn example_f_codec_3() {
    let coded = hex("DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91");
    let fields = StaticFields {
        form: RecordForm::Single(ContentType::URL),
        codec: 3,
        dictionary: 0,
        decoded_len: 40,
    };
    assert_eq!(fields.header_len(19), Ok(6));
    assert_eq!(fields.container_len(19), Ok(29));
    let mut message = fields.write(bw().echo(), &coded).unwrap();
    assert_eq!(
        message,
        hex("03 00 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91
             13 7D CF C4")
    );
    assert_eq!(crc32c(&message[..25]), 0x137D_CFC4);
    pad_message(&mut message, 34).unwrap();

    let format = FormatWord::new(SymbolClass::Static, 20, 28, 0, 0, 0).unwrap();
    let container = parse_message(&message, &format, &ReaderConfig::DEFAULT).unwrap();
    assert_eq!(container.header.fields, fields);
    assert_eq!(container.coded, coded);
    assert_eq!(container.len, 29);
    // The codec's output is the 40 URL bytes.
    let records = parse_records(&container.header, URL).unwrap();
    assert_eq!(records.records, [presented(ContentType::URL, URL, PresentAs::Url)]);
    // Any other length breaks 3.2.4.
    assert_eq!(parse_records(&container.header, &URL[..39]), Err(Error::Malformed));
}

#[test]
fn format_echo_byte_of_3_2_2() {
    // Level (bits 7–6), colour profile (5–4), chroma cell size (3), symbol class (2).
    let echo = |level, colour, cell| {
        FormatEcho::new(SymbolClass::Static, level, colour, cell).unwrap().byte()
    };
    assert_eq!(
        [echo(0, 0, 0), echo(1, 0, 0), echo(2, 0, 0), echo(3, 0, 0)],
        [0x00, 0x40, 0x80, 0xC0]
    );
    assert_eq!([echo(0, 1, 0), echo(0, 1, 1), echo(3, 1, 1)], [0x10, 0x18, 0xD8]);
    assert_eq!(FormatEcho::new(SymbolClass::TransferTile, 0, 0, 0).unwrap().byte(), 0x04);
    assert!(FormatEcho::new(SymbolClass::Static, 4, 0, 0).is_err());
    assert!(FormatEcho::new(SymbolClass::Static, 0, 0, 1).is_err());

    // A reader compares the whole byte with the chosen format word, reserved bits included.
    let body = hex("01 41");
    for level in 0..4u8 {
        let format = FormatWord::new(SymbolClass::Static, 48, 48, level, 0, 0).unwrap();
        for byte in 0..=255u8 {
            let result = parse(&seal_with(&[0x00, byte], &body), &format);
            if byte == format.echo().byte() {
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(result, Err(Error::FormatEcho), "level {level} echo {byte:02X}");
            }
        }
    }
    assert_eq!(Error::FormatEcho.outcome(), Outcome::Malformed);
    // The CRC-32C is checked first: a damaged echo byte is damaged, not malformed.
    let mut damaged = seal(0x00, &body);
    damaged[1] ^= 0x40;
    assert_eq!(parse(&damaged, &bw()), Err(Error::CrcMismatch));
}

#[test]
fn header_sizes_of_3_2_6() {
    let single = RecordForm::Single(ContentType::TEXT);
    let stored = |len| StaticFields::stored(single, len).unwrap();
    assert_eq!(stored(126).header_len(126), Ok(4));
    assert_eq!(stored(127).header_len(127), Ok(5));
    assert_eq!(stored(200).header_len(200), Ok(5));
    assert_eq!(stored(16_382).header_len(16_382), Ok(5));
    assert_eq!(stored(16_383).header_len(16_383), Ok(6));
    assert_eq!(stored(200).container_len(200), Ok(209));
    let coded = |codec, dictionary, decoded_len| StaticFields {
        form: single,
        codec,
        dictionary,
        decoded_len,
    };
    for codec in [1, 2, 4] {
        assert_eq!(coded(codec, 0, 127).header_len(10), Ok(4 + 1));
        assert_eq!(coded(codec, 0, 16_383).header_len(10), Ok(4 + 2));
    }
    for codec in [3, 5] {
        assert_eq!(coded(codec, 0, 127).header_len(10), Ok(4 + 1 + 1));
        assert_eq!(coded(codec, 127, 16_383).header_len(10), Ok(4 + 2 + 1));
    }
    // Record-list form: the count replaces the content type in the header.
    let list = StaticFields::stored(RecordForm::List(2), 20).unwrap();
    assert_eq!(list.header_len(20), Ok(4));
    // The container length always equals the written length.
    for (fields, len) in [(coded(5, 300, 70_000), 200), (stored(300), 300), (list, 20)] {
        let written = fields.write(bw().echo(), &vec![0; len]).unwrap();
        assert_eq!(fields.container_len(len), Ok(written.len()));
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
    let echo = bw().echo();
    assert_eq!(fields(6, 0, 1).write(echo, &[0]), Err(WriteError::UnknownCodec(6)));
    assert_eq!(fields(15, 0, 1).container_len(1), Err(WriteError::UnknownCodec(15)));
    assert_eq!(fields(1, 1, 1).write(echo, &[0]), Err(WriteError::DictionaryNotAllowed));
    assert_eq!(fields(0, 0, 2).write(echo, &[0]), Err(WriteError::DecodedLengthMismatch));
    // The cap of a static container is MAX_STATIC_CONTENT_LEN_V0 (1 MiB, 6.4).
    let too_large = fields(1, 0, MAX_STATIC_CONTENT_LEN_V0 + 1).write(echo, &[0]);
    assert_eq!(too_large, Err(WriteError::ContentTooLarge));
    assert!(fields(1, 0, MAX_STATIC_CONTENT_LEN_V0).write(echo, &[0]).is_ok());
    let empty_list = StaticFields::stored(RecordForm::List(0), 0).unwrap();
    assert_eq!(empty_list.write(echo, &[]), Err(WriteError::NoRecords));

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
    let cases: [&[u8]; 8] = [
        &[],
        &[0x00],
        &[0x00, 0x00],
        &[0x00, 0x00, 0x80],
        // Lb not minimal (1.4): 02 written as 82 00.
        &[0x00, 0x00, 0x82, 0x00, 0x01, 0x41, 0, 0, 0, 0],
        // Lb of 2^32.
        &[0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x10, 0, 0, 0, 0],
        // Lb of 2^32 − 1, which wraps a 32-bit sum (1.4).
        &[0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0, 0, 0, 0],
        // The CRC-32C would lie beyond the capacity: the message ends one byte early.
        &good[..good.len() - 1],
    ];
    for message in cases {
        assert_eq!(parse(message, &bw()), Err(Error::LengthField), "{message:02X?}");
    }
    // A transfer tile has no echo byte: Lb follows the lead byte.
    assert_eq!(parse(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F], &tile()), Err(Error::LengthField));
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
        // The body can still be offered, labelled as undecoded (3.9).
        assert_eq!(split_container(&message, SymbolClass::Static).unwrap().body, [0x01, 0x41]);
        let tile_message = seal_tile(lead, &hex("01 41"));
        assert_eq!(parse(&tile_message, &tile()), Err(Error::ContainerVersion));
    }
    // The container version is read before the echo byte.
    assert_eq!(
        parse(&seal_with(&[0x40, 0xFF], &hex("01 41")), &bw()),
        Err(Error::ContainerVersion)
    );
}

#[test]
fn colour_flag_needs_a_colour_profile() {
    let mut body = vec![0x01];
    body.extend([0; 32]);
    body.extend([0x01, 0x41]);
    // X = 1 with the echo byte of a black-and-white word: the echo matches, X does not.
    let message = seal_with(&[0x10, 0x00], &body);
    assert_eq!(parse(&message, &bw()), Err(Error::ColourFlag));
    assert_eq!(parse(&seal(0x10, &body), &colour()), Ok(()));
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
    let largest = message(MAX_STATIC_CONTENT_LEN_V0);
    let container = parse_message(&largest, &bw(), &default).unwrap();
    assert_eq!(container.header.fields.decoded_len, MAX_STATIC_CONTENT_LEN_V0);
    // A static container is capped at 1 MiB, even for a reader with a higher limit.
    let generous = ReaderConfig { limit: u32::MAX, ..ReaderConfig::DEFAULT };
    for len in [MAX_STATIC_CONTENT_LEN_V0 + 1, MAX_CONTENT_LEN_V0, u32::MAX] {
        assert_eq!(
            parse_message(&message(len), &bw(), &generous).map(|_| ()),
            Err(Error::Malformed)
        );
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
    // A value length of 2^32 − 1, which wraps a 32-bit offset sum (1.4).
    let huge = seal(0x20, &hex("01 01 FF FF FF FF 0F 41"));
    assert_eq!(parse(&huge, &bw()), Err(Error::RecordList));
    assert_eq!(Error::RecordList.outcome(), Outcome::Malformed);
}

#[test]
fn colour_symbol_with_a_record_count_of_0() {
    // Record-list form with count 0 is E_RECORD_LIST in every symbol (3.2.5).
    let mut body = vec![0x01];
    body.extend([0; 32]);
    body.push(0x00);
    assert_eq!(parse(&seal(0x30, &body), &colour()), Err(Error::RecordList));
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
fn file_name_steps_of_3_4_3() {
    let cases: [(&str, &str); 12] = [
        // Step 1 before step 2: `a/b` keeps `b`.
        ("a/b", "b"),
        // Step 2: direction and invisible characters.
        ("invoice\u{202E}fdp.exe", "invoicefdp.exe"),
        ("in\u{200B}vis\u{FEFF}ible\u{00AD}.txt", "invisible.txt"),
        // Step 3: reserved characters.
        ("a:b*c?d\"e<f>g|h", "a_b_c_d_e_f_g_h"),
        // Step 4: leading dots, trailing dots and spaces.
        (".bashrc", "bashrc"),
        ("...", ""),
        ("name. . ", "name"),
        // Step 5: device names, in any case, with or without an extension.
        ("CON", "_CON"),
        ("nul.txt", "_nul.txt"),
        ("com1.log", "_com1.log"),
        ("console.txt", "console.txt"),
        ("LPT10", "LPT10"),
    ];
    for (value, expected) in cases {
        assert_eq!(safe_file_name(value.as_bytes()).as_deref(), Some(expected), "{value:?}");
    }
    // Step 6: at most 255 bytes, cut at a character boundary.
    let long = "가".repeat(100);
    let cut = safe_file_name(long.as_bytes()).unwrap();
    assert_eq!(cut.len(), 255);
    assert!(long.starts_with(&cut));
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
            "Presented, with error" => Outcome::PresentedWithError,
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
        assert_eq!(fields.container_len(len), Ok(2 + leb128_len(body) + len + 1 + 4));
    }
}
