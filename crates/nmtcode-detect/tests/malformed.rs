//! Malformed input: every failure is an error value, never a panic.

// Test code: `as` casts convert small, known image coordinates and sizes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

mod common;

use common::*;
use nmtcode_detect::{DetectError, LumaImage, MAX_PIXELS, decode_png, find_symbols, read_png};
use nmtcode_render::{RenderOptions, render_png};
use proptest::prelude::*;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// A PNG whose header claims `width` × `height` 8-bit grey pixels, with a tiny IDAT.
fn header_only_png(width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    out.extend(chunk(*b"IHDR", &ihdr));
    out.extend(chunk(*b"IDAT", &[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]));
    out.extend(chunk(*b"IEND", &[]));
    out
}

fn sample_png() -> Vec<u8> {
    render_png(&a27(), &RenderOptions { module_px: 2, ..RenderOptions::default() }).unwrap()
}

#[test]
fn crc_helper_matches_the_png_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn not_a_png() {
    assert_eq!(read_png(b""), Err(DetectError::NotPng));
    assert_eq!(read_png(b"\x89PNG"), Err(DetectError::NotPng));
    assert_eq!(read_png(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"), Err(DetectError::NotPng));
    assert_eq!(read_png(&[0u8; 1000]), Err(DetectError::NotPng));
}

#[test]
fn truncated_png() {
    let png = sample_png();
    // The last 12 bytes are the IEND chunk; every cut before it loses header or image data.
    let iend = png.len() - 12;
    for len in [8, 9, 16, 33, 40, 60, png.len() / 2, iend - 1, iend - 4] {
        let result = read_png(&png[..len]);
        assert!(matches!(result, Err(DetectError::Malformed(_))), "length {len}: {result:?}");
    }
    // With all image data and the end chunk's header present, a cut CRC is tolerated.
    assert_eq!(read_png(&png[..png.len() - 1]).map(|v| v.len()), Ok(1));
}

#[test]
fn corrupted_checksum_and_data() {
    let png = sample_png();
    // Byte 29 is inside the IHDR CRC.
    let mut bad_crc = png.clone();
    bad_crc[29] ^= 0x55;
    assert!(matches!(read_png(&bad_crc), Err(DetectError::Malformed(_))));
    // Signature followed by junk.
    let mut junk = png[..8].to_vec();
    junk.extend_from_slice(&[0xAB; 64]);
    assert!(matches!(read_png(&junk), Err(DetectError::Malformed(_))));
}

#[test]
fn too_large_is_refused_before_allocation() {
    let result = decode_png(&header_only_png(20_000, 20_000));
    assert_eq!(result, Err(DetectError::TooLarge { width: 20_000, height: 20_000 }));
    let result = read_png(&header_only_png(1 << 30, 3));
    assert_eq!(result, Err(DetectError::TooLarge { width: 1 << 30, height: 3 }));
    // Exactly at the limit the header is accepted and the short data is the failure.
    assert_eq!(MAX_PIXELS, 100_000_000);
    let result = read_png(&header_only_png(10_000, 10_000));
    assert!(matches!(result, Err(DetectError::Malformed(_))), "{result:?}");
}

#[test]
fn header_with_zero_width() {
    assert!(matches!(read_png(&header_only_png(0, 10)), Err(DetectError::Malformed(_))));
}

#[test]
fn errors_display() {
    for e in [
        DetectError::NotPng,
        DetectError::Malformed("x".into()),
        DetectError::Unsupported("y".into()),
        DetectError::TooLarge { width: 1, height: 2 },
    ] {
        assert!(!e.to_string().is_empty());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = read_png(&bytes);
        let mut with_signature = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        with_signature.extend_from_slice(&bytes);
        let _ = read_png(&with_signature);
    }

    #[test]
    fn mutated_png_never_panics(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        let mut png = sample_png();
        for (at, value) in edits {
            let i = at % png.len();
            png[i] = value;
        }
        let keep = png.len() - cut % (png.len() / 4);
        let _ = read_png(&png[..keep]);
    }

    #[test]
    fn random_luma_never_panics(
        width in 1u32..120,
        height in 1u32..120,
        seed in any::<u64>(),
        levels in 2u64..5,
    ) {
        let mut rng = Rng(seed);
        let pixels: Vec<u8> = (0..width * height)
            .map(|_| (rng.below(levels) * 255 / (levels - 1)) as u8)
            .collect();
        let _ = find_symbols(&LumaImage::new(width, height, pixels).unwrap());
    }
}
