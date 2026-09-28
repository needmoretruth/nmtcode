//! Malformed input for every rule of 6.4 and the registry rules of 6.2 and 6.11.

#![allow(missing_docs)]

mod common;

use nmtcode_payload::CodecError::{
    DictionaryMismatch, Malformed, TooLarge, UnknownDictionary, UnsupportedCodec,
};
use nmtcode_payload::{
    CodecError, MAX_CONTENT_LEN_V0, check_decoded_len, codec1, codec2, codec3, codec4, codec5,
    decode, is_supported, leb128, takes_dictionary,
};

const MAX: u32 = MAX_CONTENT_LEN_V0;

fn d(codec: u32, dictionary: u32, len: u32, bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    decode(codec, dictionary, len, bytes, MAX)
}

// ---- 6.2 and 6.11: codec and dictionary IDs -------------------------------

#[test]
fn reserved_codecs_are_unsupported_before_other_checks() {
    for codec in (6..=20).chain([u32::MAX]) {
        assert!(!is_supported(codec));
        // Checked before the dictionary ID and L.
        assert_eq!(decode(codec, 99, u32::MAX, b"x", 0), Err(UnsupportedCodec));
    }
    assert_eq!(is_supported(5), cfg!(feature = "brotli"));
    for codec in 0..=4 {
        assert!(is_supported(codec));
    }
    assert!(takes_dictionary(3) && takes_dictionary(5));
    for codec in [0, 1, 2, 4, 6, 15, 16] {
        assert!(!takes_dictionary(codec));
    }
}

#[cfg(not(feature = "brotli"))]
#[test]
fn codec5_without_the_feature_is_unsupported() {
    assert_eq!(d(5, 0, 0, &[0x06]), Err(UnsupportedCodec));
    assert_eq!(codec5::decode(0, &[0x06], MAX), Err(UnsupportedCodec));
}

#[test]
fn unknown_and_private_dictionaries() {
    let codecs: &[u32] = if cfg!(feature = "brotli") { &[3, 5] } else { &[3] };
    for &codec in codecs {
        for id in [1, 2, 127, 128, 16_383, 16_384, 20_000, 32_767, 32_768, u32::MAX] {
            // Checked before L.
            assert_eq!(decode(codec, id, u32::MAX, b"", 0), Err(UnknownDictionary));
        }
    }
}

#[test]
fn dictionary_id_on_a_codec_without_one() {
    for codec in [0, 1, 2, 4] {
        assert_eq!(d(codec, 1, 0, b""), Err(Malformed));
    }
}

#[test]
fn dictionary_kind_check_error_exists() {
    // The kind check itself is exercised with a test registry inside the crate
    // (dictionary::tests); in 0.2 no ID can reach it from `decode`.
    assert_ne!(DictionaryMismatch, UnknownDictionary);
    assert_eq!(DictionaryMismatch.spec_name(), "E_DICTIONARY_MISMATCH");
}

// ---- 6.4 rule 1: L before allocation --------------------------------------

#[test]
fn l_beyond_max_content_len_is_malformed() {
    for codec in (0..=5).filter(|&c| is_supported(c)) {
        for len in [MAX + 1, u32::MAX] {
            // Even with the largest reader limit, and before any codec check.
            assert_eq!(decode(codec, 0, len, b"", u32::MAX), Err(Malformed), "codec {codec}");
        }
    }
    assert_eq!(check_decoded_len(MAX, MAX), Ok(()));
    assert_eq!(check_decoded_len(MAX + 1, u32::MAX), Err(Malformed));
}

#[test]
fn l_beyond_limit_is_too_large() {
    for codec in (0..=5).filter(|&c| is_supported(c)) {
        // Checked before the codec's own length check: the coded field is wrong
        // for every codec here, yet the answer is E_TOO_LARGE.
        assert_eq!(decode(codec, 0, MAX, b"", MAX - 1), Err(TooLarge), "codec {codec}");
        assert_eq!(decode(codec, 0, 11, b"", 10), Err(TooLarge), "codec {codec}");
    }
    assert_eq!(decode(0, 0, 3, b"abc", 3), Ok(b"abc".to_vec()));
    assert_eq!(decode(0, 0, 3, b"abc", 2), Err(TooLarge));
}

