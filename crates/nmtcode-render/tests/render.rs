//! Output tests: placement of 8.8.2, PNG and SVG content, the bootstrap QR codewords of 8.8.1.

// Test code: `as` casts convert small, known image coordinates and sizes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use std::fmt::Write as _;
use std::io::Cursor;

use nmtcode_core::ModuleGrid;
use nmtcode_render::{
    BOOTSTRAP_URL, BootstrapLevel, BootstrapSide, Canvas, Layout, PRINT_MIN_DOTS, RenderError,
    RenderOptions, bootstrap_qr, layout, print_module_px, render_modules, render_png, render_svg,
};
use proptest::prelude::*;

/// Specification annex A, A.2.7.
const A27_ROWS: [&str; 24] = [
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

fn a27() -> ModuleGrid {
    ModuleGrid::from_rows(&A27_ROWS).unwrap()
}

/// A grid of the given size with pseudo-random modules.
fn random_grid(w: u32, h: u32, seed: u64) -> ModuleGrid {
    let mut grid = ModuleGrid::new(w, h).unwrap();
    let mut state = seed | 1;
    for y in 0..h {
        for x in 0..w {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            grid.set(x, y, state & 1 == 1);
        }
    }
    grid
}

fn opts(module_px: u32, quiet_zone: u32, bootstrap: bool) -> RenderOptions {
    RenderOptions { module_px, quiet_zone, bootstrap, ..RenderOptions::default() }
}

// ---------------------------------------------------------------------------------------------
// Placement (8.4, 8.8.2)

#[test]
fn placement_8_8_2_screen() {
    let l = layout(64, 64, &opts(4, 2, true)).unwrap();
    let b = l.bootstrap.unwrap();
    assert_eq!((b.side, b.version, b.scale, b.gap, b.size), (BootstrapSide::Left, 3, 1, 4, 29));
    // QR occupies −33 ≤ x < −4, 0 ≤ y < 29.
    assert_eq!((b.x, b.x + i64::from(b.size), b.y, b.y + i64::from(b.size)), (-33, -4, 0, 29));
    // Canvas x −37 … 66, y −4 … 66: 103 × 70, 412 × 280 px.
    assert_eq!((l.canvas_x, l.canvas_y, l.canvas_width, l.canvas_height), (-37, -4, 103, 70));
    assert_eq!((l.pixel_width(), l.pixel_height()), (412, 280));
}

#[test]
fn placement_8_8_2_tall() {
    let l = layout(48, 96, &opts(4, 2, true)).unwrap();
    let b = l.bootstrap.unwrap();
    assert_eq!((b.side, b.scale, b.gap, b.size), (BootstrapSide::Above, 1, 4, 29));
    assert_eq!((b.x, b.x + i64::from(b.size), b.y, b.y + i64::from(b.size)), (0, 29, -33, -4));
    assert_eq!((l.canvas_x, l.canvas_y, l.canvas_width, l.canvas_height), (-4, -37, 54, 135));
}

#[test]
fn placement_8_8_2_print() {
    // 0.25 mm modules: 10 dots at 1016 dpi.
    let options = RenderOptions { module_px: 10, dpi: Some(1016), ..RenderOptions::default() };
    let l = layout(80, 80, &options).unwrap();
    let b = l.bootstrap.unwrap();
    assert_eq!((b.side, b.scale, b.gap, b.size), (BootstrapSide::Left, 2, 8, 58));
    assert_eq!((b.x, b.x + i64::from(b.size), b.y, b.y + i64::from(b.size)), (-66, -8, 0, 58));
    assert_eq!((l.canvas_x, l.canvas_y, l.canvas_width, l.canvas_height), (-74, -8, 156, 90));
    // 39.0 × 22.5 mm.
    let svg = render_svg(&random_grid(80, 80, 1), &options).unwrap();
    assert!(svg.contains(r#"width="39mm" height="22.5mm""#), "{}", &svg[..300]);
    // QR module 0.5 mm, QR 14.5 mm wide, gap 2 mm, with modules of 250 µm.
    assert_eq!(b.scale * 250, 500);
    assert_eq!(b.size * 250, 14_500);
    assert_eq!(b.gap * 250, 2_000);
}

#[test]
fn default_side_follows_the_shorter_side() {
    let side = |w, h| layout(w, h, &RenderOptions::default()).unwrap().bootstrap.unwrap().side;
    assert_eq!(side(20, 20), BootstrapSide::Left);
    assert_eq!(side(100, 20), BootstrapSide::Left);
    assert_eq!(side(20, 24), BootstrapSide::Above);
    let forced =
        RenderOptions { bootstrap_side: Some(BootstrapSide::Above), ..RenderOptions::default() };
    assert_eq!(layout(64, 64, &forced).unwrap().bootstrap.unwrap().side, BootstrapSide::Above);
    let forced =
        RenderOptions { bootstrap_side: Some(BootstrapSide::Left), ..RenderOptions::default() };
    assert_eq!(layout(20, 64, &forced).unwrap().bootstrap.unwrap().side, BootstrapSide::Left);
}

#[test]
fn gap_of_8_4_2() {
    let gap = |module_px, quiet_zone| {
        layout(64, 64, &opts(module_px, quiet_zone, true)).unwrap().bootstrap.unwrap().gap
    };
    assert_eq!(gap(4, 2), 4); // n = 1, Q = 2
    assert_eq!(gap(4, 6), 6); // n = 1, Q = 6
    assert_eq!(gap(2, 5), 8); // n = 2: 2 · max(4, 3)
    assert_eq!(gap(2, 9), 10); // n = 2: 2 · max(4, 5)
    assert_eq!(gap(1, 2), 16); // n = 4: 4 · max(4, 1)
}

#[test]
fn scale_of_8_4_1() {
    let scale = |options: &RenderOptions| layout(64, 64, options).unwrap().bootstrap.unwrap().scale;
    assert_eq!(scale(&opts(4, 2, true)), 1);
    assert_eq!(scale(&opts(3, 2, true)), 2);
    assert_eq!(scale(&opts(1, 2, true)), 4);
    let print = RenderOptions::print(600).unwrap();
    assert_eq!(scale(&print), 1);
    let bigger = RenderOptions { bootstrap_scale: Some(3), ..RenderOptions::default() };
    assert_eq!(scale(&bigger), 3);
    let smaller =
        RenderOptions { module_px: 2, bootstrap_scale: Some(1), ..RenderOptions::default() };
    assert_eq!(
        layout(64, 64, &smaller),
        Err(RenderError::BootstrapScaleTooSmall { requested: 1, minimum: 2 })
    );
}

#[test]
fn levels_q_and_h() {
    for (level, version, side) in
        [(BootstrapLevel::M, 3, 29), (BootstrapLevel::Q, 4, 33), (BootstrapLevel::H, 5, 37)]
    {
        let options = RenderOptions { bootstrap_level: level, ..RenderOptions::default() };
        let b = layout(64, 64, &options).unwrap().bootstrap.unwrap();
        assert_eq!((b.version, b.size), (version, side));
        assert_eq!(bootstrap_qr(level).unwrap().width(), side);
    }
}

#[test]
fn bootstrap_off() {
    let l = layout(24, 24, &opts(3, 5, false)).unwrap();
    assert_eq!(l.bootstrap, None);
    assert_eq!((l.canvas_x, l.canvas_y, l.canvas_width, l.canvas_height), (-5, -5, 34, 34));
}

// ---------------------------------------------------------------------------------------------
// Canvas content

fn in_rect(x: i64, y: i64, x0: i64, y0: i64, w: i64, h: i64) -> bool {
    x >= x0 && y >= y0 && x < x0 + w && y < y0 + h
}

/// Every canvas module is the symbol's, the QR's (scaled by n) or light; the QR is outside the
/// symbol's quiet zone and has its own 4-module quiet zone inside the canvas.
fn check_canvas(grid: &ModuleGrid, canvas: &Canvas) {
    let l: &Layout = &canvas.layout;
    let (w, h) = (i64::from(grid.width()), i64::from(grid.height()));
    let q = i64::from(l.quiet_zone);
    let qr = l.bootstrap.map(|b| (b, bootstrap_qr(b.level).unwrap()));
    for cy in 0..l.canvas_height {
        for cx in 0..l.canvas_width {
            let x = i64::from(cx) + l.canvas_x;
            let y = i64::from(cy) + l.canvas_y;
            let got = canvas.modules.get(cx, cy).unwrap();
            let expected = if in_rect(x, y, 0, 0, w, h) {
                grid.get(x as u32, y as u32).unwrap()
            } else if let Some((b, m)) = &qr {
                let s = i64::from(b.size);
                if in_rect(x, y, b.x, b.y, s, s) {
                    let n = i64::from(b.scale);
                    m.get(((x - b.x) / n) as u32, ((y - b.y) / n) as u32).unwrap()
                } else {
                    false
                }
            } else {
                false
            };
            assert_eq!(got, expected, "canvas ({cx}, {cy}) = symbol ({x}, {y})");
        }
    }
    if let Some((b, _)) = qr {
        let (n, s) = (i64::from(b.scale), i64::from(b.size));
        // The QR (with its 4n quiet zone) lies inside the canvas.
        assert!(b.x - 4 * n >= l.canvas_x && b.y - 4 * n >= l.canvas_y);
        assert!(b.x + s + 4 * n <= l.canvas_x + i64::from(l.canvas_width));
        assert!(b.y + s + 4 * n <= l.canvas_y + i64::from(l.canvas_height));
        // The QR is outside the NMT Code quiet zone, and outside its own quiet zone of the symbol.
        let apart_x = b.x + s + q.max(4 * n) <= 0 || b.x >= w + q.max(4 * n);
        let apart_y = b.y + s + q.max(4 * n) <= 0 || b.y >= h + q.max(4 * n);
        assert!(apart_x || apart_y, "QR at ({}, {}) too close", b.x, b.y);
        assert!(i64::from(b.gap) >= q && i64::from(b.gap) >= 4 * n);
    }
    // The NMT Code quiet zone is inside the canvas.
    assert!(l.canvas_x <= -q && l.canvas_y <= -q);
    assert!(l.canvas_x + i64::from(l.canvas_width) >= w + q);
    assert!(l.canvas_y + i64::from(l.canvas_height) >= h + q);
}

#[test]
fn canvas_holds_symbol_qr_and_nothing_else() {
    for (grid, options) in [
        (a27(), opts(4, 2, true)),
        (a27(), opts(2, 3, true)),
        (a27(), opts(1, 2, true)),
        (random_grid(64, 64, 3), opts(4, 2, true)),
        (random_grid(48, 96, 4), opts(4, 2, true)),
        (
            random_grid(80, 80, 5),
            RenderOptions { module_px: 10, dpi: Some(1016), ..RenderOptions::default() },
        ),
        (
            random_grid(20, 20, 6),
            RenderOptions {
                bootstrap_level: BootstrapLevel::H,
                quiet_zone: 9,
                ..RenderOptions::default()
            },
        ),
        (random_grid(20, 20, 7), opts(5, 2, false)),
    ] {
        check_canvas(&grid, &render_modules(&grid, &options).unwrap());
    }
}

// ---------------------------------------------------------------------------------------------
// The bootstrap QR as drawn: read back its format and codewords (ISO/IEC 18004 structure)

fn qr_function_v3(x: usize, y: usize, size: usize) -> bool {
    let finder = (y < 9 && (x < 9 || x >= size - 8)) || (x < 9 && y >= size - 8);
    let timing = x == 6 || y == 6;
    let alignment = (20..=24).contains(&x) && (20..=24).contains(&y);
    finder || timing || alignment
}

fn qr_mask(mask: u32, x: usize, y: usize) -> bool {
    match mask {
        0 => (x + y).is_multiple_of(2),
        1 => y.is_multiple_of(2),
        2 => x.is_multiple_of(3),
        3 => (x + y).is_multiple_of(3),
        4 => (x / 3 + y / 2).is_multiple_of(2),
        5 => x * y % 2 + x * y % 3 == 0,
        6 => (x * y % 2 + x * y % 3).is_multiple_of(2),
        _ => ((x + y) % 2 + x * y % 3).is_multiple_of(2),
    }
}

fn format_bits(data: u32) -> u32 {
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    ((data << 10) | (rem & 0x3FF)) ^ 0x5412
}

/// Reads (level bits, mask) from the first format copy, checking its BCH code.
fn read_format(qr: &ModuleGrid) -> (u32, u32) {
    let bit = |x: u32, y: u32| u32::from(qr.get(x, y).unwrap());
    let mut bits = 0u32;
    for i in 0..=5 {
        bits |= bit(8, i) << i;
    }
    bits |= bit(8, 7) << 6;
    bits |= bit(8, 8) << 7;
    bits |= bit(7, 8) << 8;
    for i in 9..15 {
        bits |= bit(14 - i, 8) << i;
    }
    let data = (bits ^ 0x5412) >> 10;
    assert_eq!(format_bits(data), bits, "format BCH");
    (data >> 3, data & 7)
}

/// The 70 codewords of a version 3 symbol, unmasked, in placement order.
fn read_codewords_v3(qr: &ModuleGrid) -> Vec<u8> {
    let size = qr.width() as usize;
    assert_eq!(size, 29);
    let (_, mask) = read_format(qr);
    let mut bits = Vec::new();
    let mut right = size as i64 - 1;
    while right >= 1 {
        if right == 6 {
            right = 5;
        }
        for vert in 0..size {
            for j in 0..2 {
                let x = (right - j) as usize;
                let upward = (right + 1) & 2 == 0;
                let y = if upward { size - 1 - vert } else { vert };
                if !qr_function_v3(x, y, size) {
                    let module = qr.get(x as u32, y as u32).unwrap();
                    bits.push(module ^ qr_mask(mask, x, y));
                }
            }
        }
        right -= 2;
    }
    // 567 data modules: 70 codewords and 7 remainder bits.
    assert_eq!(bits.len(), 567);
    bits.chunks(8).take(70).map(|c| c.iter().fold(0u8, |a, &b| a << 1 | u8::from(b))).collect()
}

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|p| u8::from_str_radix(p, 16).unwrap()).collect()
}

/// The bootstrap QR cut out of a drawn canvas, one module per QR module.
fn qr_from_canvas(canvas: &Canvas) -> ModuleGrid {
    let l = &canvas.layout;
    let b = l.bootstrap.unwrap();
    let side = b.size / b.scale;
    let mut qr = ModuleGrid::new(side, side).unwrap();
    for y in 0..side {
        for x in 0..side {
            let cx = (b.x - l.canvas_x) as u32 + x * b.scale + b.scale / 2;
            let cy = (b.y - l.canvas_y) as u32 + y * b.scale + b.scale / 2;
            qr.set(x, y, canvas.modules.get(cx, cy).unwrap());
        }
    }
    qr
}

#[test]
fn drawn_bootstrap_carries_the_codewords_of_8_8_1() {
    let data = hex("42 86 87 47 47 07 33 A2 F2 F6 76 97 46 87 56 22 E6 36 F6 D2 F6 E6 \
         56 56 46 D6 F7 26 57 47 27 57 46 82 F6 E6 D7 46 36 F6 46 50 EC 11");
    let parity =
        hex("3D 7E 96 5A A8 1D 01 67 EA 68 9C D9 12 4C D4 F3 33 BC E8 1A DD 81 C4 14 2B 16");
    let expected: Vec<u8> = data.iter().chain(parity.iter()).copied().collect();
    for options in [
        opts(4, 2, true),
        opts(2, 2, true),
        RenderOptions { module_px: 10, dpi: Some(1016), ..RenderOptions::default() },
    ] {
        let canvas = render_modules(&a27(), &options).unwrap();
        let qr = qr_from_canvas(&canvas);
        assert_eq!(read_format(&qr).0, 0, "level M");
        assert_eq!(read_codewords_v3(&qr), expected);
    }
    // The data codewords spell the URL: 0100, count 40, then the bytes.
    let mut bits = String::new();
    for b in &data {
        write!(bits, "{b:08b}").unwrap();
    }
    let value: Vec<u8> =
        (0..40).map(|i| u8::from_str_radix(&bits[12 + 8 * i..20 + 8 * i], 2).unwrap()).collect();
    assert_eq!(value, BOOTSTRAP_URL.as_bytes());
}

#[test]
fn levels_q_and_h_have_their_format_bits() {
    // Level bits of ISO/IEC 18004: M = 00, Q = 11, H = 10.
    for (level, bits) in [(BootstrapLevel::M, 0), (BootstrapLevel::Q, 3), (BootstrapLevel::H, 2)] {
        assert_eq!(read_format(&bootstrap_qr(level).unwrap()).0, bits);
    }
}

// ---------------------------------------------------------------------------------------------
// PNG

struct Decoded {
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    dims: Option<png::PixelDimensions>,
    /// One value per pixel: true = black.
    black: Vec<bool>,
}

fn decode(bytes: &[u8]) -> Decoded {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let info = reader.info().clone();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut buf).unwrap();
    let mut black = Vec::new();
    for row in buf.chunks(frame.line_size).take(frame.height as usize) {
        for x in 0..frame.width as usize {
            black.push(row[x / 8] & (0x80 >> (x % 8)) == 0);
        }
    }
    Decoded {
        width: info.width,
        height: info.height,
        color: info.color_type,
        depth: info.bit_depth,
        dims: info.pixel_dims,
        black,
    }
}

