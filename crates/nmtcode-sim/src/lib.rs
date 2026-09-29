//! Synthetic camera frames of NMT Code symbols: a test and measurement tool for the reader, not
//! a user feature.
//!
//! A frame is made in the order light takes:
//!
//! 1. The scene: the symbol's canvas from `nmtcode-render` (modules, quiet zone, optional
//!    bootstrap QR Code) printed on a sheet of paper with a light margin, in front of a
//!    cluttered background of random shapes, some of them dark squares with light borders.
//! 2. The camera: a pinhole camera with a horizontal field of view of 66°, looking at the sheet
//!    tilted by a random angle around a random axis in its plane, turned by any angle in its
//!    plane, at the distance that makes one module `k` pixels long at the symbol's centre, along
//!    the tilt axis (the foreshortened direction gets k · cos(tilt)). Each pixel integrates the
//!    scene over its area (4 × 4 samples).
//! 3. Light: a linear illumination gradient in a random direction and a radial vignetting.
//! 4. Optics: a Gaussian blur of σ modules (σ · k pixels), and optionally a motion blur along a
//!    random direction.
//! 5. The sensor: an encoding gamma, and additive Gaussian noise at a signal-to-noise ratio in
//!    dB: `20 · log10(contrast / σ_noise)`, with the contrast between paper and ink at the symbol
//!    centre after gamma, in 8-bit values.
//! 6. Storage: JPEG compression at a given quality (greyscale baseline, which gives the luma
//!    artefacts of a colour JPEG), decoded again by `nmtcode_detect::read_image`.
//!
//! Every frame is fixed by its seed.

// A measurement tool: frame buffers hold `f32` light values converted from `f64` geometry,
// the camera model uses the usual one-letter names (x, y, u, v, f, h, z), and a capture
// condition is a set of switches.
#![allow(
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::struct_excessive_bools
)]

pub mod jpeg;
pub mod rng;

use nmtcode::{ContentType, EncodeError, EncodeOptions, Record, SizeRule, Symbol};
use nmtcode_detect::{LumaImage, Point};
use nmtcode_render::{Canvas, RenderOptions, render_modules};

pub use rng::{Rng, seed};

/// The conditions of a synthetic capture.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Camera pixels per module at the symbol's centre, along the tilt axis.
    pub k: f64,
    /// Standard deviation of the Gaussian blur, in modules.
    pub blur: f64,
    /// Signal-to-noise ratio in dB; `None` for no noise.
    pub snr_db: Option<f64>,
    /// JPEG quality 1 to 100; `None` for no compression.
    pub jpeg_quality: Option<u8>,
    /// The tilt is uniform between these two angles, in degrees.
    pub tilt_deg: (f64, f64),
    /// Whether the symbol is turned by a uniform random angle in its plane.
    pub rotate: bool,
    /// Relative change of the illumination across the frame (0 = even light).
    pub gradient: f64,
    /// Relative fall-off of the illumination in the frame corners (0 = none).
    pub vignetting: f64,
    /// Encoding gamma: the stored value is the linear value to the power 1 / gamma.
    pub gamma: f64,
    /// Length of a motion blur in modules (0 = none).
    pub motion_blur: f64,
    /// Whether the background is cluttered; otherwise it is plain grey.
    pub clutter: bool,
    /// Whether the frame shows the symbol mirrored.
    pub mirror: bool,
    /// Whether the symbol is shown light on dark.
    pub reversed: bool,
}

impl Default for Channel {
    /// Condition A of the measurement: 1920 × 1080, k = 3, σ = 0.5 module, 30 dB, JPEG
    /// quality 80, tilt up to 30°, any rotation, clutter, mild uneven light.
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            k: 3.0,
            blur: 0.5,
            snr_db: Some(30.0),
            jpeg_quality: Some(80),
            tilt_deg: (0.0, 30.0),
            rotate: true,
            gradient: 0.3,
            vignetting: 0.3,
            gamma: 2.2,
            motion_blur: 0.0,
            clutter: true,
            mirror: false,
            reversed: false,
        }
    }
}

