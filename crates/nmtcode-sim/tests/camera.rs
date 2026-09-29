//! Camera frames from the channel model, read back by `nmtcode-detect` and `nmtcode`: a few
//! frames per condition, so the tests stay fast. The measurement example runs hundreds.

use nmtcode::{DecodeOptions, SpecError, decode_with_erasures};
use nmtcode_core::ModuleGrid;
use nmtcode_detect::{DetectOptions, LumaImage, Scan, find, read_image};
use nmtcode_render::{Canvas, Layout, RenderOptions, render_png};
use nmtcode_sim::{Channel, Frame, Rng, canvas, jpeg, random_symbol, render};

/// A channel of `width` × `height` with condition A's defaults otherwise.
fn channel(width: u32, height: u32) -> Channel {
    Channel { width, height, ..Channel::default() }
}

/// Renders a `w` × `h` level-`level` symbol under `channel` with `seed` and reads it.
fn capture(channel: &Channel, w: u32, h: u32, level: u8, seed: u64) -> (Frame, Vec<u8>, Scan) {
    let mut rng = Rng::new(seed);
    let (symbol, content) = random_symbol(w, h, level, &mut rng).unwrap();
    let canvas = canvas(&symbol, false).unwrap();
    let frame = render(&canvas, channel, rng.next_u64()).expect("the symbol fits in the frame");
    let scan = find(&frame.image, &DetectOptions::default());
    (frame, content, scan)
}

/// The contents of every symbol of `scan` that decodes.
fn contents(scan: &Scan) -> Vec<Vec<u8>> {
    scan.found
        .iter()
        .filter_map(|f| decode_with_erasures(&f.grid, &f.uncertain, &DecodeOptions::default()).ok())
        .map(|d| d.records.into_iter().flat_map(|r| r.value).collect())
        .collect()
}

#[track_caller]
fn assert_reads(channel: &Channel, cases: &[(u32, u32, u8, u64)]) {
    for &(w, h, level, seed) in cases {
        let (frame, content, scan) = capture(channel, w, h, level, seed);
        assert_eq!(contents(&scan), vec![content], "{w} x {h} L{level} seed {seed}: {scan:?}");
        // The corners the detector reports are the symbol's, within half a module.
        let found = &scan.found[0];
        for (a, b) in found.corners.iter().zip(frame.corners.iter()) {
            let error = (a.x - b.x).hypot(a.y - b.y) / frame.min_pitch;
            assert!(error < 0.5, "{w} x {h} seed {seed}: corner off by {error:.2} modules");
        }
    }
}

#[test]
fn condition_a_at_a_smaller_frame() {
    // k = 3, σ = 0.5 module, 30 dB, JPEG 80, tilt up to 30°, any rotation, clutter.
    assert_reads(
        &channel(960, 720),
        &[(24, 28, 0, 1), (40, 48, 1, 2), (64, 48, 0, 3), (100, 96, 1, 4)],
    );
}

#[test]
fn tilt_of_40_degrees_and_any_rotation() {
    let c = Channel { k: 4.0, blur: 0.35, tilt_deg: (40.0, 40.0), ..channel(960, 720) };
    assert_reads(&c, &[(32, 32, 0, 11), (56, 40, 1, 12), (48, 72, 0, 13)]);
}

#[test]
fn mirror_images() {
    let c = Channel { mirror: true, ..channel(960, 720) };
    let (_, content, scan) = capture(&c, 36, 36, 1, 21);
    assert!(scan.found.first().is_some_and(|f| f.mirrored));
    assert_eq!(contents(&scan), vec![content]);
}

#[test]
fn light_on_dark_is_read_from_the_inverted_image() {
    let c = Channel { reversed: true, ..channel(960, 720) };
    let (_, content, scan) = capture(&c, 40, 32, 1, 31);
    assert!(scan.inverted && scan.found.first().is_some_and(|f| f.inverted), "{scan:?}");
    assert_eq!(contents(&scan), vec![content]);
}

#[test]
fn uneven_light_motion_blur_and_a_low_signal_to_noise_ratio() {
    let uneven = Channel { gradient: 0.8, vignetting: 0.6, ..channel(960, 720) };
    assert_reads(&uneven, &[(44, 44, 1, 41)]);
    let motion = Channel { blur: 0.3, motion_blur: 0.6, ..channel(960, 720) };
    assert_reads(&motion, &[(44, 40, 1, 42)]);
    let noisy = Channel { snr_db: Some(22.0), blur: 0.3, ..channel(960, 720) };
    assert_reads(&noisy, &[(40, 44, 1, 43)]);
}

#[test]
fn a_small_symbol_far_away_and_one_that_fills_the_frame() {
    let far = Channel { k: 2.5, blur: 0.3, tilt_deg: (0.0, 15.0), ..channel(1280, 720) };
    assert_reads(&far, &[(28, 28, 1, 51)]);
    let near = Channel { k: 8.0, blur: 0.5, tilt_deg: (0.0, 10.0), ..channel(640, 480) };
    assert_reads(&near, &[(24, 24, 1, 52)]);
}