// ---- 6.4 rule 2: codec-specific length checks -----------------------------

#[test]
fn codec0_length_must_match() {
    assert_eq!(d(0, 0, 2, b"abc"), Err(Malformed));
    assert_eq!(d(0, 0, 4, b"abc"), Err(Malformed));
}

#[test]
fn codec1_coded_length_must_be_exact() {
    let good = codec1::encode(b"0123456789").unwrap();
    assert_eq!(d(1, 0, 10, &good), Ok(b"0123456789".to_vec()));
    assert_eq!(d(1, 0, 10, &good[..4]), Err(Malformed));
    let mut long = good.clone();
    long.push(0);
    assert_eq!(d(1, 0, 10, &long), Err(Malformed));
    // L = 11 and L = 12 also take 5 bytes (37 and 40 bits); L = 13 takes 44
    // bits = 6 bytes.
    assert_eq!(d(1, 0, 11, &good), Ok(b"01234567872".to_vec()));
    assert_eq!(d(1, 0, 13, &good), Err(Malformed));
    assert_eq!(d(1, 0, 0, b""), Ok(Vec::new()));
    assert_eq!(d(1, 0, 0, b"\0"), Err(Malformed));
    // A huge L with a short field fails before allocating.
    assert_eq!(d(1, 0, MAX, &good), Err(Malformed));
}

#[test]
fn codec2_coded_length_must_be_exact() {
    let good = codec2::encode(b"HTTPS://EXAMPLE.COM/ABC").unwrap();
    assert_eq!(d(2, 0, 23, &good[..15]), Err(Malformed));
    let mut long = good.clone();
    long.push(0);
    assert_eq!(d(2, 0, 23, &long), Err(Malformed));
    // L = 22 is 121 bits = 16 bytes too, so the length check passes and the
    // unused 7 bits of the last byte must be zero: here they are not.
    assert_eq!(d(2, 0, 22, &good), Err(Malformed));
    assert_eq!(d(2, 0, MAX, &good), Err(Malformed));
}

#[test]
fn codec4_length_bound() {
    let good = codec4::encode("안녕하세요 NMT Code".as_bytes()).unwrap();
    assert_eq!(good.len(), 18);
    // 7·L ≤ 16·Lc: with Lc = 18, L = 41 is the largest allowed.
    assert_eq!(d(4, 0, 42, &good), Err(Malformed));
    assert_eq!(d(4, 0, MAX, &good), Err(Malformed));
    // L = 0 requires Lc = 0.
    assert_eq!(d(4, 0, 0, b""), Ok(Vec::new()));
    assert_eq!(d(4, 0, 0, &[0x00]), Err(Malformed));
}

// ---- 6.4 rule 4: exactly L bytes -------------------------------------------

#[test]
fn codec3_token_beyond_l() {
    let coded = codec3::encode(b"https://", &codec3::Model::model0());
    assert_eq!(d(3, 0, 8, &coded), Ok(b"https://".to_vec()));
    // The first symbol is an 8-byte token; with L = 5 it goes beyond L.
    assert_eq!(d(3, 0, 5, &coded), Err(Malformed));
}

#[test]
fn codec4_unit_beyond_l_and_early_end() {
    let content = "요 x".as_bytes();
    let coded = codec4::encode(content).unwrap();
    assert_eq!(d(4, 0, 5, &coded), Ok(content.to_vec()));
    // The first unit is syllable + space (4 bytes); L = 3 stops inside it.
    assert_eq!(d(4, 0, 3, &coded), Err(Malformed));
    // The field ends before L bytes are produced.
    assert_eq!(d(4, 0, 6, &coded), Err(Malformed));
}

