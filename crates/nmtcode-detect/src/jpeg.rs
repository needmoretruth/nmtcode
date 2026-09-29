//! JPEG decoding to an 8-bit luminance image, turned upright by the EXIF orientation tag.

use std::panic::{AssertUnwindSafe, catch_unwind};

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::luma::luma;
use crate::{DetectError, LumaImage, MAX_PIXELS};

/// Decodes a baseline or progressive JPEG into an 8-bit luminance image and applies its EXIF
/// orientation (tag 0x0112), so the result is the image as a viewer shows it.
///
/// A YCbCr or greyscale JPEG gives its Y channel, which JFIF defines with the BT.601 weights of
/// specification 1.4; other colour spaces are decoded to RGB and converted with those weights.
/// The declared width and height are checked against [`MAX_PIXELS`] before any pixel buffer is
/// allocated.
///
/// # Errors
///
/// [`DetectError::Jpeg`] for data the decoder refuses, [`DetectError::TooLarge`] above
/// [`MAX_PIXELS`].
pub fn decode_jpeg(bytes: &[u8]) -> Result<LumaImage, DetectError> {
    // The decoder is a third-party crate; a panic inside it on hostile input is turned into an
    // error here as a second line of defence.
    catch_unwind(AssertUnwindSafe(|| decode_inner(bytes)))
        .unwrap_or_else(|_| Err(DetectError::Jpeg("the decoder failed".to_owned())))
}

fn decode_inner(bytes: &[u8]) -> Result<LumaImage, DetectError> {
    let error = |e: zune_jpeg::errors::DecodeErrors| DetectError::Jpeg(e.to_string());
    let options = DecoderOptions::default().set_max_width(1 << 16).set_max_height(1 << 16);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(error)?;
    let (w, h) =
        decoder.dimensions().ok_or_else(|| DetectError::Jpeg("no image size".to_owned()))?;
    let width = u32::try_from(w).map_err(|_| DetectError::Jpeg("width".to_owned()))?;
    let height = u32::try_from(h).map_err(|_| DetectError::Jpeg("height".to_owned()))?;
    if width == 0 || height == 0 {
        return Err(DetectError::Jpeg("empty image".to_owned()));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(DetectError::TooLarge { width, height });
    }
    let orientation = decoder.exif().map_or(1, |exif| exif_orientation(exif));
    let direct = matches!(decoder.input_colorspace(), Some(ColorSpace::YCbCr | ColorSpace::Luma));
    let out = if direct { ColorSpace::Luma } else { ColorSpace::RGB };
    decoder.set_options(options.jpeg_set_out_colorspace(out));
    let channels = if direct { 1 } else { 3 };
    let size = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(channels))
        .ok_or(DetectError::TooLarge { width, height })?;
    let mut buffer = vec![0u8; size];
    decoder.decode_into(&mut buffer).map_err(error)?;
    let pixels: Vec<u8> = if direct {
        buffer
    } else {
        buffer.as_chunks::<3>().0.iter().map(|&[r, g, b]| luma(r, g, b)).collect()
    };
    let image = LumaImage::new(width, height, pixels)
        .ok_or_else(|| DetectError::Jpeg("short image data".to_owned()))?;
    Ok(orient(image, orientation))
}

/// The EXIF orientation (1 to 8) of TIFF-structured EXIF data, 1 when absent or unreadable.
pub(crate) fn exif_orientation(exif: &[u8]) -> u16 {
    // Some writers keep the "Exif\0\0" prefix of the APP1 segment.
    let data = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    let little = match data.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return 1,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b = data.get(at..at.checked_add(2)?)?;
        let pair = [*b.first()?, *b.get(1)?];
        Some(if little { u16::from_le_bytes(pair) } else { u16::from_be_bytes(pair) })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b = data.get(at..at.checked_add(4)?)?;
        let quad = [*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?];
        Some(if little { u32::from_le_bytes(quad) } else { u32::from_be_bytes(quad) })
    };
    let read = || -> Option<u16> {
        if u16_at(2)? != 42 {
            return None;
        }
        let ifd = usize::try_from(u32_at(4)?).ok()?;
        let count = usize::from(u16_at(ifd)?);
        // At most 1000 entries are looked at: bounded work on any input.
        for i in 0..count.min(1000) {
            let entry = ifd.checked_add(2)?.checked_add(i.checked_mul(12)?)?;
            if u16_at(entry)? == 0x0112 {
                let value = u16_at(entry.checked_add(8)?)?;
                return (1..=8).contains(&value).then_some(value);
            }
        }
        None
    };
    read().unwrap_or(1)
}

