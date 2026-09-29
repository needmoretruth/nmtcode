//! Shared helpers of the detect tests: test symbols, image transforms and PNG encoders.

// Test code: `as` casts convert small, known image coordinates and sizes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
#![allow(dead_code, clippy::unreadable_literal)]

use nmtcode_core::{FormatWord, ModuleGrid, SymbolClass};
use nmtcode_detect::LumaImage;
use nmtcode_symbol::{Corner, Layout};

/// A `width` × `height` symbol as the generator draws it (chapter 5): the finders, separators,
/// reference marks and both format copies of a level-0 static format word, and a codeword
/// stream from `seed` on the data modules.
pub fn drawn_symbol(width: u32, height: u32, seed: u64) -> ModuleGrid {
    let layout = Layout::new(width, height).unwrap();
    let word = FormatWord::new(SymbolClass::Static, width, height, 0, 0, 0).unwrap();
    let mut rng = Rng(seed);
    let stream: Vec<u8> = (0..layout.codeword_count()).map(|_| rng.next() as u8).collect();
    layout.draw_copies(word.encode_copies(), &stream).unwrap()
}

/// A 24 × 24 symbol at level 0 with data from a fixed seed.
pub fn symbol_24() -> ModuleGrid {
    drawn_symbol(24, 24, 0x0A27)
}

/// The smallest symbol, 20 × 20, with the format word of 2.8 (level 1) on both copies and data
/// from `seed`.
pub fn symbol_20(seed: u64) -> ModuleGrid {
    let layout = Layout::new(20, 20).unwrap();
    let word = FormatWord::new(SymbolClass::Static, 20, 20, 1, 0, 0).unwrap();
    assert_eq!(word.encode_copies(), [0x51F7_680D_3ACD, 0x3FCE_35FD_8A41]);
    let mut rng = Rng(seed);
    let stream: Vec<u8> = (0..layout.codeword_count()).map(|_| rng.next() as u8).collect();
    layout.draw_copies(word.encode_copies(), &stream).unwrap()
}

/// The `SplitMix64` generator.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn bit(&mut self) -> bool {
        self.next() & 1 == 1
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Reference-mark lines of 5.6.1 for a side of `z` modules.
pub fn mark_lines(z: u32) -> Vec<u32> {
    let s = z - 5;
    let n = if z < 48 { 1 } else { s.div_ceil(24) };
    (0..=n).map(|i| 2 + (2 * i * s + n) / (2 * n)).collect()
}

/// A `w` × `h` symbol as the generator draws it: random codewords from `seed` on the data
/// modules, the finders (5.3), separators (5.4), reference marks (5.6) and both format copies of
/// a level-0 static format word. The detector confirms the format word, so the copies are real.
pub fn make_symbol(w: u32, h: u32, seed: u64) -> ModuleGrid {
    drawn_symbol(w, h, seed)
}

/// A `w` × `h` symbol with random data modules from `seed`, then the finders (5.3), separators
/// (5.4) and reference marks (5.6). The format areas hold random bits like data, so the format
/// word does not decode.
pub fn make_symbol_without_format(w: u32, h: u32, seed: u64) -> ModuleGrid {
    let mut grid = ModuleGrid::new(w, h).unwrap();
    let mut rng = Rng(seed);
    for y in 0..h {
        for x in 0..w {
            grid.set(x, y, rng.bit());
        }
    }
    for corner in Corner::ALL {
        let (ox, oy) = corner.origin(w, h).unwrap();
        for (y, row) in (0..).zip(corner.pattern()) {
            for (x, dark) in (0..).zip(row) {
                grid.set(ox + x, oy + y, dark);
            }
        }
    }
    // Separators (5.4).
    for i in 0..=5 {
        for (x, y) in [
            (5, i),
            (i, 5),
            (w - 6, i),
            (w - 6 + i, 5),
            (5, h - 6 + i),
            (i, h - 6),
            (w - 6, h - 6 + i),
            (w - 6 + i, h - 6),
        ] {
            grid.set(x, y, false);
        }
    }
    // Reference marks (5.6).
    let xs = mark_lines(w);
    let ys = mark_lines(h);
    for &cy in &ys {
        for &cx in &xs {
            let finder_centre = (cx == 2 || cx == w - 3) && (cy == 2 || cy == h - 3);
            if finder_centre {
                continue;
            }
            for dy in 0..3 {
                for dx in 0..3 {
                    grid.set(cx - 1 + dx, cy - 1 + dy, !(dx == 1 && dy == 1));
                }
            }
        }
    }
    grid
}

/// Bilinear resampling by `factor`: output pixel centre (i + 0.5) maps to source coordinate
/// (i + 0.5) / factor, interpolated between the four nearest source pixel centres; edges clamp.
pub fn resample(image: &LumaImage, factor: f64) -> LumaImage {
    let ow = ((f64::from(image.width) * factor).round() as u32).max(1);
    let oh = ((f64::from(image.height) * factor).round() as u32).max(1);
    let get = |x: i64, y: i64| -> f64 {
        let x = x.clamp(0, i64::from(image.width) - 1) as usize;
        let y = y.clamp(0, i64::from(image.height) - 1) as usize;
        f64::from(image.pixels[y * image.width as usize + x])
    };
    let mut pixels = Vec::with_capacity((ow * oh) as usize);
    for oy in 0..oh {
        let sy = (f64::from(oy) + 0.5) / factor - 0.5;
        let y0 = sy.floor();
        let ay = sy - y0;
        for ox in 0..ow {
            let sx = (f64::from(ox) + 0.5) / factor - 0.5;
            let x0 = sx.floor();
            let ax = sx - x0;
            let (xi, yi) = (x0 as i64, y0 as i64);
            let top = get(xi, yi) * (1.0 - ax) + get(xi + 1, yi) * ax;
            let bottom = get(xi, yi + 1) * (1.0 - ax) + get(xi + 1, yi + 1) * ax;
            pixels.push((top * (1.0 - ay) + bottom * ay).round().clamp(0.0, 255.0) as u8);
        }
    }
    LumaImage::new(ow, oh, pixels).unwrap()
}