#[test]
fn png_is_1_bit_grey_and_matches_the_canvas() {
    for (grid, options) in [
        (a27(), RenderOptions::default()),
        (a27(), opts(1, 2, true)),
        (a27(), opts(3, 3, false)),
        (random_grid(48, 96, 8), opts(5, 2, true)),
        (random_grid(20, 20, 9), opts(7, 4, true)),
    ] {
        let canvas = render_modules(&grid, &options).unwrap();
        let png = decode(&render_png(&grid, &options).unwrap());
        assert_eq!((png.color, png.depth), (png::ColorType::Grayscale, png::BitDepth::One));
        assert_eq!(u64::from(png.width), canvas.layout.pixel_width());
        assert_eq!(u64::from(png.height), canvas.layout.pixel_height());
        assert!(png.dims.is_none());
        let s = options.module_px;
        for y in 0..png.height {
            for x in 0..png.width {
                let expected = canvas.modules.get(x / s, y / s).unwrap();
                assert_eq!(png.black[(y * png.width + x) as usize], expected, "pixel ({x}, {y})");
            }
        }
    }
}

#[test]
fn png_carries_the_dpi() {
    let options = RenderOptions::print(300).unwrap();
    assert_eq!(options.module_px, 5);
    let png = decode(&render_png(&a27(), &options).unwrap());
    let dims = png.dims.unwrap();
    assert_eq!((dims.xppu, dims.yppu, dims.unit), (11_811, 11_811, png::Unit::Meter));
}

