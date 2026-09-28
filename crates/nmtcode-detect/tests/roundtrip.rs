//! Render → detect round trips: the grid read back must equal the grid drawn, exactly.

// Test code: `as` casts convert small, known image coordinates and sizes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

mod common;

use common::*;
use nmtcode_core::ModuleGrid;
use nmtcode_detect::{LumaImage, decode_png, find_symbols, read_png};
use nmtcode_render::{RenderOptions, render_png};

fn options(module_px: u32, quiet_zone: u32, bootstrap: bool) -> RenderOptions {
    RenderOptions { module_px, quiet_zone, bootstrap, ..RenderOptions::default() }
}

fn render_luma(grid: &ModuleGrid, options: &RenderOptions) -> LumaImage {
    decode_png(&render_png(grid, options).unwrap()).unwrap()
}

#[track_caller]
fn assert_single(found: &[ModuleGrid], expected: &ModuleGrid, what: &str) {
    assert_eq!(found.len(), 1, "{what}: {} symbols found", found.len());
    assert_eq!(&found[0], expected, "{what}");
}

#[test]
fn a27_transcription_matches_its_hex_rows() {
    for (y, (row, hex)) in A27_ROWS.iter().zip(A27_HEX).enumerate() {
        let bits: u32 = row.chars().fold(0, |acc, c| acc << 1 | u32::from(c == '#'));
        assert_eq!(bits, hex, "row {y}");
    }
}

#[test]
fn a27_default_options() {
    let grid = a27();
    let found = read_png(&render_png(&grid, &RenderOptions::default()).unwrap()).unwrap();
    assert_single(&found, &grid, "A.2.7, default options");
}

#[test]
fn a27_module_sizes_and_quiet_zones() {
    let grid = a27();
    for module_px in 2..=9 {
        for quiet_zone in [2, 3, 4, 7] {
            for bootstrap in [false, true] {
                let png = render_png(&grid, &options(module_px, quiet_zone, bootstrap)).unwrap();
                let found = read_png(&png).unwrap();
                assert_single(
                    &found,
                    &grid,
                    &format!("{module_px} px, Q {quiet_zone}, bootstrap {bootstrap}"),
                );
            }
        }
    }
}

#[test]
fn a27_larger_modules() {
    let grid = a27();
    for module_px in [12, 16, 25] {
        let found = read_png(&render_png(&grid, &options(module_px, 2, true)).unwrap()).unwrap();
        assert_single(&found, &grid, &format!("{module_px} px"));
    }
}

#[test]
fn map_2_8_symbols() {
    for seed in 0..6 {
        let grid = map_2_8(seed);
        for module_px in [2, 3, 5] {
            for bootstrap in [false, true] {
                let found =
                    read_png(&render_png(&grid, &options(module_px, 2, bootstrap)).unwrap())
                        .unwrap();
                assert_single(&found, &grid, &format!("2.8 seed {seed}, {module_px} px"));
            }
        }
    }
}

#[test]
fn other_sizes() {
    let cases = [(48, 32), (32, 48), (20, 20), (20, 28), (64, 64), (100, 100), (132, 60)];
    for (i, &(w, h)) in cases.iter().enumerate() {
        let grid = make_symbol(w, h, 100 + i as u64);
        for module_px in [2, 3, 4] {
            for bootstrap in [false, true] {
                let found =
                    read_png(&render_png(&grid, &options(module_px, 2, bootstrap)).unwrap())
                        .unwrap();
                assert_single(
                    &found,
                    &grid,
                    &format!("{w} x {h}, {module_px} px, bootstrap {bootstrap}"),
                );
            }
        }
    }
}

#[test]
fn large_symbol_400() {
    let grid = make_symbol(400, 400, 7);
    for module_px in [2, 3] {
        let found = read_png(&render_png(&grid, &options(module_px, 2, true)).unwrap()).unwrap();
        assert_single(&found, &grid, &format!("400 x 400, {module_px} px"));
    }
}

#[test]
fn extreme_aspect_ratios_4108() {
    for (w, h) in [(4108, 20), (20, 4108), (4108, 24)] {
        let grid = make_symbol(w, h, u64::from(w + h));
        let found = read_png(&render_png(&grid, &options(2, 2, true)).unwrap()).unwrap();
        assert_single(&found, &grid, &format!("{w} x {h}"));
    }
}

#[test]
#[ignore = "about 67 megapixels; run with --ignored"]
fn largest_symbol() {
    let grid = make_symbol(4108, 4108, 3);
    let found = read_png(&render_png(&grid, &options(2, 2, false)).unwrap()).unwrap();
    assert_single(&found, &grid, "4108 x 4108");
}