/// Two symbols side by side on one canvas, 6 modules apart.
fn pair_canvas(a: &ModuleGrid, b: &ModuleGrid) -> Canvas {
    let (q, gap) = (2u32, 6u32);
    let width = a.width() + b.width() + 4 * q + gap;
    let height = a.height().max(b.height()) + 2 * q;
    let mut modules = ModuleGrid::new(width, height).unwrap();
    for (grid, ox) in [(a, q), (b, 3 * q + a.width() + gap)] {
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                modules.set(ox + x, q + y, grid.get(x, y).unwrap());
            }
        }
    }
    let layout = Layout {
        symbol_width: a.width(),
        symbol_height: a.height(),
        quiet_zone: q,
        module_px: 1,
        canvas_x: -i64::from(q),
        canvas_y: -i64::from(q),
        canvas_width: width,
        canvas_height: height,
        bootstrap: None,
    };
    Canvas { layout, modules }
}

#[test]
fn several_symbols_in_one_frame() {
    let mut rng = Rng::new(61);
    let (a, content_a) = random_symbol(36, 32, 1, &mut rng).unwrap();
    let (b, content_b) = random_symbol(28, 36, 0, &mut rng).unwrap();
    let canvas = pair_canvas(a.grid(), b.grid());
    let c = Channel { blur: 0.35, ..channel(1280, 720) };
    let frame = render(&canvas, &c, 62).unwrap();
    let scan = find(&frame.image, &DetectOptions::default());
    let mut read = contents(&scan);
    read.sort();
    let mut expected = vec![content_a, content_b];
    expected.sort();
    assert_eq!(read, expected);
}

#[test]
fn a_symbol_above_the_largest_area_is_rejected_before_its_data() {
    let c = Channel { blur: 0.3, ..channel(960, 720) };
    let (frame, _, _) = capture(&c, 48, 48, 0, 71);
    let options = DetectOptions { max_area: 48 * 44, ..DetectOptions::default() };
    let scan = find(&frame.image, &options);
    assert!(scan.found.is_empty());
    assert!(scan.rejected.iter().any(|r| r.error == SpecError::SizeLimit), "{scan:?}");
}

/// A clean render of a symbol as RGB, turned by a quarter turn clockwise: a phone that stores
/// the sensor's rows and an EXIF orientation of 8 ("rotate 270° clockwise to display").
fn turned_rgb(grid: &ModuleGrid) -> (u32, u32, Vec<u8>, LumaImage) {
    let png =
        render_png(grid, &RenderOptions { module_px: 4, ..RenderOptions::default() }).unwrap();
    let upright = read_image(&png).unwrap();
    let (w, h) = (upright.width, upright.height);
    // Stored (x, y) shows at displayed (y, w − 1 − x) after a turn of 270° clockwise; the
    // stored image is h × w.
    let mut rgb = Vec::with_capacity(usize::try_from(w * h * 3).unwrap());
    for y in 0..w {
        for x in 0..h {
            let v = upright.get(w - 1 - y, x).unwrap();
            rgb.extend_from_slice(&[v, v.saturating_sub(10), v]);
        }
    }
    (h, w, rgb, upright)
}

#[test]
fn a_colour_jpeg_is_turned_upright_by_its_exif_orientation() {
    let mut rng = Rng::new(81);
    let (symbol, content) = random_symbol(32, 24, 1, &mut rng).unwrap();
    let (sw, sh, rgb, upright) = turned_rgb(symbol.grid());
    let file = jpeg::encode_rgb(sw, sh, &rgb, 90, Some(8)).unwrap();
    let image = read_image(&file).unwrap();
    assert_eq!((image.width, image.height), (upright.width, upright.height));
    let scan = find(&image, &DetectOptions::default());
    assert_eq!(contents(&scan), vec![content]);
    // Without the tag the stored image is read as it is, turned; it still decodes.
    let plain = jpeg::encode_rgb(sw, sh, &rgb, 90, None).unwrap();
    let image = read_image(&plain).unwrap();
    assert_eq!((image.width, image.height), (sw, sh));
    assert_eq!(contents(&find(&image, &DetectOptions::default())).len(), 1);
}

#[test]
fn luma_from_rgba_matches_the_jpeg_and_png_paths() {
    let mut rng = Rng::new(91);
    let (symbol, content) = random_symbol(24, 24, 0, &mut rng).unwrap();
    let png = render_png(symbol.grid(), &RenderOptions::default()).unwrap();
    let luma = read_image(&png).unwrap();
    let rgba: Vec<u8> = luma.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect();
    let image = LumaImage::from_rgba(luma.width, luma.height, &rgba).unwrap();
    assert_eq!(image, luma);
    assert_eq!(contents(&find(&image, &DetectOptions::default())), vec![content]);
}