// ---------------------------------------------------------------------------------------------
// SVG

/// A minimal XML well-formedness check for the SVG this crate writes: one prolog, balanced
/// tags, quoted and unique attributes, no stray `<` or `&` in text. Returns the elements with
/// their attributes, in document order.
fn parse_xml(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let rest = text.strip_prefix(r#"<?xml version="1.0" encoding="UTF-8"?>"#).expect("prolog");
    let mut stack: Vec<String> = Vec::new();
    let mut elements = Vec::new();
    let mut roots = 0;
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'<' {
            let end = rest[i..].find('<').map_or(rest.len(), |p| i + p);
            let chunk = &rest[i..end];
            assert!(!chunk.contains('&') && !chunk.contains('>'), "text {chunk:?}");
            if stack.is_empty() {
                assert!(chunk.trim().is_empty(), "text outside the root: {chunk:?}");
            }
            i = end;
            continue;
        }
        let end = i + rest[i..].find('>').expect("unclosed tag");
        let tag = &rest[i + 1..end];
        i = end + 1;
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name), "closing {name}");
            continue;
        }
        let self_closing = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let (name, mut attrs_text) = tag.split_once(' ').unwrap_or((tag, ""));
        assert!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()),
            "name {name:?}"
        );
        let mut attrs: Vec<(String, String)> = Vec::new();
        loop {
            attrs_text = attrs_text.trim_start();
            if attrs_text.is_empty() {
                break;
            }
            let (key, after) = attrs_text.split_once("=\"").expect("attribute");
            let (value, after) = after.split_once('"').expect("closing quote");
            assert!(
                key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':'),
                "{key}"
            );
            assert!(!value.contains('<') && !value.contains('&'), "{value}");
            assert!(attrs.iter().all(|(k, _)| k != key), "duplicate {key}");
            attrs.push((key.to_owned(), value.to_owned()));
            attrs_text = after;
        }
        if stack.is_empty() {
            roots += 1;
        }
        elements.push((name.to_owned(), attrs));
        if !self_closing {
            stack.push(name.to_owned());
        }
    }
    assert!(stack.is_empty(), "unclosed {stack:?}");
    assert_eq!(roots, 1);
    elements
}

