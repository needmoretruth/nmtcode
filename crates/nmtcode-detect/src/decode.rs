//! PNG decoding to an 8-bit luminance image.

use std::io::Cursor;

use crate::luma::{luma, over_white};
use crate::{DetectError, LumaImage, MAX_PIXELS};

/// The eight bytes every PNG file starts with.
pub(crate) const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn decoding_error(error: png::DecodingError) -> DetectError {
    match error {
        png::DecodingError::LimitsExceeded => {
            DetectError::Unsupported("the decoder's memory limit was exceeded".to_owned())
        }
        other => DetectError::Malformed(other.to_string()),
    }
}

/// Decodes a PNG of any colour type and bit depth into an 8-bit luminance image: 16-bit samples
/// are cut to 8 bits, palettes and low bit depths are expanded, colour is converted with the
/// ITU-R BT.601 weights and transparency is composited over white. For an animated PNG, the
/// default image is used.
///
/// The image size is checked against [`MAX_PIXELS`] from the header, before any pixel buffer is
/// allocated. The decoded buffer takes at most 4 bytes per pixel while the luminance is computed.
///
/// # Errors
///
/// [`DetectError::NotPng`] without the PNG signature, [`DetectError::TooLarge`] above
/// [`MAX_PIXELS`], [`DetectError::Malformed`] for damaged or truncated data and
/// [`DetectError::Unsupported`] when the decoder hits its memory limit on metadata.
pub fn decode_png(bytes: &[u8]) -> Result<LumaImage, DetectError> {
    if bytes.get(..PNG_SIGNATURE.len()) != Some(&PNG_SIGNATURE[..]) {
        return Err(DetectError::NotPng);
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let header = decoder.read_header_info().map_err(decoding_error)?;
    let (width, height) = (header.width, header.height);
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(DetectError::TooLarge { width, height });
    }
    let mut reader = decoder.read_info().map_err(decoding_error)?;
    let size = reader.output_buffer_size().ok_or(DetectError::TooLarge { width, height })?;
    let mut buffer = vec![0u8; size];
    let frame = reader.next_frame(&mut buffer).map_err(decoding_error)?;
    if frame.bit_depth != png::BitDepth::Eight {
        return Err(DetectError::Unsupported(format!("decoded bit depth {:?}", frame.bit_depth)));
    }
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(DetectError::Unsupported("palette left unexpanded".to_owned()));
        }
    };
    let (fw, fh) = (frame.width, frame.height);
    let cols = usize::try_from(fw).map_err(|_| DetectError::TooLarge { width, height })?;
    let rows = usize::try_from(fh).map_err(|_| DetectError::TooLarge { width, height })?;
    let line = frame.line_size;
    if line < cols * channels || buffer.len() < line * rows {
        return Err(DetectError::Malformed("short image data".to_owned()));
    }
    let mut pixels = Vec::with_capacity(cols * rows);
    for row in buffer.chunks(line).take(rows) {
        let row = row.get(..cols * channels).unwrap_or(&[]);
        if channels == 1 {
            pixels.extend_from_slice(row);
            continue;
        }
        pixels.extend(row.chunks_exact(channels).map(|px| match *px {
            [grey, alpha] => over_white(grey, alpha),
            [red, green, blue] => luma(red, green, blue),
            [red, green, blue, alpha] => over_white(luma(red, green, blue), alpha),
            _ => u8::MAX,
        }));
    }
    LumaImage::new(fw, fh, pixels).ok_or(DetectError::Malformed("short image data".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(luma(255, 255, 255), 255);
        assert_eq!(luma(255, 0, 0), 76);
        assert_eq!(over_white(0, 255), 0);
        assert_eq!(over_white(0, 0), 255);
        // (100 · 128 + 255 · 127) / 255 = 177.2, rounded to 177.
        assert_eq!(over_white(100, 128), 177);
    }

    #[test]
    fn not_png() {
        assert_eq!(decode_png(b""), Err(DetectError::NotPng));
        assert_eq!(decode_png(b"GIF89a....."), Err(DetectError::NotPng));
        assert!(matches!(decode_png(&PNG_SIGNATURE), Err(DetectError::Malformed(_))));
    }
}