#[cfg(feature = "brotli")]
#[test]
fn brotli_output_must_be_exactly_l() {
    let content = b"hello hello hello hello hello, brotli";
    let coded = codec5::encode(content).unwrap();
    let len = u32::try_from(content.len()).unwrap();
    assert_eq!(d(5, 0, len, &coded), Ok(content.to_vec()));
    // More than L bytes: the decoder stops at the first byte beyond L.
    assert_eq!(d(5, 0, len - 1, &coded), Err(Malformed));
    assert_eq!(d(5, 0, 0, &coded), Err(Malformed));
    // Fewer than L bytes.
    assert_eq!(d(5, 0, len + 1, &coded), Err(Malformed));
    // A stream that declares far more than L (4 MiB of zeros in a few bytes).
    let zeros = vec![0u8; 4 << 20];
    let bomb = codec5::encode(&zeros).unwrap();
    assert!(bomb.len() < 32);
    assert_eq!(d(5, 0, 10, &bomb), Err(Malformed));
}

/// 6.4 rule 2: L ≤ 256 · (Lc + 4) for codec 5, checked before allocation, and
/// the encoder offers no candidate beyond it.
#[cfg(feature = "brotli")]
#[test]
fn brotli_expansion_bound() {
    let zeros = vec![0u8; 4 << 20];
    let bomb = codec5::encode(&zeros).unwrap();
    assert_eq!(d(5, 0, 4 << 20, &bomb), Err(Malformed));
    let all = nmtcode_payload::candidates(&zeros, &nmtcode_payload::EncodeOptions::ALL);
    assert!(all.iter().all(|c| c.codec != 5 && c.codec != 3), "{:?}", all.len());
    // At the bound itself the stream is decoded.
    let content = vec![b'a'; 4000];
    let coded = codec5::encode(&content).unwrap();
    let bound = codec5::EXPANSION_MAX as usize * (coded.len() + 4);
    assert!(content.len() <= bound);
    assert_eq!(d(5, 0, 4000, &coded).map(|v| v.len()), Ok(4000));
    assert!(nmtcode_payload::within_expansion(1024, 0, 256));
    assert!(!nmtcode_payload::within_expansion(1025, 0, 256));
}

// ---- 6.4 rule 5: out-of-range values, reserved codes, trailing data, padding

#[test]
fn codec1_group_values_out_of_range() {
    // 3 digits: 10 bits 1111101000 = 1000; padded to 2 bytes.
    assert_eq!(d(1, 0, 3, &[0b1111_1010, 0b0000_0000]), Err(Malformed));
    assert_eq!(d(1, 0, 3, &[0b1111_1001, 0b1100_0000]), Ok(b"999".to_vec()));
    // 2 digits: 7 bits 1100100 = 100.
    assert_eq!(d(1, 0, 2, &[0b1100_1000]), Err(Malformed));
    assert_eq!(d(1, 0, 2, &[0b1100_0110]), Ok(b"99".to_vec()));
    // 1 digit: 4 bits 1010 = 10.
    assert_eq!(d(1, 0, 1, &[0b1010_0000]), Err(Malformed));
    assert_eq!(d(1, 0, 1, &[0b1001_0000]), Ok(b"9".to_vec()));
}

#[test]
fn codec2_values_out_of_range() {
    // Pair 2025 = 11111101001, padded to 2 bytes.
    assert_eq!(d(2, 0, 2, &[0b1111_1101, 0b0010_0000]), Err(Malformed));
    // Pair 2024 = "::".
    assert_eq!(d(2, 0, 2, &[0b1111_1101, 0b0000_0000]), Ok(b"::".to_vec()));
    // Single 45 = 101101.
    assert_eq!(d(2, 0, 1, &[0b1011_0100]), Err(Malformed));
    assert_eq!(d(2, 0, 1, &[0b1011_0000]), Ok(b":".to_vec()));
}

#[test]
fn non_zero_padding_bits() {
    // 6.12.1: 6 padding bits in the last byte 0x40.
    assert_eq!(d(1, 0, 10, &[0x03, 0x15, 0x9A, 0x9A, 0x41]), Err(Malformed));
    assert_eq!(d(1, 0, 10, &[0x03, 0x15, 0x9A, 0x9A, 0x60]), Err(Malformed));
    // 6.12.2: 1 padding bit in the last byte 0x98.
    let mut c2 = codec2::encode(b"HTTPS://EXAMPLE.COM/ABC").unwrap();
    *c2.last_mut().unwrap() |= 1;
    assert_eq!(d(2, 0, 23, &c2), Err(Malformed));
    // 6.12.4: 3 padding bits in the last byte 0x28.
    let mut c4 = codec4::encode("안녕하세요 NMT Code".as_bytes()).unwrap();
    *c4.last_mut().unwrap() |= 1;
    assert_eq!(d(4, 0, 24, &c4), Err(Malformed));
}

