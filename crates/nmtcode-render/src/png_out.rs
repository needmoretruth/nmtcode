//! PNG output: 1-bit greyscale, written row by row, with the print growth compensation of
//! specification 1.5 applied per pixel.

use std::io::Write;

use crate::canvas::Canvas;
use crate::growth::{bands, eroded};
use crate::{RenderError, RenderOptions};

fn png_error(error: impl core::fmt::Display) -> RenderError {
    RenderError::Png(error.to_string())
}

/// Pixels per metre for `dpi` dots per inch, rounded: dpi / 0.0254.
pub(crate) fn pixels_per_metre(dpi: u32) -> u32 {
    let ppm = (u64::from(dpi) * 10_000 + 127) / 254;
    u32::try_from(ppm).unwrap_or(u32::MAX)
}

pub(crate) fn write(canvas: &Canvas, options: &RenderOptions) -> Result<Vec<u8>, RenderError> {
    let layout = &canvas.layout;
    let width = u32::try_from(layout.pixel_width()).map_err(|_| RenderError::TooLarge)?;
    let height = u32::try_from(layout.pixel_height()).map_err(|_| RenderError::TooLarge)?;
    let width_px = usize::try_from(width).map_err(|_| RenderError::TooLarge)?;
    let row_bytes = width_px.div_ceil(8);

    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::One);
    if let Some(dpi) = options.dpi {
        let ppm = pixels_per_metre(dpi);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
    }
    let mut writer = encoder.write_header().map_err(png_error)?;
    let mut stream = writer.stream_writer().map_err(png_error)?;

    let modules = &canvas.modules;
    // Three bands per module and axis; with no growth compensation only the middle band has
    // pixels, and it is the whole module.
    let bands = bands(layout.module_px, options.print_growth_dots);
    let mut row = vec![0u8; row_bytes];
    for my in 0..modules.height() {
        for &(y0, y1, repeat) in &bands {
            if repeat == 0 {
                continue;
            }
            // In 1-bit greyscale, 0 is black and 1 is white.
            row.fill(0xFF);
            let mut px = 0usize;
            for mx in 0..modules.width() {
                for &(x0, x1, width) in &bands {
                    let width = usize::try_from(width).map_err(|_| RenderError::TooLarge)?;
                    if width > 0 && eroded(modules, mx, my, (x0, x1), (y0, y1)) {
                        for p in px..px + width {
                            if let Some(byte) = row.get_mut(p / 8) {
                                *byte &= !(0x80u8 >> (p % 8));
                            }
                        }
                    }
                    px += width;
                }
            }
            debug_assert_eq!(px, width_px);
            // Padding bits after the last pixel of a row are ignored by readers; clear them.
            if width_px % 8 != 0
                && let Some(last) = row.last_mut()
            {
                *last &= 0xFFu8 << (8 - width_px % 8);
            }
            for _ in 0..repeat {
                stream.write_all(&row).map_err(png_error)?;
            }
        }
    }
    stream.finish().map_err(png_error)?;
    writer.finish().map_err(png_error)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density() {
        assert_eq!(pixels_per_metre(300), 11_811);
        assert_eq!(pixels_per_metre(600), 23_622);
        assert_eq!(pixels_per_metre(1016), 40_000);
        assert_eq!(pixels_per_metre(96), 3_780);
    }
}
