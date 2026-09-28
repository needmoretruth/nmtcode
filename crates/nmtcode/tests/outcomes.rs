//! The reader outcomes of chapter 3 (3.9) and the error names of chapter 9 (9.8), through the
//! whole pipeline: each case is a symbol whose base layer carries a crafted container.

mod common;

use common::{sealed_container, symbol_with_container};
use nmtcode::{
    CodecOptions, ContentType, DecodeOptions, EncodeError, EncodeOptions, Outcome, PresentAs,
    Profile, Record, RecordForm, SizeRule, SpecError, SymbolClass, ValueNotice, decode, encode,
    encode_text,
};
use nmtcode_core::WriteError;

fn read(
    container: &[u8],
    colour_profile: u8,
    class: SymbolClass,
) -> Result<nmtcode::Decoded, nmtcode::DecodeError> {
    let grid = symbol_with_container(container, 48, 48, 0, colour_profile, class);
    decode(&grid, &DecodeOptions::default())
}

fn error_of(container: &[u8]) -> (&'static str, Outcome) {
    let error = read(container, 0, SymbolClass::Static).unwrap_err();
    (error.name(), error.outcome())
}

/// 3.10 e: the base container of a colour symbol.
fn colour_base_container() -> Vec<u8> {
    let digest = [
        0x53, 0x14, 0x09, 0xA7, 0x3A, 0xCC, 0xA0, 0x2F, 0xA0, 0x78, 0x07, 0xF5, 0x45, 0xBA, 0xB6,
        0xB8, 0xF0, 0x40, 0x3D, 0x7F, 0x96, 0xCA, 0x2A, 0xA1, 0xF8, 0x39, 0xA6, 0x73, 0xFD, 0xDB,
        0x53, 0x31,
    ];
    let mut body = vec![0x01];
    body.extend_from_slice(&digest);
    body.push(0x01);
    body.extend_from_slice(b"NMT Code");
    let container = sealed_container(0x10, &body);
    assert_eq!(container[container.len() - 4..], [0x05, 0x06, 0xD2, 0xA1]);
    container
}

#[test]
fn a_black_and_white_reader_presents_the_base_records_of_a_colour_symbol() {
    let decoded = read(&colour_base_container(), 1, SymbolClass::Static).unwrap();
    assert_eq!(decoded.outcome, Outcome::PresentedBaseOnly);
    assert_eq!(decoded.notice, Some(SpecError::ExtensionUnread));
    assert_eq!(decoded.notice.map(SpecError::name), Some("E_EXTENSION_UNREAD"));
    assert_eq!(decoded.records.len(), 1);
    assert_eq!(decoded.records[0].text(), Some("NMT Code"));
    assert_eq!(decoded.format.colour_profile(), 1);

    // A colour symbol whose base container has C = 0 holds every record in the base layer.
    let all_in_base = sealed_container(0x00, b"\x01NMT Code");
    let decoded = read(&all_in_base, 1, SymbolClass::Static).unwrap();
    assert_eq!(decoded.outcome, Outcome::Presented);
    assert_eq!(decoded.notice, None);
}