#[test]
fn codec4_trailing_data() {
    let mut coded = codec4::encode("안녕하세요 NMT Code".as_bytes()).unwrap();
    coded.push(0x00);
    assert_eq!(d(4, 0, 24, &coded), Err(Malformed));
}

/// A codec-4 field from a mode bit and 14-bit H units (plus raw extra bits).
fn h_units(fields: &[(u32, u32)]) -> Vec<u8> {
    let mut bits: Vec<bool> = vec![false];
    for &(value, width) in fields {
        bits.extend((0..width).rev().map(|i| (value >> i) & 1 == 1));
    }
    bits.chunks(8)
        .map(|chunk| {
            chunk.iter().enumerate().fold(0u8, |byte, (i, &bit)| byte | (u8::from(bit) << (7 - i)))
        })
        .collect()
}

#[test]
fn codec4_reserved_units_and_bad_escapes() {
    for reserved in 16_365..16_384 {
        assert_eq!(d(4, 0, 1, &h_units(&[(reserved, 14)])), Err(Malformed));
    }
    // ESC16: U+AC00 is fine (a direct code exists, but the decoder accepts).
    assert_eq!(d(4, 0, 3, &h_units(&[(16_363, 14), (0xAC00, 16)])), Ok("가".as_bytes().to_vec()));
    for surrogate in [0xD800, 0xDBFF, 0xDC00, 0xDFFF] {
        assert_eq!(d(4, 0, 3, &h_units(&[(16_363, 14), (surrogate, 16)])), Err(Malformed));
        assert_eq!(d(4, 0, 3, &h_units(&[(16_364, 14), (surrogate, 21)])), Err(Malformed));
    }
    assert_eq!(
        d(4, 0, 4, &h_units(&[(16_364, 14), (0x10_FFFF, 21)])),
        Ok("\u{10FFFF}".as_bytes().to_vec())
    );
    for too_big in [0x11_0000, 0x1F_FFFF] {
        assert_eq!(d(4, 0, 4, &h_units(&[(16_364, 14), (too_big, 21)])), Err(Malformed));
    }
    // A read past the end: ESC21 with only 16 bits of payload.
    assert_eq!(d(4, 0, 4, &h_units(&[(16_364, 14), (0x1F60, 16)])), Err(Malformed));
}

#[test]
fn codec3_out_of_range_value_and_trailing_data() {
    // code = 0xFFFFFFFF, r = 0xFFFF: code / r = 65537 ≥ 2^16.
    assert_eq!(d(3, 0, 1, &[0xFF, 0xFF, 0xFF, 0xFF]), Err(Malformed));
    let content = b"https://github.com/needmoretruth/nmtcode";
    let good = codec3::encode(content, &codec3::Model::model0());
    // The test vector of 6.8.5: the 19-byte field of 6.12.3 with a byte 01 appended. The
    // decoder reads 22 bytes (M = 22), so the old checks (Lc ≤ p, no final 00) passed it; the
    // canonical-termination check rejects it.
    let mut appended = good.clone();
    appended.push(0x01);
    assert_eq!(appended.len(), 20);
    assert_eq!(d(3, 0, 40, &good), Ok(content.to_vec()));
    assert_eq!(d(3, 0, 40, &appended), Err(Malformed));
    // A trailing 0x00 byte.
    let mut zero = good.clone();
    zero.push(0x00);
    assert_eq!(d(3, 0, 40, &zero), Err(Malformed));
    // Bytes the decoder never read: M = 22 for this parse (6.12.3).
    let mut unread = good.clone();
    unread.extend_from_slice(&[0x01; 4]);
    assert_eq!(unread.len(), 23);
    assert_eq!(d(3, 0, 40, &unread), Err(Malformed));
    // Empty content: at most 4 bytes are read.
    assert_eq!(d(3, 0, 0, &[0x01, 0x02, 0x03, 0x04, 0x05]), Err(Malformed));
    assert_eq!(d(3, 0, 0, &[0x01, 0x00]), Err(Malformed));
}