/// One synthetic frame.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The frame as the reader receives it (after JPEG, when enabled).
    pub image: LumaImage,
    /// The JPEG file, when compression is enabled.
    pub jpeg: Option<Vec<u8>>,
    /// The true image positions of the centres of the finders TL, TR, BL and BR.
    pub finder_centres: [Point; 4],
    /// The true image positions of the symbol's outer corners: top-left, top-right,
    /// bottom-right, bottom-left, as `nmtcode_detect::Found::corners` reports them.
    pub corners: [Point; 4],
    /// Camera pixels per module along the foreshortened direction at the symbol's centre.
    pub min_pitch: f64,
}

/// A 3 × 3 matrix, row-major.
type Mat3 = [f64; 9];

fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut m = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            m[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
        }
    }
    m
}

fn apply(m: &Mat3, x: f64, y: f64) -> (f64, f64) {
    let w = m[6] * x + m[7] * y + m[8];
    let w = if w.abs() < 1e-12 { 1e-12 } else { w };
    ((m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w)
}

fn inverse(m: &Mat3) -> Option<Mat3> {
    let [a, b, c, d, e, f, g, h, i] = *m;
    let co = [
        e * i - f * h,
        c * h - b * i,
        b * f - c * e,
        f * g - d * i,
        a * i - c * g,
        c * d - a * f,
        d * h - e * g,
        b * g - a * h,
        a * e - b * d,
    ];
    let det = a * co[0] + b * co[3] + c * co[6];
    (det.abs() > 1e-18).then(|| co.map(|v| v / det))
}

/// Rotation by `angle` about the unit axis (`ax`, `ay`, 0).
fn axis_rotation(ax: f64, ay: f64, angle: f64) -> Mat3 {
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        t * ax * ax + c,
        t * ax * ay,
        s * ay,
        t * ax * ay,
        t * ay * ay + c,
        -s * ax,
        -s * ay,
        s * ax,
        c,
    ]
}

/// A random shape of the background, in frame pixels.
enum Shape {
    Rect {
        cx: f64,
        cy: f64,
        hw: f64,
        hh: f64,
        cos: f64,
        sin: f64,
        v: f32,
    },
    Disc {
        cx: f64,
        cy: f64,
        r: f64,
        v: f32,
    },
    /// A dark square with a light border: a decoy for the finder search.
    Framed {
        cx: f64,
        cy: f64,
        half: f64,
        cos: f64,
        sin: f64,
        dark: f32,
        light: f32,
    },
}

impl Shape {
    fn bounds(&self) -> (f64, f64, f64, f64) {
        match *self {
            Self::Rect { cx, cy, hw, hh, .. } => {
                let r = hw.hypot(hh);
                (cx - r, cy - r, cx + r, cy + r)
            }
            Self::Disc { cx, cy, r, .. } => (cx - r, cy - r, cx + r, cy + r),
            Self::Framed { cx, cy, half, .. } => {
                let r = half * 1.5 * core::f64::consts::SQRT_2;
                (cx - r, cy - r, cx + r, cy + r)
            }
        }
    }

    fn value(&self, x: f64, y: f64) -> Option<f32> {
        match *self {
            Self::Rect { cx, cy, hw, hh, cos, sin, v } => {
                let (dx, dy) = (x - cx, y - cy);
                let (u, w) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                (u.abs() <= hw && w.abs() <= hh).then_some(v)
            }
            Self::Disc { cx, cy, r, v } => ((x - cx).hypot(y - cy) <= r).then_some(v),
            Self::Framed { cx, cy, half, cos, sin, dark, light } => {
                let (dx, dy) = (x - cx, y - cy);
                let (u, w) = ((dx * cos + dy * sin).abs(), (-dx * sin + dy * cos).abs());
                let m = u.max(w);
                if m <= half {
                    // A few light holes so the decoy has an inner pattern.
                    let hole = u < half * 0.4 && w < half * 0.4 && (dx * cos + dy * sin) > 0.0;
                    Some(if hole { light } else { dark })
                } else if m <= half * 1.4 {
                    Some(light)
                } else {
                    None
                }
            }
        }
    }
}