#[test]
fn every_container_check_gives_its_named_error() {
    // C = 1 in a black-and-white symbol.
    assert_eq!(error_of(&colour_base_container()), ("E_COLOUR_FLAG", Outcome::Malformed));
    // A CRC-32C that does not match.
    let mut bad_crc = sealed_container(0x00, b"\x01text");
    *bad_crc.last_mut().unwrap() ^= 1;
    assert_eq!(error_of(&bad_crc), ("E_CRC_MISMATCH", Outcome::Damaged));
    // Lb = 16383 places the CRC-32C beyond the capacity (K = 214 at 48 × 48, level 0).
    assert_eq!(error_of(&[0x00, 0xFF, 0x7F]), ("E_LENGTH_FIELD", Outcome::Damaged));
    // Container version 1.
    assert_eq!(
        error_of(&sealed_container(0x40, b"\x01text")),
        ("E_CONTAINER_VERSION", Outcome::Unsupported)
    );
    // Codec 6, reserved.
    assert_eq!(
        error_of(&sealed_container(0x06, b"\x04\x01text")),
        ("E_UNSUPPORTED_CODEC", Outcome::Unsupported)
    );
    // Codec escape below 15.
    assert_eq!(
        error_of(&sealed_container(0x0F, b"\x03\x00\x04\x01text")),
        ("E_CODEC_ESCAPE", Outcome::Malformed)
    );
    // A non-minimal LEB128 in the header.
    assert_eq!(
        error_of(&sealed_container(0x00, b"\x81\x00text")),
        ("E_LEB128", Outcome::Malformed)
    );
    // Header fields that run past the body: codec 3 with only its dictionary ID.
    assert_eq!(
        error_of(&sealed_container(0x03, b"\x00")),
        ("E_HEADER_OVERRUN", Outcome::Malformed)
    );
    // A dictionary that this reader does not carry.
    assert_eq!(
        error_of(&sealed_container(0x03, b"\x05\x04\x01abcd")),
        ("E_UNKNOWN_DICTIONARY", Outcome::Unsupported)
    );
    // L above MAX_CONTENT_LEN_V0.
    assert_eq!(
        error_of(&sealed_container(0x01, b"\x81\x80\x80\x08\x01")),
        ("E_MALFORMED", Outcome::Malformed)
    );
    // Codec 1 with a non-zero padding bit: "5" is 4 bits, 0101, then 0001 padding.
    assert_eq!(
        error_of(&sealed_container(0x01, b"\x01\x01\x51")),
        ("E_MALFORMED", Outcome::Malformed)
    );
    // Record count 0.
    assert_eq!(error_of(&sealed_container(0x20, b"\x00")), ("E_RECORD_LIST", Outcome::Malformed));
    // Bytes left over after the records.
    assert_eq!(
        error_of(&sealed_container(0x20, b"\x01\x01\x01ax")),
        ("E_RECORD_LIST", Outcome::Malformed)
    );
    // Two action records.
    assert_eq!(
        error_of(&sealed_container(0x20, b"\x02\x02\x01a\x02\x01b")),
        ("E_ACTION_RULE", Outcome::Malformed)
    );
    // A transfer tile.
    let tile = sealed_container(0x00, &[0; 16]);
    let error = read(&tile, 0, SymbolClass::TransferTile).unwrap_err();
    assert_eq!((error.name(), error.outcome()), ("E_TRANSFER_UNSUPPORTED", Outcome::Unsupported));
    let error = read(&sealed_container(0x01, &[0; 16]), 0, SymbolClass::TransferTile).unwrap_err();
    assert_eq!(error.name(), "E_TILE_RESERVED_BITS");
    // Hash algorithm ID 0 in a colour symbol.
    let error = read(&sealed_container(0x10, b"\x00\x01a"), 1, SymbolClass::Static).unwrap_err();
    assert_eq!((error.name(), error.outcome()), ("E_HASH_ID_INVALID", Outcome::Malformed));
}

#[test]
fn values_that_fail_their_type_are_presented_otherwise() {
    // Unknown content type 99: presented as unknown data.
    let decoded = read(&sealed_container(0x00, b"\x63abc"), 0, SymbolClass::Static).unwrap();
    assert_eq!(decoded.outcome, Outcome::Presented);
    assert_eq!(decoded.records[0].content_type, ContentType(99));
    assert_eq!(decoded.records[0].present_as, PresentAs::Unknown);
    assert_eq!(decoded.records[0].value, b"abc");
    // Text that is not UTF-8: presented as bytes with a notice.
    let decoded = read(&sealed_container(0x00, b"\x01\xFF\xFE"), 0, SymbolClass::Static).unwrap();
    assert_eq!(decoded.records[0].present_as, PresentAs::Bytes);
    assert_eq!(decoded.records[0].notice, Some(ValueNotice::NotUtf8));
    assert_eq!(decoded.records[0].text(), None);
    // A file name without a file after it: unknown data, and no name for the next record.
    let decoded =
        read(&sealed_container(0x20, b"\x02\x05\x01a\x01\x01b"), 0, SymbolClass::Static).unwrap();
    assert_eq!(decoded.records[0].present_as, PresentAs::Unknown);
    assert_eq!(decoded.file_name_for(1), None);
}

#[test]
fn the_reader_limit_is_applied_before_decoding() {
    let text = "x".repeat(1000);
    let stored = CodecOptions {
        digits: false,
        upper_alphanumeric: false,
        token_model: false,
        hangul: false,
        brotli: false,
    };
    for codecs in [stored, CodecOptions::default()] {
        let options = EncodeOptions { codecs, ..EncodeOptions::default() };
        let symbol = encode_text(&text, &options).unwrap();
        let error = decode(symbol.grid(), &DecodeOptions { limit: 999 }).unwrap_err();
        assert_eq!((error.name(), error.outcome()), ("E_TOO_LARGE", Outcome::Unsupported));
        let decoded = decode(symbol.grid(), &DecodeOptions { limit: 1000 }).unwrap();
        assert_eq!(decoded.records[0].value.len(), 1000);
    }
}