fn attr<'a>(element: &'a (String, Vec<(String, String)>), key: &str) -> Option<&'a str> {
    element.1.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Rasterises the path's `M x y h w v h h -w z` rectangles; panics on overlap.
fn rasterise(d: &str, width: u32, height: u32) -> (ModuleGrid, usize) {
    let mut grid = ModuleGrid::new(width, height).unwrap();
    let mut count = 0;
    for rect in d.split('z').filter(|r| !r.is_empty()) {
        let rect = rect.strip_prefix('M').expect("M");
        let (xy, rest) = rect.split_once('h').expect("h");
        let (x, y) = xy.split_once(' ').expect("x y");
        let (w, rest) = rest.split_once('v').expect("v");
        let (h, back) = rest.split_once('h').expect("h back");
        assert_eq!(back, format!("-{w}"));
        let (x, y, w, h): (u32, u32, u32, u32) =
            (x.parse().unwrap(), y.parse().unwrap(), w.parse().unwrap(), h.parse().unwrap());
        assert!(w > 0 && h > 0);
        for yy in y..y + h {
            for xx in x..x + w {
                assert_eq!(grid.get(xx, yy), Some(false), "overlap or outside at ({xx}, {yy})");
                grid.set(xx, yy, true);
            }
        }
        count += 1;
    }
    (grid, count)
}