/// 6.8.5: the decoder accepts only the canonical termination, the one coded
/// field the encoder writes for a symbol sequence. `55 55 55 55` lies in the
/// final interval of `a` as well, and `56` is the canonical field.
#[test]
fn codec3_accepts_only_the_canonical_termination() {
    let canonical = codec3::encode(b"a", &codec3::Model::model0());
    assert_eq!(canonical, [0x56]);
    assert_eq!(d(3, 0, 1, &[0x56]), Ok(b"a".to_vec()));
    assert_eq!(d(3, 0, 1, &[0x55, 0x55, 0x55, 0x55]), Err(Malformed));
    assert_eq!(d(3, 0, 1, &[0x57]), Err(Malformed));
    assert_eq!(d(3, 0, 1, &[0x56, 0x01]), Err(Malformed));
    assert_eq!(d(3, 0, 1, &[0x55, 0x55, 0x55, 0x55, 0x55]), Err(Malformed));
    // L = 0: only the empty field.
    assert_eq!(d(3, 0, 0, &[]), Ok(Vec::new()));
    assert_eq!(d(3, 0, 0, &[0x01]), Err(Malformed));
}

/// 6.4 rule 2: L ≤ 64 · (Lc + 4) for codec 3, checked before allocation.
#[test]
fn codec3_expansion_bound() {
    // Content of NUL bytes codes to an empty field under model 0 (low stays 0).
    let zeros = vec![0u8; 256];
    assert!(codec3::encode(&zeros, &codec3::Model::model0()).is_empty());
    assert_eq!(d(3, 0, 256, &[]), Ok(zeros.clone()));
    assert_eq!(d(3, 0, 257, &[]), Err(Malformed));
    assert_eq!(d(3, 0, 16 << 20, &[]), Err(Malformed));
    // The encoder offers no codec-3 candidate beyond the bound.
    let longer = vec![0u8; 257];
    let all = nmtcode_payload::candidates(&longer, &nmtcode_payload::EncodeOptions::ALL);
    assert!(all.iter().all(|c| c.codec != 3));
}

#[test]
fn codec3_zero_extension_is_not_a_read_past_the_end() {
    // Content of NUL bytes: model-0 low stays 0, so C is empty.
    let coded = codec3::encode(&[0, 0, 0], &codec3::Model::model0());
    assert!(coded.is_empty());
    assert_eq!(d(3, 0, 3, &coded), Ok(vec![0, 0, 0]));
    assert_eq!(d(3, 0, 0, b""), Ok(Vec::new()));
}

#[cfg(feature = "brotli")]
#[test]
fn brotli_trailing_bytes_and_padding_bits() {
    // Empty content is the single byte 06: WBITS=16 (bit 0), ISLAST (bit 1),
    // ISLASTEMPTY (bit 2); bits 3–7 are the unused bits of the final byte.
    assert_eq!(d(5, 0, 0, &[0x06]), Ok(Vec::new()));
    for padding in [0x0E, 0x16, 0x86, 0xFE] {
        assert_eq!(d(5, 0, 0, &[padding]), Err(Malformed));
    }
    assert_eq!(d(5, 0, 0, &[0x06, 0x00]), Err(Malformed));
    assert_eq!(d(5, 0, 0, &[0x06, 0x06]), Err(Malformed));
    let content = b"NMT Code NMT Code NMT Code";
    let coded = codec5::encode(content).unwrap();
    let len = u32::try_from(content.len()).unwrap();
    for extra in [0x00u8, 0x01, 0xFF] {
        let mut trailing = coded.clone();
        trailing.push(extra);
        assert_eq!(d(5, 0, len, &trailing), Err(Malformed));
    }
    // Truncated streams.
    for cut in 0..coded.len() {
        assert_eq!(d(5, 0, len, &coded[..cut]), Err(Malformed));
    }
}