#[test]
fn encoder_options_and_errors() {
    let text = [Record { content_type: ContentType::TEXT, value: b"NMT Code" }];
    // The color profile is refused, not downgraded.
    let colour = EncodeOptions { profile: Profile::Color, ..EncodeOptions::default() };
    assert_eq!(encode(&text, &colour).unwrap_err(), EncodeError::ColourNotImplemented);
    // Profile levels (1.5) and the override.
    for (profile, level) in [(Profile::Screen, 0), (Profile::Print, 1), (Profile::LowEnd, 1)] {
        let options = EncodeOptions { profile, ..EncodeOptions::default() };
        assert_eq!(encode(&text, &options).unwrap().level(), level);
        let options = EncodeOptions { profile, level: Some(3), ..EncodeOptions::default() };
        assert_eq!(encode(&text, &options).unwrap().level(), 3);
    }
    let bad_level = EncodeOptions { level: Some(4), ..EncodeOptions::default() };
    assert_eq!(encode(&text, &bad_level).unwrap_err(), EncodeError::InvalidLevel(4));
    // Sizes.
    let odd = EncodeOptions {
        size: SizeRule::Exact { width: 22, height: 20 },
        ..EncodeOptions::default()
    };
    assert_eq!(
        encode(&text, &odd).unwrap_err(),
        EncodeError::InvalidSize { width: 22, height: 20 }
    );
    let small = EncodeOptions {
        size: SizeRule::Exact { width: 20, height: 20 },
        ..EncodeOptions::default()
    };
    let url = nmtcode::encode_url("https://github.com/needmoretruth/nmtcode", &small).unwrap_err();
    assert_eq!(
        url,
        EncodeError::DoesNotFit {
            width: 20,
            height: 20,
            level: 0,
            container_len: 28,
            capacity: 16
        }
    );
    // Record rules.
    assert_eq!(
        encode(&[], &EncodeOptions::default()).unwrap_err(),
        EncodeError::Records(WriteError::NoRecords)
    );
    let two_urls = [
        Record { content_type: ContentType::URL, value: b"a" },
        Record { content_type: ContentType::URL, value: b"b" },
    ];
    assert_eq!(
        encode(&two_urls, &EncodeOptions::default()).unwrap_err(),
        EncodeError::Records(WriteError::ActionRule)
    );
    // Content larger than the largest symbol.
    let stored = CodecOptions {
        digits: false,
        upper_alphanumeric: false,
        token_model: false,
        hangul: false,
        brotli: false,
    };
    let big = vec![0u8; 3_000_000];
    let options = EncodeOptions { codecs: stored, ..EncodeOptions::default() };
    let error =
        encode(&[Record { content_type: ContentType::FILE, value: &big }], &options).unwrap_err();
    assert!(matches!(error, EncodeError::NoSize { level: 0, .. }), "{error:?}");
}

#[test]
fn codec_restriction_and_never_compress() {
    let url = "https://github.com/needmoretruth/nmtcode";
    // Without codec 3 the URL goes to codec 4 (44 bytes against 47 stored, 6.12.5).
    let no_token = CodecOptions { token_model: false, ..CodecOptions::default() };
    let options = EncodeOptions { codecs: no_token, ..EncodeOptions::default() };
    let symbol = nmtcode::encode_url(url, &options).unwrap();
    assert_eq!(symbol.codec(), 4);
    assert_eq!(symbol.container_len(), 44);
    // A "never compress" type forces codec 0 even when codec 3 would be smaller.
    let signed = [Record { content_type: ContentType::COSE_SIGN1, value: url.as_bytes() }];
    let symbol = encode(&signed, &EncodeOptions::default()).unwrap();
    assert_eq!(symbol.codec(), 0);
    assert_eq!(symbol.record_form(), RecordForm::Single(ContentType::COSE_SIGN1));
    let decoded = decode(symbol.grid(), &DecodeOptions::default()).unwrap();
    assert_eq!(decoded.records[0].present_as, PresentAs::File);
}