fn check_svg(grid: &ModuleGrid, options: &RenderOptions) {
    let canvas = render_modules(grid, options).unwrap();
    let svg = render_svg(grid, options).unwrap();
    let elements = parse_xml(&svg);
    let names: Vec<&str> = elements.iter().map(|e| e.0.as_str()).collect();
    assert_eq!(names, ["svg", "title", "rect", "path"]);
    let root = &elements[0];
    let (cw, ch) = (canvas.layout.canvas_width, canvas.layout.canvas_height);
    assert_eq!(attr(root, "xmlns"), Some("http://www.w3.org/2000/svg"));
    assert_eq!(attr(root, "viewBox"), Some(format!("0 0 {cw} {ch}").as_str()));
    assert_eq!(attr(root, "shape-rendering"), Some("crispEdges"));
    if options.dpi.is_none() {
        assert_eq!(attr(root, "width"), Some(canvas.layout.pixel_width().to_string().as_str()));
        assert_eq!(attr(root, "height"), Some(canvas.layout.pixel_height().to_string().as_str()));
    } else {
        assert!(attr(root, "width").unwrap().ends_with("mm"));
    }
    assert!(!svg.contains("script") && !svg.contains(" on"), "no scripts or handlers");
    let background = &elements[2];
    assert_eq!(attr(background, "fill"), Some("#ffffff"));
    assert_eq!(attr(background, "width"), Some(cw.to_string().as_str()));
    let path = &elements[3];
    assert_eq!(attr(path, "fill"), Some("#000000"));
    let (raster, rects) = rasterise(attr(path, "d").unwrap(), cw, ch);
    assert_eq!(raster, canvas.modules, "SVG coverage");
    // Compact: no more rectangles than horizontal runs of dark modules, and fewer than modules.
    let mut runs = 0usize;
    for y in 0..ch {
        for x in 0..cw {
            let dark = canvas.modules.get(x, y) == Some(true);
            let prev_dark = x > 0 && canvas.modules.get(x - 1, y) == Some(true);
            if dark && !prev_dark {
                runs += 1;
            }
        }
    }
    let dark = usize::try_from(canvas.modules.dark_count()).unwrap();
    assert!(rects <= runs && runs < dark, "{rects} rectangles, {runs} runs, {dark} dark modules");
}

