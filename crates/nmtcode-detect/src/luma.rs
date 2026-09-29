//! 8-bit luminance images.

use crate::MAX_PIXELS;
use crate::num::floor_i32;

/// An 8-bit luminance image: `width × height` pixels, row by row from the top-left, 0 = black,
/// 255 = white.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LumaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height` luminance values, row-major.
    pub pixels: Vec<u8>,
}

/// BT.601 luma Y′ = 0.299 R′ + 0.587 G′ + 0.114 B′ of 8-bit sRGB-encoded values, rounded
/// (specification 1.4).
pub(crate) fn luma(r: u8, g: u8, b: u8) -> u8 {
    // 299 + 587 + 114 = 1000, so the weighted sum over 1000 is at most 255.
    let y = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) + 500) / 1000;
    u8::try_from(y).unwrap_or(u8::MAX)
}

/// `value` with opacity `alpha`, composited over white.
pub(crate) fn over_white(value: u8, alpha: u8) -> u8 {
    let a = u32::from(alpha);
    let v = (u32::from(value) * a + 255 * (255 - a) + 127) / 255;
    u8::try_from(v).unwrap_or(u8::MAX)
}

impl LumaImage {
    /// An image from its pixels. `None` when a side is 0, the image has more than
    /// [`MAX_PIXELS`] pixels, or `pixels` does not hold exactly `width × height` values.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let image = Self { width, height, pixels };
        image.is_usable().then_some(image)
    }

    /// The luma image of an RGBA buffer: `width × height` pixels of 4 bytes (red, green, blue,
    /// alpha), row by row from the top-left, as a canvas's `ImageData` or a decoded image holds
    /// them. Each pixel becomes the BT.601 luma of its sRGB-encoded values (specification 1.4),
    /// Y′ = 0.299 R′ + 0.587 G′ + 0.114 B′, rounded, and is then composited over white with its
    /// alpha.
    ///
    /// `None` when a side is 0, the image has more than [`MAX_PIXELS`] pixels, or `rgba` does
    /// not hold exactly `width × height × 4` bytes. The size is checked before anything is
    /// allocated.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let count = u64::from(width).checked_mul(u64::from(height))?;
        if width == 0 || height == 0 || count > MAX_PIXELS {
            return None;
        }
        let expected = usize::try_from(count.checked_mul(4)?).ok()?;
        if rgba.len() != expected {
            return None;
        }
        let pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| over_white(luma(r, g, b), a))
            .collect();
        Self::new(width, height, pixels)
    }

    /// The pixel at (`x`, `y`), `None` outside the image.
    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        self.pixels.get(index).copied()
    }

    /// True when the sides are non-zero, the pixel count is at most [`MAX_PIXELS`] and the
    /// buffer length matches. Every other method of this crate relies on it.
    pub(crate) fn is_usable(&self) -> bool {
        let count = u64::from(self.width) * u64::from(self.height);
        self.width > 0
            && self.height > 0
            && count <= MAX_PIXELS
            && usize::try_from(count).is_ok_and(|c| c == self.pixels.len())
    }

    /// The inverted image (specification 1.4 and 5.11: a reader MAY read the inverted image):
    /// light and dark swap in linear light, so that a blurred edge keeps its midpoint (see
    /// `light`). The map is decreasing, so luma order is reversed and nothing else changes.
    pub(crate) fn inverted(&self) -> Self {
        let mut table = [0u8; 256];
        for (v, slot) in (0u32..).zip(table.iter_mut()) {
            *slot = crate::light::stored_u8(255.0 - crate::light::linear(f64::from(v)));
        }
        Self {
            width: self.width,
            height: self.height,
            pixels: self
                .pixels
                .iter()
                .map(|&v| table.get(usize::from(v)).copied().unwrap_or(0))
                .collect(),
        }
    }

    /// Width as `usize`.
    pub(crate) fn w_usize(&self) -> usize {
        usize::try_from(self.width).unwrap_or(0)
    }

    /// Height as `usize`.
    pub(crate) fn h_usize(&self) -> usize {
        usize::try_from(self.height).unwrap_or(0)
    }

    /// Width as `i32` (at most 10^8 on a usable image).
    pub(crate) fn w(&self) -> i32 {
        i32::try_from(self.width).unwrap_or(i32::MAX)
    }

    /// Height as `i32` (at most 10^8 on a usable image).
    pub(crate) fn h(&self) -> i32 {
        i32::try_from(self.height).unwrap_or(i32::MAX)
    }

    /// The pixel at (`x`, `y`); outside the image the background is white (255).
    pub(crate) fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w() || y >= self.h() {
            return 255;
        }
        // Both coordinates are non-negative and inside the image here.
        let (Ok(xu), Ok(yu)) = (usize::try_from(x), usize::try_from(y)) else {
            return 255;
        };
        self.pixels.get(yu * self.w_usize() + xu).copied().unwrap_or(255)
    }

    /// Bilinear interpolation at the continuous point (`x`, `y`); pixel (i, j) covers
    /// [i, i + 1) × [j, j + 1) and its value sits at its centre.
    pub(crate) fn bilinear(&self, x: f64, y: f64) -> f64 {
        let fx = x - 0.5;
        let fy = y - 0.5;
        let x0 = floor_i32(fx);
        let y0 = floor_i32(fy);
        let ax = (fx - f64::from(x0)).clamp(0.0, 1.0);
        let ay = (fy - f64::from(y0)).clamp(0.0, 1.0);
        let x1 = x0.saturating_add(1);
        let y1 = y0.saturating_add(1);
        // Fast path: all four pixels inside the image.
        if x0 >= 0 && y0 >= 0 && x1 < self.w() && y1 < self.h() {
            let w = self.w_usize();
            let (Ok(xi), Ok(yi)) = (usize::try_from(x0), usize::try_from(y0)) else {
                return 255.0;
            };
            let i = yi * w + xi;
            let p = |k: usize| f64::from(self.pixels.get(k).copied().unwrap_or(255));
            let top = p(i) * (1.0 - ax) + p(i + 1) * ax;
            let bottom = p(i + w) * (1.0 - ax) + p(i + w + 1) * ax;
            return top * (1.0 - ay) + bottom * ay;
        }
        let p = |xx: i32, yy: i32| f64::from(self.at(xx, yy));
        let top = p(x0, y0) * (1.0 - ax) + p(x1, y0) * ax;
        let bottom = p(x0, y1) * (1.0 - ax) + p(x1, y1) * ax;
        top * (1.0 - ay) + bottom * ay
    }

    /// [`LumaImage::bilinear`] at a point.
    pub(crate) fn sample(&self, p: crate::geom::Point) -> f64 {
        self.bilinear(p.x, p.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_checks_the_buffer() {
        assert!(LumaImage::new(2, 2, vec![0; 4]).is_some());
        assert!(LumaImage::new(2, 2, vec![0; 5]).is_none());
        assert!(LumaImage::new(0, 2, vec![]).is_none());
        assert!(LumaImage::new(100_000, 100_000, vec![]).is_none());
    }

    #[test]
    fn sampling() {
        let image = LumaImage::new(2, 1, vec![0, 200]).unwrap();
        assert_eq!(image.at(-1, 0), 255);
        assert_eq!(image.at(1, 0), 200);
        assert_eq!(image.get(2, 0), None);
        assert!((image.bilinear(0.5, 0.5) - 0.0).abs() < 1e-9);
        assert!((image.bilinear(1.0, 0.5) - 100.0).abs() < 1e-9);
        assert!((image.bilinear(1.5, 0.5) - 200.0).abs() < 1e-9);
        let big = LumaImage::new(3, 3, vec![0, 10, 20, 30, 40, 50, 60, 70, 80]).unwrap();
        assert!((big.bilinear(1.0, 1.0) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn rgba() {
        let image = LumaImage::from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
        assert_eq!(image.pixels, vec![76, 255]);
        assert!(LumaImage::from_rgba(2, 1, &[0; 7]).is_none());
        assert!(LumaImage::from_rgba(0, 1, &[]).is_none());
        assert!(LumaImage::from_rgba(100_000, 100_000, &[]).is_none());
        assert_eq!(luma(255, 255, 255), 255);
        assert_eq!(over_white(100, 128), 177);
    }
}