/// `image` as displayed under EXIF orientation `orientation`.
pub(crate) fn orient(image: LumaImage, orientation: u16) -> LumaImage {
    if !(2..=8).contains(&orientation) {
        return image;
    }
    let (w, h) = (image.w_usize(), image.h_usize());
    let swap = orientation >= 5;
    let (dw, dh) = if swap { (h, w) } else { (w, h) };
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..dh {
        for x in 0..dw {
            // Stored position of displayed pixel (x, y).
            let (sx, sy) = match orientation {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                _ => (w - 1 - y, x),
            };
            pixels.push(image.pixels.get(sy * w + sx).copied().unwrap_or(u8::MAX));
        }
    }
    let (Ok(dw), Ok(dh)) = (u32::try_from(dw), u32::try_from(dh)) else { return image };
    LumaImage::new(dw, dh, pixels).unwrap_or(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exif(little: bool, orientation: u16) -> Vec<u8> {
        let mut out = Vec::new();
        let p16 = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let p32 = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        out.extend_from_slice(if little { b"II" } else { b"MM" });
        out.extend_from_slice(&p16(42));
        out.extend_from_slice(&p32(8));
        out.extend_from_slice(&p16(2));
        // An unrelated entry, then the orientation (SHORT, count 1).
        out.extend_from_slice(&p16(0x010F));
        out.extend_from_slice(&p16(2));
        out.extend_from_slice(&p32(4));
        out.extend_from_slice(&p32(0));
        out.extend_from_slice(&p16(0x0112));
        out.extend_from_slice(&p16(3));
        out.extend_from_slice(&p32(1));
        out.extend_from_slice(&p16(orientation));
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn orientation_tag() {
        for o in 1..=8 {
            assert_eq!(exif_orientation(&exif(true, o)), o);
            assert_eq!(exif_orientation(&exif(false, o)), o);
        }
        let mut prefixed = b"Exif\0\0".to_vec();
        prefixed.extend(exif(false, 6));
        assert_eq!(exif_orientation(&prefixed), 6);
        assert_eq!(exif_orientation(&exif(true, 9)), 1);
        assert_eq!(exif_orientation(b"II*"), 1);
        assert_eq!(exif_orientation(&[]), 1);
        let mut cut = exif(true, 6);
        cut.truncate(20);
        assert_eq!(exif_orientation(&cut), 1);
    }

    #[test]
    fn orientations_move_the_top_left_pixel() {
        // 3 × 2 image, values 0..6 row by row.
        let image = LumaImage::new(3, 2, (0..6).collect()).unwrap();
        let expect = [
            (1, 3, vec![0, 1, 2, 3, 4, 5]),
            (2, 3, vec![2, 1, 0, 5, 4, 3]),
            (3, 3, vec![5, 4, 3, 2, 1, 0]),
            (4, 3, vec![3, 4, 5, 0, 1, 2]),
            (5, 2, vec![0, 3, 1, 4, 2, 5]),
            (6, 2, vec![3, 0, 4, 1, 5, 2]),
            (7, 2, vec![5, 2, 4, 1, 3, 0]),
            (8, 2, vec![2, 5, 1, 4, 0, 3]),
        ];
        for (o, width, pixels) in expect {
            let turned = orient(image.clone(), o);
            assert_eq!((turned.width, turned.pixels), (width, pixels), "orientation {o}");
        }
    }

    #[test]
    fn not_a_jpeg() {
        assert!(matches!(decode_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0]), Err(DetectError::Jpeg(_))));
        assert!(matches!(decode_jpeg(b""), Err(DetectError::Jpeg(_))));
    }
}