fn clutter(rng: &mut Rng, width: usize, height: usize, pitch: f64, enabled: bool) -> Vec<f32> {
    let base = rng.range(0.2, 0.7) as f32;
    let mut out = vec![base; width * height];
    if !enabled {
        return out;
    }
    // A soft gradient of the background itself.
    let (gx, gy) = (rng.range(-0.25, 0.25), rng.range(-0.25, 0.25));
    for (i, v) in out.iter_mut().enumerate() {
        let (x, y) = (count(i % width) / count(width), count(i / width) / count(height));
        *v = (f64::from(*v) * (1.0 + gx * (x - 0.5) + gy * (y - 0.5))).clamp(0.02, 0.98) as f32;
    }
    let (wf, hf) = (count(width), count(height));
    let n = 60 + rng.below(80);
    let mut shapes = Vec::new();
    for _ in 0..n {
        let (cx, cy) = (rng.range(0.0, wf), rng.range(0.0, hf));
        let angle = rng.range(0.0, core::f64::consts::TAU);
        let v = rng.range(0.03, 0.95) as f32;
        let shape = match rng.below(10) {
            0..=4 => Shape::Rect {
                cx,
                cy,
                hw: rng.range(2.0, 120.0),
                hh: rng.range(1.0, 60.0),
                cos: angle.cos(),
                sin: angle.sin(),
                v,
            },
            5..=7 => Shape::Disc { cx, cy, r: rng.range(2.0, 80.0), v },
            _ => {
                let dark = rng.range(0.03, 0.2) as f32;
                let light = rng.range(0.7, 0.95) as f32;
                Shape::Framed {
                    cx,
                    cy,
                    half: pitch * rng.range(1.5, 4.0),
                    cos: angle.cos(),
                    sin: angle.sin(),
                    dark,
                    light,
                }
            }
        };
        shapes.push(shape);
    }
    for shape in &shapes {
        let (x0, y0, x1, y1) = shape.bounds();
        let (xa, ya) = (to_index(x0.floor(), width), to_index(y0.floor(), height));
        let (xb, yb) = (to_index(x1.ceil(), width), to_index(y1.ceil(), height));
        for y in ya..yb {
            for x in xa..xb {
                if let (Some(v), Some(slot)) =
                    (shape.value(count(x) + 0.5, count(y) + 0.5), out.get_mut(y * width + x))
                {
                    *slot = v;
                }
            }
        }
    }
    // Fine texture.
    for v in &mut out {
        *v = (f64::from(*v) + rng.normal() * 0.01).clamp(0.01, 0.99) as f32;
    }
    out
}

