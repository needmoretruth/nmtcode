//! Shared helpers of the detect tests: test symbols, image transforms and PNG encoders.

// Test code: `as` casts convert small, known image coordinates and sizes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
#![allow(dead_code, clippy::unreadable_literal)]

use nmtcode_core::ModuleGrid;
use nmtcode_detect::LumaImage;

/// Specification annex A, A.2.7: the complete 24 × 24 module matrix, `#` dark.
pub const A27_ROWS: [&str; 24] = [
    "#####.#.#.#..#..#..#####",
    "#..##...##.##...#..##..#",
    "#..##.######.#####.#####",
    "#####..##....##....##..#",
    "#####.##.#..######.#####",
    ".......#####.#####......",
    ".#....##.##.#......##.#.",
    "#.##.#..##..#.###.#..#..",
    "..###.######.##..###..##",
    ".###.###..#..##.##.##...",
    "....##.#.........##..#.#",
    "#...#.###.###.#..#.#.#..",
    "#..######.##....##...##.",
    "..###..#..#...##....#...",
    "#.###.#.#.#...#.#.#.###.",
    "#..#..#..###.#.###.###..",
    ".#....#######.##..#.##.#",
    ".##..###...#...##.....#.",
    "......##.#..##.##.......",
    "#####...#####.#.##.#####",
    "#####.###..###.##..#..##",
    "#####..##.##.#####.#.#.#",
    "#####..##..#####...##..#",
    "#####.###...##.#.#.#####",
];

/// The same matrix as 24-bit hexadecimal rows, x = 0 most significant (A.2.7).
pub const A27_HEX: [u32; 24] = [
    0xFAA49F, 0x98D899, 0x9BF7DF, 0xF98619, 0xFB4FDF, 0x01F7C0, 0x43681A, 0xB4CBA4, 0x3BF673,
    0x7726D8, 0x0D0065, 0x8BBA54, 0x9FB0C6, 0x392308, 0xBAA2AE, 0x9275DC, 0x43FB2D, 0x671182,
    0x034D80, 0xF8FADF, 0xFB9D93, 0xF9B7D5, 0xF99F19, 0xFB8D5F,
];

/// Specification 2.8: the 20 × 20 map with both format copies. `#` dark, `o` light, `-`
/// separator (light), `.` data (unfilled in the specification).
pub const MAP_2_8: [&str; 20] = [
    "#####-#o#o....-#####",
    "#oo##-oo##....-##oo#",
    "#oo##-###o....-#####",
    "#####-###o....-##oo#",
    "#####-##o#....-#####",
    "------ooo.....------",
    "o#o###..............",
    "o#oo##..............",
    "oo##oo..............",
    "o##oo#..............",
    "..............#oo##o",
    "..............oo##oo",
    "..............##oo#o",
    "..............###o#o",
    "------.....ooo------",
    "#####-....#o##-#####",
    "#####-....o###-#oo##",
    "#####-....o###-#o#o#",
    "#####-....##oo-##oo#",
    "#####-....o#o#-#####",
];

pub fn a27() -> ModuleGrid {
    ModuleGrid::from_rows(&A27_ROWS).unwrap()
}

/// The 2.8 map with its data modules filled from `seed`.
pub fn map_2_8(seed: u64) -> ModuleGrid {
    let mut rng = Rng(seed);
    let rows: Vec<String> = MAP_2_8
        .iter()
        .map(|row| {
            row.chars()
                .map(|c| match c {
                    '#' => '#',
                    '.' if rng.bit() => '#',
                    _ => '.',
                })
                .collect()
        })
        .collect();
    ModuleGrid::from_rows(&rows).unwrap()
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

/// The four finders of 5.3.1, upright, TL, TR, BL, BR.
const FINDERS: [[&str; 5]; 4] = [
    ["#####", "#..##", "#..##", "#####", "#####"],
    ["#####", "##..#", "#####", "##..#", "#####"],
    ["#####", "#####", "#####", "#####", "#####"],
    ["#####", "#..##", "#.#.#", "##..#", "#####"],
];

/// Reference-mark lines of 5.6.1 for a side of `z` modules.
pub fn mark_lines(z: u32) -> Vec<u32> {
    let s = z - 5;
    let n = if z < 48 { 1 } else { s.div_ceil(24) };
    (0..=n).map(|i| 2 + (2 * i * s + n) / (2 * n)).collect()
}

/// A `w` × `h` symbol: random data modules from `seed`, then the finders (5.3), separators
/// (5.4) and reference marks (5.6). The format areas hold random bits like data.
pub fn make_symbol(w: u32, h: u32, seed: u64) -> ModuleGrid {
    let mut grid = ModuleGrid::new(w, h).unwrap();
    let mut rng = Rng(seed);
    for y in 0..h {
        for x in 0..w {
            grid.set(x, y, rng.bit());
        }
    }
    let origins = [(0, 0), (w - 5, 0), (0, h - 5), (w - 5, h - 5)];
    for (kind, &(ox, oy)) in origins.iter().enumerate() {
        for (y, row) in FINDERS[kind].iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                grid.set(ox + x as u32, oy + y as u32, c == '#');
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