#[cfg(feature = "brotli")]
#[test]
fn brotli_metadata_meta_blocks_are_skipped() {
    // WBITS 16; a metadata meta-block (ISLAST 0, MNIBBLES code 3, reserved 0,
    // MSKIPBYTES 1, MSKIPLEN − 1 = 2) with 3 bytes; then ISLAST, ISLASTEMPTY.
    let stream = [0x2C, 0x01, b'M', b'D', b'!', 0x03];
    assert_eq!(d(5, 0, 0, &stream), Ok(Vec::new()));
    assert_eq!(d(5, 0, 1, &stream), Err(Malformed));
    // The reserved bit set is invalid.
    assert_eq!(d(5, 0, 0, &[0x3C, 0x01, b'M', b'D', b'!', 0x03]), Err(Malformed));
}

#[cfg(feature = "brotli")]
mod large_window {
    use brotli::enc::encode::{
        BrotliEncoderOperation, BrotliEncoderParameter, BrotliEncoderStateStruct,
    };
    use brotli::enc::interface::PredictionModeContextMap;
    use brotli::enc::{BrotliAlloc, StaticCommand};
    use brotli::{InputPair, InputReferenceMut};
    use brotli_decompressor::{
        Allocator, BrotliDecompressStream, BrotliResult, BrotliState, SliceWrapper, SliceWrapperMut,
    };

    use super::d;
    use nmtcode_payload::CodecError::Malformed;

    struct Block<T>(Box<[T]>);
    impl<T> Default for Block<T> {
        fn default() -> Self {
            Self(Box::default())
        }
    }
    impl<T> SliceWrapper<T> for Block<T> {
        fn slice(&self) -> &[T] {
            &self.0
        }
    }
    impl<T> SliceWrapperMut<T> for Block<T> {
        fn slice_mut(&mut self) -> &mut [T] {
            &mut self.0
        }
    }
    struct Heap;
    impl<T: Clone + Default> Allocator<T> for Heap {
        type AllocatedMemory = Block<T>;
        fn alloc_cell(&mut self, len: usize) -> Block<T> {
            Block(vec![T::default(); len].into_boxed_slice())
        }
        fn free_cell(&mut self, _data: Block<T>) {}
    }
    impl BrotliAlloc for Heap {}

    fn callback(
        _: &mut PredictionModeContextMap<InputReferenceMut<'_>>,
        _: &mut [StaticCommand],
        _: InputPair<'_>,
        _: &mut Heap,
    ) {
    }

    #[test]
    fn brotli_large_window_streams_are_malformed() {
        let content = b"large window stream, large window stream";
        let mut state = BrotliEncoderStateStruct::new(Heap);
        assert!(state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_LARGE_WINDOW, 1));
        assert!(state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_LGWIN, 26));
        let mut out = vec![0u8; 1024];
        let (mut available_in, mut input_offset) = (content.len(), 0);
        let (mut available_out, mut output_offset, mut total_out) = (out.len(), 0, None);
        assert!(state.compress_stream(
            BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
            &mut available_in,
            content,
            &mut input_offset,
            &mut available_out,
            &mut out,
            &mut output_offset,
            &mut total_out,
            &mut callback,
        ));
        assert!(state.is_finished());
        out.truncate(output_offset);
        // The large-window marker: WBITS field 0010001 (read from the low bit).
        assert_eq!(out[0] & 0x7F, 0x11);
        let len = u32::try_from(content.len()).unwrap();
        assert_eq!(d(5, 0, len, &out), Err(Malformed));
        // The same stream is valid under the extension, so the rejection is
        // the window rule and not a broken stream.
        let mut decoded = vec![0u8; content.len()];
        let mut reader: BrotliState<Heap, Heap, Heap> = BrotliState::new(Heap, Heap, Heap);
        reader.large_window = true;
        let (mut available_in, mut input_offset) = (out.len(), 0);
        let (mut available_out, mut output_offset, mut total) = (decoded.len(), 0, 0);
        let result = BrotliDecompressStream(
            &mut available_in,
            &mut input_offset,
            &out,
            &mut available_out,
            &mut output_offset,
            &mut decoded,
            &mut total,
            &mut reader,
        );
        assert!(matches!(result, BrotliResult::ResultSuccess));
        assert_eq!(&decoded, content);
    }
}

// ---- LEB128 (1.4) and short-text model dictionaries (6.8.4) ----------------