#[allow(clippy::cast_precision_loss)]
fn count(v: usize) -> f64 {
    v as f64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_index(v: f64, limit: usize) -> usize {
    (v.max(0.0) as usize).min(limit)
}

/// Separable Gaussian blur with standard deviation `sigma` pixels, edges clamped.
fn gaussian(buffer: &mut [f32], width: usize, height: usize, sigma: f64) {
    if sigma < 0.05 {
        return;
    }
    let radius = (3.0 * sigma).ceil().max(1.0);
    let r = to_index(radius, 64);
    let kernel: Vec<f32> = {
        let raw: Vec<f64> = (0..=2 * r)
            .map(|i| (-(count(i) - count(r)).powi(2) / (2.0 * sigma * sigma)).exp())
            .collect();
        let sum: f64 = raw.iter().sum();
        raw.iter().map(|v| (v / sum) as f32).collect()
    };
    let mut tmp = vec![0.0f32; buffer.len()];
    for y in 0..height {
        let row = &buffer[y * width..(y + 1) * width];
        for x in 0..width {
            let mut s = 0.0f32;
            for (k, &w) in kernel.iter().enumerate() {
                let xi = (x + k).saturating_sub(r).min(width - 1);
                s += w * row[xi];
            }
            tmp[y * width + x] = s;
        }
    }
    for x in 0..width {
        for y in 0..height {
            let mut s = 0.0f32;
            for (k, &w) in kernel.iter().enumerate() {
                let yi = (y + k).saturating_sub(r).min(height - 1);
                s += w * tmp[yi * width + x];
            }
            buffer[y * width + x] = s;
        }
    }
}

/// Motion blur: the mean along a segment of `length` pixels in direction `angle`.
fn motion(buffer: &mut [f32], width: usize, height: usize, length: f64, angle: f64) {
    if length < 0.5 {
        return;
    }
    let taps = to_index(length.ceil(), 256).max(2);
    let (dx, dy) = (angle.cos(), angle.sin());
    let src = buffer.to_vec();
    let get = |x: f64, y: f64| -> f32 {
        let xi = to_index(x.round(), width - 1);
        let yi = to_index(y.round(), height - 1);
        src[yi * width + xi]
    };
    for y in 0..height {
        for x in 0..width {
            let mut s = 0.0f32;
            for t in 0..taps {
                let f = (count(t) / count(taps - 1) - 0.5) * length;
                s += get(count(x) + f * dx, count(y) + f * dy);
            }
            buffer[y * width + x] = s / count(taps) as f32;
        }
    }
}

/// Samples per pixel along each axis where the paper is.
const S: usize = 4;

/// Renders `canvas` into a frame under `channel`, with randomness from `seed`. `None` when the
/// symbol does not fit in the frame in 40 tries of position and angle.
// The steps of the channel in their order, in one place.
#[allow(clippy::too_many_lines)]
pub fn render(canvas: &Canvas, channel: &Channel, seed: u64) -> Option<Frame> {
    let mut rng = Rng::new(seed);
    let (width, height) =
        (usize::try_from(channel.width).ok()?, usize::try_from(channel.height).ok()?);
    let (wf, hf) = (count(width), count(height));
    let layout = &canvas.layout;
    let (sw, sh) = (f64::from(layout.symbol_width), f64::from(layout.symbol_height));
    let (ox, oy) = layout.symbol_offset();
    let (cw, ch) = (f64::from(layout.canvas_width), f64::from(layout.canvas_height));
    // Plane coordinates: modules, origin at the symbol's centre. The canvas spans
    // [−ox − W/2, cw − ox − W/2] and the paper adds a margin.
    let (px0, py0) = (-f64::from(ox) - sw / 2.0, -f64::from(oy) - sh / 2.0);
    let margin = rng.range(1.0, 6.0);
    let paper = (px0 - margin, py0 - margin, px0 + cw + margin, py0 + ch + margin);
    let f = (wf / 2.0) / (33.0f64.to_radians()).tan();
    let z = f / channel.k;
    let mut placed: Option<(Mat3, f64)> = None;
    for _ in 0..40 {
        let tilt =
            rng.range(channel.tilt_deg.0, channel.tilt_deg.1.max(channel.tilt_deg.0)).to_radians();
        let phi = rng.range(0.0, core::f64::consts::TAU);
        let psi = if channel.rotate { rng.range(0.0, core::f64::consts::TAU) } else { 0.0 };
        let (sp, cp) = psi.sin_cos();
        let rz: Mat3 = [cp, -sp, 0.0, sp, cp, 0.0, 0.0, 0.0, 1.0];
        let rt = axis_rotation(phi.cos(), phi.sin(), tilt);
        let r = mul(&rt, &rz);
        let mirror = if channel.mirror { -1.0 } else { 1.0 };
        let (tx, ty) = (rng.range(-0.4, 0.4) * wf * z / f, rng.range(-0.4, 0.4) * hf * z / f);
        // H = K [r1 r2 t] with K = [f 0 w/2; 0 f h/2; 0 0 1]; mirror flips the plane's x.
        let rt3: Mat3 = [mirror * r[0], r[1], tx, mirror * r[3], r[4], ty, mirror * r[6], r[7], z];
        let k: Mat3 = [f, 0.0, wf / 2.0, 0.0, f, hf / 2.0, 0.0, 0.0, 1.0];
        let h = mul(&k, &rt3);
        let corners =
            [(paper.0, paper.1), (paper.2, paper.1), (paper.2, paper.3), (paper.0, paper.3)];
        let inside = corners.iter().all(|&(x, y)| {
            let w = h[6] * x + h[7] * y + h[8];
            let (u, v) = apply(&h, x, y);
            w > 0.0 && u >= 2.0 && v >= 2.0 && u <= wf - 2.0 && v <= hf - 2.0
        });
        if inside {
            placed = Some((h, tilt));
            break;
        }
    }
    let (h, tilt) = placed?;
    let hinv = inverse(&h)?;
    let pitch = channel.k;
    let mut buffer = clutter(&mut rng, width, height, pitch, channel.clutter);
    let (ink, sheet) = if channel.reversed { (0.80f32, 0.06f32) } else { (0.06f32, 0.80f32) };
    let module_at = |x: f64, y: f64| -> Option<f32> {
        if x < paper.0 || y < paper.1 || x >= paper.2 || y >= paper.3 {
            return None;
        }
        let (u, v) = (x - px0, y - py0);
        if u < 0.0 || v < 0.0 || u >= cw || v >= ch {
            return Some(sheet);
        }
        let (Ok(mu), Ok(mv)) = (
            u32::try_from(to_index(u.floor(), 1 << 22)),
            u32::try_from(to_index(v.floor(), 1 << 22)),
        ) else {
            return Some(sheet);
        };
        Some(if canvas.modules.get(mu, mv) == Some(true) { ink } else { sheet })
    };
    // Bounding box of the paper in the frame.
    let corners: Vec<(f64, f64)> =
        [(paper.0, paper.1), (paper.2, paper.1), (paper.2, paper.3), (paper.0, paper.3)]
            .iter()
            .map(|&(x, y)| apply(&h, x, y))
            .collect();
    let bx0 = to_index(corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min).floor(), width);
    let by0 = to_index(corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min).floor(), height);
    let bx1 = to_index(corners.iter().map(|c| c.0).fold(0.0, f64::max).ceil() + 1.0, width);
    let by1 = to_index(corners.iter().map(|c| c.1).fold(0.0, f64::max).ceil() + 1.0, height);
    for y in by0..by1 {
        for x in bx0..bx1 {
            let Some(slot) = buffer.get_mut(y * width + x) else { continue };
            let background = *slot;
            let mut sum = 0.0f32;
            for sy in 0..S {
                for sx in 0..S {
                    let (fx, fy) = (
                        count(x) + (count(sx) + 0.5) / count(S),
                        count(y) + (count(sy) + 0.5) / count(S),
                    );
                    let (u, v) = apply(&hinv, fx, fy);
                    sum += module_at(u, v).unwrap_or(background);
                }
            }
            *slot = sum / count(S * S) as f32;
        }
    }
    // Illumination.
    let angle = rng.range(0.0, core::f64::consts::TAU);
    let (gx, gy) = (angle.cos(), angle.sin());
    let half_diag = wf.hypot(hf) / 2.0;
    let exposure = rng.range(0.85, 1.15);
    let light = |x: f64, y: f64| -> f64 {
        let (dx, dy) = (x - wf / 2.0, y - hf / 2.0);
        let g = 1.0 + channel.gradient * (dx * gx + dy * gy) / half_diag;
        let r2 = (dx * dx + dy * dy) / (half_diag * half_diag);
        exposure * g * (1.0 - channel.vignetting * r2)
    };
    for (i, v) in buffer.iter_mut().enumerate() {
        *v = (f64::from(*v) * light(count(i % width) + 0.5, count(i / width) + 0.5)) as f32;
    }
    gaussian(&mut buffer, width, height, channel.blur * channel.k);
    if channel.motion_blur > 0.0 {
        motion(
            &mut buffer,
            width,
            height,
            channel.motion_blur * channel.k,
            rng.range(0.0, core::f64::consts::PI),
        );
    }
    // Sensor: gamma and noise.
    let (scx, scy) = apply(&h, 0.0, 0.0);
    let centre_light = light(scx, scy);
    let encode = |e: f64| 255.0 * e.max(0.0).powf(1.0 / channel.gamma);
    let contrast = (encode(0.80 * centre_light) - encode(0.06 * centre_light)).abs();
    let noise = channel.snr_db.map_or(0.0, |snr| contrast / 10f64.powf(snr / 20.0));
    let pixels: Vec<u8> = buffer
        .iter()
        .map(|&v| {
            let value = encode(f64::from(v)) + noise * rng.normal();
            u8::try_from(to_index(value.round().clamp(0.0, 255.0), 255)).unwrap_or(255)
        })
        .collect();
    let (jpeg, image) = match channel.jpeg_quality {
        Some(q) => {
            let bytes = jpeg::encode_luma(channel.width, channel.height, &pixels, q, None)?;
            let image = nmtcode_detect::read_image(&bytes).ok()?;
            (Some(bytes), image)
        }
        None => (None, LumaImage::new(channel.width, channel.height, pixels)?),
    };
    let fc = [(2.5, 2.5), (sw - 2.5, 2.5), (2.5, sh - 2.5), (sw - 2.5, sh - 2.5)];
    let finder_centres = fc.map(|(u, v)| {
        let (x, y) = apply(&h, u - sw / 2.0, v - sh / 2.0);
        Point::new(x, y)
    });
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)].map(|(u, v)| {
        let (x, y) = apply(&h, u - sw / 2.0, v - sh / 2.0);
        Point::new(x, y)
    });
    Some(Frame { image, jpeg, finder_centres, corners, min_pitch: channel.k * tilt.cos() })
}