/// (base module px, factor): the resampled module size is their product.
const RESAMPLINGS: [(u32, f64); 12] = [
    (2, 1.85),  // 3.7 px, upscaled from 2 px
    (4, 0.925), // 3.7 px, downscaled from 4 px
    (10, 0.37), // 3.7 px, downscaled from 10 px
    (3, 1.3),   // 3.9 px
    (4, 0.65),  // 2.6 px
    (10, 0.25), // 2.5 px
    (10, 0.23), // 2.3 px
    (5, 0.5),   // 2.5 px
    (2, 2.65),  // 5.3 px
    (5, 1.45),  // 7.25 px
    (3, 3.1),   // 9.3 px
    (7, 0.61),  // 4.27 px
];

#[test]
fn non_integer_resampling() {
    let symbols = [a27(), make_symbol(100, 100, 11), make_symbol(48, 32, 12), map_2_8(9)];
    for grid in &symbols {
        for &(base, factor) in &RESAMPLINGS {
            for bootstrap in [false, true] {
                let image = resample(&render_luma(grid, &options(base, 2, bootstrap)), factor);
                let found = find_symbols(&image);
                let what = format!(
                    "{} x {}, {base} px × {factor} = {:.3} px, bootstrap {bootstrap}",
                    grid.width(),
                    grid.height(),
                    f64::from(base) * factor
                );
                assert_single(&found, grid, &what);
            }
        }
    }
}

#[test]
fn resampled_400() {
    let grid = make_symbol(400, 400, 21);
    for (base, factor) in [(4, 0.925), (2, 1.85), (10, 0.29)] {
        let image = resample(&render_luma(&grid, &options(base, 2, true)), factor);
        assert_single(&find_symbols(&image), &grid, &format!("400 x 400, {base} × {factor}"));
    }
}

#[test]
fn extra_white_margin() {
    let grid = a27();
    let image = render_luma(&grid, &options(3, 2, true));
    for (l, t, r, b) in [(0, 0, 0, 0), (50, 3, 0, 170), (1, 200, 333, 9), (400, 400, 400, 400)] {
        assert_single(
            &find_symbols(&pad(&image, l, t, r, b)),
            &grid,
            &format!("margin {l} {t} {r} {b}"),
        );
    }
}

