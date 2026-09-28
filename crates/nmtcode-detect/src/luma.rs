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

impl LumaImage {
    /// An image from its pixels. `None` when a side is 0, the image has more than
    /// [`MAX_PIXELS`] pixels, or `pixels` does not hold exactly `width × height` values.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let image = Self { width, height, pixels };
        image.is_usable().then_some(image)
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
        let (Ok(xu), Ok(yu), Ok(wu)) =
            (usize::try_from(x), usize::try_from(y), usize::try_from(self.width))
        else {
            return 255;
        };
        self.pixels.get(yu * wu + xu).copied().unwrap_or(255)
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
        let p = |xx: i32, yy: i32| f64::from(self.at(xx, yy));
        let x1 = x0.saturating_add(1);
        let y1 = y0.saturating_add(1);
        let top = p(x0, y0) * (1.0 - ax) + p(x1, y0) * ax;
        let bottom = p(x0, y1) * (1.0 - ax) + p(x1, y1) * ax;
        top * (1.0 - ay) + bottom * ay
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
    }
}