/// `image` on a larger white canvas with the given margins.
pub fn pad(image: &LumaImage, left: u32, top: u32, right: u32, bottom: u32) -> LumaImage {
    let w = image.width + left + right;
    let h = image.height + top + bottom;
    let mut pixels = vec![255u8; (w * h) as usize];
    for y in 0..image.height {
        for x in 0..image.width {
            pixels[((y + top) * w + x + left) as usize] =
                image.pixels[(y * image.width + x) as usize];
        }
    }
    LumaImage::new(w, h, pixels).unwrap()
}

/// `a` and `b` side by side (`b` to the right, `gap` white pixels between), tops aligned.
pub fn beside(a: &LumaImage, b: &LumaImage, gap: u32) -> LumaImage {
    let w = a.width + gap + b.width;
    let h = a.height.max(b.height);
    let mut pixels = vec![255u8; (w * h) as usize];
    for (img, ox) in [(a, 0), (b, a.width + gap)] {
        for y in 0..img.height {
            for x in 0..img.width {
                pixels[(y * w + x + ox) as usize] = img.pixels[(y * img.width + x) as usize];
            }
        }
    }
    LumaImage::new(w, h, pixels).unwrap()
}

/// Transposes the image (mirror across the main diagonal).
pub fn transpose(image: &LumaImage) -> LumaImage {
    let (w, h) = (image.width, image.height);
    let mut pixels = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            pixels[(x * h + y) as usize] = image.pixels[(y * w + x) as usize];
        }
    }
    LumaImage::new(h, w, pixels).unwrap()
}

/// Mirrors the image left to right.
pub fn mirror(image: &LumaImage) -> LumaImage {
    let (w, h) = (image.width, image.height);
    let mut pixels = image.pixels.clone();
    for y in 0..h {
        pixels[(y * w) as usize..((y + 1) * w) as usize].reverse();
    }
    LumaImage::new(w, h, pixels).unwrap()
}

/// Rotates the image a quarter turn clockwise.
pub fn rotate90(image: &LumaImage) -> LumaImage {
    mirror(&transpose(image))
}

/// Mirrors a module grid left to right.
pub fn mirror_grid(grid: &ModuleGrid) -> ModuleGrid {
    let rows: Vec<String> = grid.to_rows().iter().map(|r| r.chars().rev().collect()).collect();
    ModuleGrid::from_rows(&rows).unwrap()
}

/// Every pixel mapped through `f`.
pub fn map_pixels(image: &LumaImage, f: impl Fn(u8) -> u8) -> LumaImage {
    LumaImage::new(image.width, image.height, image.pixels.iter().map(|&v| f(v)).collect()).unwrap()
}

fn encode(
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
    }
    out
}

pub fn png_gray8(image: &LumaImage) -> Vec<u8> {
    encode(
        image.width,
        image.height,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &image.pixels,
    )
}

pub fn png_gray16(image: &LumaImage) -> Vec<u8> {
    let data: Vec<u8> = image.pixels.iter().flat_map(|&v| [v, v]).collect();
    encode(image.width, image.height, png::ColorType::Grayscale, png::BitDepth::Sixteen, &data)
}

/// RGB with a slight warm tint that keeps the BT.601 luminance within 2 of the grey value.
pub fn png_rgb(image: &LumaImage) -> Vec<u8> {
    let data: Vec<u8> =
        image.pixels.iter().flat_map(|&v| [v.saturating_add(4), v, v.saturating_sub(8)]).collect();
    encode(image.width, image.height, png::ColorType::Rgb, png::BitDepth::Eight, &data)
}

/// RGBA: opaque where `opaque` says so; transparent pixels carry black colour, which a reader
/// must composite over white.
pub fn png_rgba(image: &LumaImage, opaque: impl Fn(u32, u32) -> bool) -> Vec<u8> {
    let mut data = Vec::with_capacity(image.pixels.len() * 4);
    for y in 0..image.height {
        for x in 0..image.width {
            let v = image.pixels[(y * image.width + x) as usize];
            if opaque(x, y) {
                data.extend_from_slice(&[v, v, v, 255]);
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    encode(image.width, image.height, png::ColorType::Rgba, png::BitDepth::Eight, &data)
}

pub fn png_gray_alpha(image: &LumaImage) -> Vec<u8> {
    let data: Vec<u8> = image.pixels.iter().flat_map(|&v| [v, 255]).collect();
    encode(image.width, image.height, png::ColorType::GrayscaleAlpha, png::BitDepth::Eight, &data)
}

/// Two-colour palette PNG, 1 bit per index; any pixel below 128 is index 0 (black).
pub fn png_palette(image: &LumaImage) -> Vec<u8> {
    let row_bytes = image.width.div_ceil(8) as usize;
    let mut data = vec![0u8; row_bytes * image.height as usize];
    for y in 0..image.height as usize {
        for x in 0..image.width as usize {
            if image.pixels[y * image.width as usize + x] >= 128 {
                data[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::One);
        encoder.set_palette(vec![0u8, 0, 0, 255, 255, 255]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
    }
    out
}