/// A named change of every pixel value.
type Change = (&'static str, fn(u8) -> u8);

#[test]
fn uniform_brightness_changes() {
    let grid = make_symbol(64, 48, 5);
    let image = render_luma(&grid, &options(3, 2, true));
    let changes: [Change; 5] = [
        ("dimmer", |v| (f64::from(v) * 0.75).round() as u8),
        ("lower contrast", |v| (40.0 + f64::from(v) * 0.7).round() as u8),
        ("brighter", |v| v.saturating_add(35)),
        ("darker", |v| v.saturating_sub(30)),
        ("grey on grey", |v| (70.0 + f64::from(v) * 0.4).round() as u8),
    ];
    for (name, change) in changes {
        assert_single(&find_symbols(&map_pixels(&image, change)), &grid, name);
        let resampled = resample(&map_pixels(&image, change), 1.23);
        assert_single(&find_symbols(&resampled), &grid, &format!("{name}, resampled"));
    }
}

#[test]
fn png_colour_types() {
    let grid = make_symbol(48, 32, 77);
    let own = render_png(&grid, &options(3, 2, true)).unwrap();
    let image = decode_png(&own).unwrap();
    let resampled = resample(&image, 1.37);
    let margin = 3 * 2;
    let (w, h) = (image.width, image.height);
    let encodings: Vec<(&str, Vec<u8>)> = vec![
        ("1-bit grey (renderer)", own.clone()),
        ("8-bit grey", png_gray8(&image)),
        ("16-bit grey", png_gray16(&image)),
        ("8-bit RGB", png_rgb(&image)),
        ("8-bit RGBA, opaque", png_rgba(&image, |_, _| true)),
        (
            "8-bit RGBA, transparent margin",
            png_rgba(&image, |x, y| x >= margin && y >= margin && x < w - margin && y < h - margin),
        ),
        ("8-bit grey + alpha", png_gray_alpha(&image)),
        ("1-bit palette", png_palette(&image)),
        ("8-bit grey, resampled", png_gray8(&resampled)),
        ("8-bit RGB, resampled", png_rgb(&resampled)),
        ("8-bit RGBA, resampled", png_rgba(&resampled, |_, _| true)),
    ];
    for (name, bytes) in encodings {
        let found = read_png(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_single(&found, &grid, name);
    }
}

#[test]
fn rotations_and_mirror_images() {
    for grid in [a27(), make_symbol(48, 32, 31), make_symbol(20, 36, 32)] {
        let image = render_luma(&grid, &options(3, 2, true));
        let turned90 = rotate90(&image);
        let turned180 = rotate90(&turned90);
        let turned270 = rotate90(&turned180);
        for (name, img) in [
            ("90°", &turned90),
            ("180°", &turned180),
            ("270°", &turned270),
            ("mirror", &mirror(&image)),
            ("transpose", &transpose(&image)),
            ("mirror of 90°", &mirror(&turned90)),
            ("90°, resampled", &resample(&turned90, 1.21)),
        ] {
            let found = find_symbols(img);
            assert_single(&found, &grid, &format!("{} x {}, {name}", grid.width(), grid.height()));
        }
    }
}

#[test]
fn several_symbols_in_one_image() {
    let a = a27();
    let b = make_symbol(48, 32, 41);
    let c = map_2_8(3);
    let ia = render_luma(&a, &options(3, 2, true));
    let ib = render_luma(&b, &options(3, 2, false));
    let ic = render_luma(&c, &options(3, 4, true));
    let image = beside(&beside(&ia, &ib, 0), &ic, 17);
    let found = find_symbols(&image);
    assert_eq!(found.len(), 3, "{} symbols", found.len());
    for expected in [&a, &b, &c] {
        assert!(found.contains(expected), "missing {} x {}", expected.width(), expected.height());
    }
    // Two copies of one symbol side by side, aligned: two results, not one spanning both.
    let twice = beside(&ib, &ib, 0);
    let found = find_symbols(&twice);
    assert_eq!(found, vec![b.clone(), b]);
}

#[test]
fn bootstrap_qr_alone_is_not_a_symbol() {
    let grid = make_symbol(64, 64, 51);
    let opts = options(3, 2, true);
    let canvas = nmtcode_render::render_modules(&grid, &opts).unwrap();
    let mut image = render_luma(&grid, &opts);
    // Paint the NMT Code symbol and its quiet zone white; only the QR Code is left.
    let (ox, oy) = canvas.layout.symbol_offset();
    let px = opts.module_px;
    for y in 0..image.height {
        for x in 0..image.width {
            if x >= (ox - 2) * px && y + 2 * px >= oy * px && y < (oy + 66) * px {
                image.pixels[(y * image.width + x) as usize] = 255;
            }
        }
    }
    assert!(image.pixels.iter().any(|&v| v < 128), "the QR Code is still drawn");
    assert!(find_symbols(&image).is_empty());
    assert!(find_symbols(&resample(&image, 1.7)).is_empty());
}

#[test]
fn blank_and_noise_images() {
    assert!(find_symbols(&LumaImage::new(50, 40, vec![255; 2000]).unwrap()).is_empty());
    assert!(find_symbols(&LumaImage::new(50, 40, vec![0; 2000]).unwrap()).is_empty());
    let mut rng = Rng(9);
    let noise: Vec<u8> = (0..300 * 200).map(|_| (rng.next() & 0xFF) as u8).collect();
    assert!(find_symbols(&LumaImage::new(300, 200, noise).unwrap()).is_empty());
    let blocks: Vec<u8> = (0..300 * 200)
        .map(|i| if ((i % 300) / 4 + (i / 300) / 4) % 3 == 0 { 0 } else { 255 })
        .collect();
    assert!(find_symbols(&LumaImage::new(300, 200, blocks).unwrap()).is_empty());
}

#[test]
fn inconsistent_luma_image_gives_nothing() {
    let grid = a27();
    let mut image = render_luma(&grid, &options(3, 2, false));
    image.pixels.pop();
    assert!(find_symbols(&image).is_empty());
    image.width = 0;
    assert!(find_symbols(&image).is_empty());
}

#[test]
fn damaged_finder_is_rejected_but_three_suffice() {
    let grid = make_symbol(48, 48, 61);
    let opts = options(3, 2, false);
    let mut image = render_luma(&grid, &opts);
    let px = opts.module_px;
    let q = opts.quiet_zone;
    // Cover the BR finder with white: three finders are left (5.11).
    for y in (q + 42) * px..(q + 48) * px {
        for x in (q + 42) * px..(q + 48) * px {
            image.pixels[(y * image.width + x) as usize] = 255;
        }
    }
    let found = find_symbols(&image);
    assert_eq!(found.len(), 1);
    // The sampled grid shows the covered corner as light; everything else is unchanged.
    let mut expected = grid.clone();
    for y in 42..48 {
        for x in 42..48 {
            expected.set(x, y, false);
        }
    }
    assert_eq!(found[0], expected);
    // Cover a second finder too: no symbol.
    for y in q * px..(q + 6) * px {
        for x in q * px..(q + 6) * px {
            image.pixels[(y * image.width + x) as usize] = 255;
        }
    }
    assert!(find_symbols(&image).is_empty());
}