/// A symbol of exactly `width` × `height` modules at `level`, filled with random bytes from
/// `rng`, and the bytes. `None` when the size or level is not valid.
pub fn random_symbol(
    width: u32,
    height: u32,
    level: u8,
    rng: &mut Rng,
) -> Option<(Symbol, Vec<u8>)> {
    let capacity = nmtcode::capacity(width, height, level)?;
    let options = EncodeOptions {
        level: Some(level),
        size: SizeRule::Exact { width, height },
        ..EncodeOptions::default()
    };
    let mut len = capacity.saturating_sub(8).max(1);
    for _ in 0..4 {
        let content: Vec<u8> =
            (0..len).map(|_| u8::try_from(rng.below(256)).unwrap_or(0)).collect();
        let record = Record { content_type: ContentType::UNSPECIFIED, value: &content };
        match nmtcode::encode(&[record], &options) {
            Ok(symbol) => return Some((symbol, content)),
            Err(EncodeError::DoesNotFit { container_len, capacity, .. }) => {
                let over = container_len.saturating_sub(capacity);
                len = len.saturating_sub(over.max(1));
                if len == 0 {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
    None
}

/// The canvas of `symbol` as `nmtcode-render` draws it: a quiet zone of 2 modules, with or
/// without the bootstrap QR Code.
pub fn canvas(symbol: &Symbol, bootstrap: bool) -> Option<Canvas> {
    let options = RenderOptions { module_px: 1, bootstrap, ..RenderOptions::default() };
    render_modules(symbol.grid(), &options).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic() {
        let mut rng = Rng::new(3);
        let (symbol, _) = random_symbol(24, 24, 0, &mut rng).unwrap();
        let canvas = canvas(&symbol, false).unwrap();
        let channel = Channel { width: 320, height: 240, jpeg_quality: None, ..Channel::default() };
        let a = render(&canvas, &channel, 11).unwrap();
        let b = render(&canvas, &channel, 11).unwrap();
        assert_eq!(a.image, b.image);
        let c = render(&canvas, &channel, 12).unwrap();
        assert_ne!(a.image, c.image);
    }

    #[test]
    fn random_symbols_fill_their_size() {
        let mut rng = Rng::new(5);
        for (w, h, level) in [(24, 24, 0), (100, 64, 1), (48, 96, 0)] {
            let (symbol, content) = random_symbol(w, h, level, &mut rng).unwrap();
            assert_eq!((symbol.width(), symbol.height(), symbol.level()), (w, h, level));
            assert!(content.len() + 16 >= nmtcode::capacity(w, h, level).unwrap());
        }
    }
}