#[test]
fn svg_is_well_formed_and_covers_exactly_the_dark_modules() {
    check_svg(&a27(), &RenderOptions::default());
    check_svg(&a27(), &opts(1, 2, false));
    check_svg(&random_grid(64, 64, 11), &opts(4, 3, true));
    check_svg(&random_grid(48, 96, 12), &opts(2, 2, true));
    check_svg(
        &random_grid(80, 80, 13),
        &RenderOptions { module_px: 10, dpi: Some(1016), ..RenderOptions::default() },
    );
    let svg = render_svg(&a27(), &RenderOptions::default()).unwrap();
    assert!(svg.contains("<title>NMT Code, 24 × 24 modules</title>"));
    assert!(!svg.contains("QR"), "no trademark word in the output");
}

#[test]
fn all_light_grid_has_no_path() {
    let grid = ModuleGrid::new(20, 20).unwrap();
    let svg = render_svg(&grid, &opts(4, 2, false)).unwrap();
    let names: Vec<String> = parse_xml(&svg).into_iter().map(|e| e.0).collect();
    assert_eq!(names, ["svg", "title", "rect"]);
}

// ---------------------------------------------------------------------------------------------
// Print helper and errors

#[test]
fn print_module_sizes() {
    assert_eq!(print_module_px(600, 0.4), Ok(10));
    assert_eq!(print_module_px(300, 0.4), Ok(5));
    assert_eq!(print_module_px(1200, 0.4), Ok(19));
    assert_eq!(print_module_px(2400, 0.4), Ok(38));
    assert_eq!(print_module_px(96, 0.4), Ok(PRINT_MIN_DOTS));
    assert_eq!(print_module_px(254, 0.4), Ok(4));
    assert_eq!(print_module_px(600, 0.5), Ok(12));
    // 0.4 mm exactly at 1016 dpi (16 dots = 0.4 mm): no rounding up.
    assert_eq!(print_module_px(1016, 0.4), Ok(16));
    for (dpi, px) in [(300, 5), (600, 10), (1200, 19), (720, 12)] {
        let mm = f64::from(px) * 25.4 / f64::from(dpi);
        assert!(mm >= 0.4 && f64::from(px - 1) * 25.4 / f64::from(dpi) < 0.4, "{dpi} dpi");
    }
    assert_eq!(print_module_px(0, 0.4), Err(RenderError::InvalidDpi));
    assert_eq!(print_module_px(600, 0.0), Err(RenderError::InvalidModuleMm));
    assert_eq!(print_module_px(600, -1.0), Err(RenderError::InvalidModuleMm));
    assert_eq!(print_module_px(600, f64::NAN), Err(RenderError::InvalidModuleMm));
    assert_eq!(print_module_px(600, f64::INFINITY), Err(RenderError::InvalidModuleMm));
    let options = RenderOptions::for_print(600, 0.5).unwrap();
    assert_eq!((options.module_px, options.dpi, options.bootstrap), (12, Some(600), true));
}