#[test]
fn leb128_rejects_non_minimal_long_and_large() {
    for bad in [
        &[0x80u8, 0x00][..],
        &[0xFF, 0x00],
        &[0x80, 0x80, 0x80, 0x80, 0x00],
        &[0xFF, 0xFF, 0xFF, 0xFF, 0x10],
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        &[0x80],
        &[],
    ] {
        assert_eq!(leb128::read(bad), Err(Malformed), "{bad:02X?}");
    }
    assert_eq!(leb128::read(&[0x00, 0xFF]), Ok((0, 1)));
}

#[test]
fn model_dictionary_validation() {
    let good = common::order2_model();
    assert!(codec3::Model::from_dictionary(&good).is_ok());
    let model0 = common::model0_as_dictionary();
    assert!(codec3::Model::from_dictionary(&model0).is_ok());
    let reject = |bytes: &[u8]| assert_eq!(codec3::Model::from_dictionary(bytes), Err(Malformed));

    // Header fields.
    let with = |offset: usize, value: u8| {
        let mut bytes = good.clone();
        bytes[offset] = value;
        bytes
    };
    reject(&with(0, 1)); // format
    reject(&with(1, 5)); // order k > 4
    reject(&with(1, 0)); // k = 0 with b ≠ 0
    reject(&with(2, 13)); // b > 12
    reject(&with(2, 0)); // b = 0 with k ≠ 0
    reject(&with(6, 0xF1)); // A = 497
    reject(&with(5, 0x00)); // A = 240
    // V = 0 and V > 4096.
    let mut zero_vectors = model0.clone();
    zero_vectors[3..5].copy_from_slice(&0u16.to_be_bytes());
    reject(&zero_vectors);
    let mut too_many = model0.clone();
    too_many[3..5].copy_from_slice(&4097u16.to_be_bytes());
    reject(&too_many);
    // A slot pointing past the vectors (V = 3).
    let mut slot = good.clone();
    slot[7..9].copy_from_slice(&3u16.to_be_bytes());
    reject(&slot);
    // Trailing byte, missing byte.
    let mut trailing = good.clone();
    trailing.push(0);
    reject(&trailing);
    reject(&good[..good.len() - 1]);
    reject(&[]);

    // Vectors: sum ≠ 2^16, symbol ≥ A, n > A, non-minimal LEB128.
    let vector = |entries: common::Entries| common::model_bytes(0, 0, &[0], &[entries]);
    assert!(codec3::Model::from_dictionary(&vector(vec![(0, 65_536 - 495)])).is_ok());
    reject(&vector(vec![(0, 65_536 - 494)]));
    reject(&vector(vec![(0, 65_536 - 496)]));
    reject(&vector(vec![(496, 65_536 - 495)]));
    let mut header = vec![0, 0, 0, 0, 1, 0x01, 0xF0, 0, 0];
    let mut n_too_big = header.clone();
    leb128::write(497, &mut n_too_big);
    reject(&n_too_big);
    // n = 1, gap = 0 written as 80 00, e = 65 039.
    header.extend_from_slice(&[0x01, 0x80, 0x00]);
    leb128::write(65_536 - 495 - 2, &mut header);
    reject(&header);
}

#[test]
fn model_dictionary_accepts_boundaries() {
    // A vector with every symbol explicit, the last at A − 1.
    let mut entries: common::Entries = (0u16..495).map(|s| (s, 2)).collect();
    entries.push((495, 65_536 - 2 * 495));
    let bytes = common::model_bytes(0, 0, &[0], &[entries]);
    let model = codec3::Model::from_dictionary(&bytes).unwrap();
    let content = b"boundary \xFF\x00 model";
    let coded = codec3::encode(content, &model);
    let len = u32::try_from(content.len()).unwrap();
    assert_eq!(codec3::decode(&model, len, &coded, MAX), Ok(content.to_vec()));
    // Order 4 with 2^12 slots.
    let slotmap = vec![0u16; 4096];
    let bytes = common::model_bytes(4, 12, &slotmap, &[vec![(0, 65_536 - 495)]]);
    assert!(codec3::Model::from_dictionary(&bytes).is_ok());
}