#[test]
fn option_errors() {
    let grid = a27();
    let bad_sizes = [(16, 20), (22, 20), (20, 4112), (4, 4)];
    for (w, h) in bad_sizes {
        let g = ModuleGrid::new(w, h).unwrap();
        assert_eq!(
            render_png(&g, &RenderOptions::default()),
            Err(RenderError::InvalidSymbolSize { width: w, height: h })
        );
        assert_eq!(
            render_svg(&g, &RenderOptions::default()),
            Err(RenderError::InvalidSymbolSize { width: w, height: h })
        );
    }
    assert_eq!(render_png(&grid, &opts(0, 2, true)), Err(RenderError::ModuleSizeZero));
    assert_eq!(
        render_png(&grid, &opts(4, 1, true)),
        Err(RenderError::QuietZoneTooSmall { quiet_zone: 1 })
    );
    let zero_dpi = RenderOptions { dpi: Some(0), ..RenderOptions::default() };
    assert_eq!(render_svg(&grid, &zero_dpi), Err(RenderError::InvalidDpi));
    assert_eq!(render_png(&grid, &opts(1 << 20, 2, true)), Err(RenderError::TooLarge));
    assert_eq!(render_png(&grid, &opts(4, u32::MAX, true)), Err(RenderError::TooLarge));
    let huge_scale = RenderOptions { bootstrap_scale: Some(u32::MAX), ..RenderOptions::default() };
    assert_eq!(render_svg(&grid, &huge_scale), Err(RenderError::TooLarge));
    for e in [RenderError::TooLarge, RenderError::Bootstrap, RenderError::Png("x".into())] {
        assert!(!e.to_string().is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Properties

proptest! {
    #![proptest_config(ProptestConfig { cases: 40, .. ProptestConfig::default() })]

    #[test]
    fn outputs_agree(
        wk in 5u32..=20,
        hk in 5u32..=20,
        seed in any::<u64>(),
        module_px in 1u32..=5,
        quiet_zone in 2u32..=9,
        bootstrap in any::<bool>(),
        level in prop_oneof![Just(BootstrapLevel::M), Just(BootstrapLevel::Q), Just(BootstrapLevel::H)],
        side in prop_oneof![Just(None), Just(Some(BootstrapSide::Left)), Just(Some(BootstrapSide::Above))],
        dpi in prop_oneof![Just(None), Just(Some(300u32)), Just(Some(96u32))],
    ) {
        let grid = random_grid(4 * wk, 4 * hk, seed);
        let options = RenderOptions {
            module_px,
            quiet_zone,
            bootstrap,
            bootstrap_level: level,
            bootstrap_side: side,
            bootstrap_scale: None,
            dpi,
        };
        let canvas = render_modules(&grid, &options).unwrap();
        check_canvas(&grid, &canvas);
        check_svg(&grid, &options);
        let png = decode(&render_png(&grid, &options).unwrap());
        let s = module_px;
        for y in (0..png.height).step_by(s as usize) {
            for x in (0..png.width).step_by(s as usize) {
                prop_assert_eq!(png.black[(y * png.width + x) as usize], canvas.modules.get(x / s, y / s).unwrap());
            }
        }
    }
}
