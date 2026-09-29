//! Finder patterns (specification 5.3): a dark 5 × 5 square with a light border, found as a
//! dark region of the thresholded image, measured to a fraction of a pixel on its four edges,
//! and classified by its inner 3 × 3 against the four patterns under the eight rotations and
//! mirror images.
//!
//! Each candidate has a *local frame*: coordinates (a, b) in [0, 5]² with its four corners at
//! (0, 0), (5, 0), (5, 5) and (0, 5) in increasing angle in the image, so the local frame is
//! never mirrored against the image. A *transform* t (0 to 7) says how the finder's own
//! coordinates (s, t) of the upright symbol (5.3.1) lie in the local frame: finder corner m
//! (0 = (0, 0), 1 = (5, 0), 2 = (5, 5), 3 = (0, 5)) sits at local corner (r + σ · m) mod 4,
//! with r = t mod 4 and σ = +1 for t < 4 and −1 otherwise. σ = −1 is a mirrored symbol.

use std::sync::OnceLock;

use nmtcode_symbol::Corner;

use crate::LumaImage;
use crate::binarize::Thresholds;
use crate::blobs::Blob;
use crate::geom::{Homography, Line, Point, signed_area2};
use crate::num::{count_f64, floor_i32, phi};

/// Finder kinds, in the order of `Corner::ALL`.
pub(crate) const TL: usize = 0;
pub(crate) const TR: usize = 1;
pub(crate) const BL: usize = 2;
pub(crate) const BR: usize = 3;

/// The corners of the local frame.
pub(crate) const LOCAL: [Point; 4] =
    [Point::new(0.0, 0.0), Point::new(5.0, 0.0), Point::new(5.0, 5.0), Point::new(0.0, 5.0)];

/// Blur levels of the classification templates, in modules (standard deviation of a Gaussian).
const SIGMAS: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

/// Largest sum of squared differences of a finder's inner 3 × 3, in units of the contrast
/// between ring and border, that still counts as a finder of some kind.
pub(crate) const SSD_ACCEPT: f64 = 1.2;
/// Smallest finder pitch in pixels: below it the modules are not resolved (specification 1.2).
const MIN_PITCH: f64 = 1.25;

/// Finder corner m (0 = (0, 0), 1 = (5, 0), 2 = (5, 5), 3 = (0, 5)) of the symbol's outer
/// corner at a finder of `kind`.
pub(crate) const fn outer_corner(kind: usize) -> usize {
    match kind {
        TL => 0,
        TR => 1,
        BL => 3,
        _ => 2,
    }
}

/// The local corner of finder corner `m` under transform `t`.
pub(crate) const fn local_corner(t: usize, m: usize) -> usize {
    let r = t % 4;
    if t < 4 { (r + m) % 4 } else { (r + 4 - m % 4) % 4 }
}

/// The transform with rotation `r` and orientation `sigma` (+1 or −1).
pub(crate) const fn transform(r: usize, sigma: i32) -> usize {
    if sigma >= 0 { r % 4 } else { 4 + r % 4 }
}

/// The local point of finder point (`s`, `u`) under transform `t`: the bilinear map of the
/// corners, which is exact for the eight transforms of the square.
fn map_point(t: usize, s: f64, u: f64) -> Point {
    let (al, be) = (s / 5.0, u / 5.0);
    let c = |m: usize| LOCAL.get(local_corner(t, m)).copied().unwrap_or_default();
    c(0).scale((1.0 - al) * (1.0 - be))
        .add(c(1).scale(al * (1.0 - be)))
        .add(c(2).scale(al * be))
        .add(c(3).scale((1.0 - al) * be))
}

/// `perm[q]`: the local inner cell (index p + 3q of the local 3 × 3) that holds finder inner
/// cell q under transform `t`.
fn inner_perm(t: usize) -> [usize; 9] {
    let mut out = [0usize; 9];
    for (q, slot) in out.iter_mut().enumerate() {
        let (p, r) = (q % 3, q / 3);
        let local = map_point(t, 1.5 + count_f64(p), 1.5 + count_f64(r));
        let lp = usize::try_from(floor_i32(local.x - 1.0).clamp(0, 2)).unwrap_or(0);
        let lq = usize::try_from(floor_i32(local.y - 1.0).clamp(0, 2)).unwrap_or(0);
        *slot = lq * 3 + lp;
    }
    out
}

/// Classification tables: the inner permutations of the eight transforms and, per kind and
/// blur level, the normalised values of the nine inner cells.
struct Tables {
    perm: [[usize; 9]; 8],
    templates: [[[f64; 9]; SIGMAS.len()]; 4],
    /// The mean of the ring cells and of the border cells of each template before
    /// normalisation, with dark = 0 and light = 1.
    levels: [[(f64, f64); SIGMAS.len()]; 4],
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut perm = [[0usize; 9]; 8];
        for (t, p) in perm.iter_mut().enumerate() {
            *p = inner_perm(t);
        }
        let mut templates = [[[0.0; 9]; SIGMAS.len()]; 4];
        let mut levels = [[(0.0, 1.0); SIGMAS.len()]; 4];
        for kind in 0..4 {
            for s in 0..SIGMAS.len() {
                let (t, l) = template(kind, SIGMAS.get(s).copied().unwrap_or(0.0));
                if let Some(slot) = templates.get_mut(kind).and_then(|k| k.get_mut(s)) {
                    *slot = t;
                }
                if let Some(slot) = levels.get_mut(kind).and_then(|k| k.get_mut(s)) {
                    *slot = l;
                }
            }
        }
        Tables { perm, templates, levels }
    })
}

/// The scene around a finder of `kind` in its own coordinates: cell (i, j) with −4 ≤ i, j < 9;
/// 0 dark, 1 light. Beyond the light border, the quiet zone on the finder's two outer sides is
/// light and the data on its two inner sides is taken as 0.5.
fn scene(kind: usize, i: i32, j: i32) -> f64 {
    let corner = Corner::ALL.get(kind).copied().unwrap_or(Corner::TopLeft);
    if (0..5).contains(&i) && (0..5).contains(&j) {
        let (Ok(x), Ok(y)) = (u32::try_from(i), u32::try_from(j)) else { return 1.0 };
        return if corner.module(x, y) == Some(true) { 0.0 } else { 1.0 };
    }
    if (-1..6).contains(&i) && (-1..6).contains(&j) {
        return 1.0;
    }
    let outer = match kind {
        TL => i <= -2 || j <= -2,
        TR => i >= 6 || j <= -2,
        BL => i <= -2 || j >= 6,
        _ => i >= 6 || j >= 6,
    };
    if outer { 1.0 } else { 0.5 }
}

/// The scene of `kind` blurred by a Gaussian of `sigma` modules, at the centre of cell (i, j).
fn blurred(kind: usize, sigma: f64, i: i32, j: i32) -> f64 {
    if sigma <= 1e-9 {
        return scene(kind, i, j);
    }
    let (x, y) = (f64::from(i) + 0.5, f64::from(j) + 0.5);
    let weight =
        |c: i32, p: f64| phi((f64::from(c) + 1.0 - p) / sigma) - phi((f64::from(c) - p) / sigma);
    let (mut sum, mut total) = (0.0, 0.0);
    for cj in -4..9 {
        let wy = weight(cj, y);
        for ci in -4..9 {
            let w = weight(ci, x) * wy;
            sum += w * scene(kind, ci, cj);
            total += w;
        }
    }
    if total > 0.0 { sum / total } else { 0.5 }
}

/// The nine inner cells of `kind` at blur `sigma`, normalised as a reader normalises a
/// capture: 0 at the mean of the ring cells and 1 at the mean of the border cells; and those
/// two means.
fn template(kind: usize, sigma: f64) -> ([f64; 9], (f64, f64)) {
    let mut cells = [0.0f64; 49];
    for j in -1i32..6 {
        for i in -1i32..6 {
            if let Some(slot) = cell_index(i, j).and_then(|k| cells.get_mut(k)) {
                *slot = blurred(kind, sigma, i, j);
            }
        }
    }
    let (ring, border) = levels(&cells);
    let contrast = (border - ring).max(1e-6);
    let mut out = [0.0; 9];
    for (q, slot) in out.iter_mut().enumerate() {
        let (p, r) = (i32::try_from(q % 3).unwrap_or(0), i32::try_from(q / 3).unwrap_or(0));
        let v = cell_index(1 + p, 1 + r).and_then(|k| cells.get(k)).copied().unwrap_or(0.0);
        *slot = (v - ring) / contrast;
    }
    (out, (ring, border))
}

/// Index of cell (i, j), −1 ≤ i, j ≤ 5, in a 7 × 7 array.
fn cell_index(i: i32, j: i32) -> Option<usize> {
    if !(-1..=5).contains(&i) || !(-1..=5).contains(&j) {
        return None;
    }
    usize::try_from((j + 1) * 7 + (i + 1)).ok()
}

/// Mean of the 16 ring cells and of the 24 border cells of a 7 × 7 cell array.
fn levels(cells: &[f64; 49]) -> (f64, f64) {
    let (mut ring, mut border) = (0.0, 0.0);
    for j in -1i32..6 {
        for i in -1i32..6 {
            let v = cell_index(i, j).and_then(|k| cells.get(k)).copied().unwrap_or(0.0);
            if i == -1 || j == -1 || i == 5 || j == 5 {
                border += v;
            } else if i == 0 || j == 0 || i == 4 || j == 4 {
                ring += v;
            }
        }
    }
    (ring / 16.0, border / 24.0)
}

/// A finder candidate measured in the image.
#[derive(Clone, Debug)]
pub(crate) struct Finder {
    /// Corners in local order (increasing angle in the image).
    pub corners: [Point; 4],
    /// The local frame [0, 5]² → image.
    pub local: Homography,
    /// Mean side in pixels divided by 5.
    pub pitch: f64,
    /// Mean of the ring cells, in linear light (0 to 255).
    pub ring: f64,
    /// Mean of the border cells, in linear light.
    pub border: f64,
    /// The dark and light plateaus in linear light, from the ring and border means and the
    /// template of the blur that fits best. Blur pulls the one-module ring and border toward
    /// each other, so their midpoint, not the midpoint of the ring and border means, splits
    /// dark from light without bias.
    pub dark: f64,
    /// See [`Finder::dark`].
    pub light: f64,
    /// Sum of squared differences of the inner 3 × 3 against each kind under each transform,
    /// the smallest over the blur levels, in units of the contrast.
    pub ssd: [[f64; 8]; 4],
    /// `quiet[s]`: side s (local corner s to s + 1) has light beyond the border as well, 1.5
    /// modules out, as the quiet zone gives the two outer sides of a finder (5.2).
    pub quiet: [bool; 4],
}

impl Finder {
    /// The smallest [`Finder::ssd`] of `kind` over the eight transforms.
    pub(crate) fn kind_score(&self, kind: usize) -> f64 {
        self.ssd
            .get(kind)
            .map_or(f64::INFINITY, |row| row.iter().copied().fold(f64::INFINITY, f64::min))
    }

    /// The kind with the smallest score and that score.
    pub(crate) fn best_kind(&self) -> (usize, f64) {
        (0..4)
            .map(|k| (k, self.kind_score(k)))
            .fold((0, f64::INFINITY), |best, c| if c.1 < best.1 { c } else { best })
    }

    /// Corner `i` (mod 4).
    pub(crate) fn corner(&self, i: usize) -> Point {
        self.corners.get(i % 4).copied().unwrap_or_default()
    }

    /// The line of side `s`, from corner s to corner s + 1.
    pub(crate) fn side_line(&self, s: usize) -> Option<Line> {
        Line::through(self.corner(s), self.corner(s + 1))
    }

    /// Length of side `s` in pixels.
    pub(crate) fn side_len(&self, s: usize) -> f64 {
        self.corner(s).dist(self.corner(s + 1))
    }

    /// The finder's centre in the image.
    pub(crate) fn centre(&self) -> Point {
        self.local.apply(Point::new(2.5, 2.5))
    }

    /// The stored value that splits dark from light around the finder: the midpoint of the
    /// plateaus in linear light.
    pub(crate) fn threshold(&self) -> f64 {
        crate::light::stored(f64::midpoint(self.dark, self.light))
    }

    /// Whether side `s` faces a quiet zone.
    pub(crate) fn quiet_side(&self, s: usize) -> bool {
        self.quiet.get(s % 4).copied().unwrap_or(false)
    }

    /// Whether both sides at local corner `j` face a quiet zone: the finder can sit at a
    /// symbol corner there.
    pub(crate) fn quiet_corner(&self, j: usize) -> bool {
        self.quiet_side(j + 3) && self.quiet_side(j)
    }
}

/// Samples the 7 × 7 cell centres of a local frame, in linear light.
fn sample_cells(image: &LumaImage, local: &Homography) -> [f64; 49] {
    let mut cells = [0.0f64; 49];
    for j in -1i32..6 {
        for i in -1i32..6 {
            let p = local.apply(Point::new(f64::from(i) + 0.5, f64::from(j) + 0.5));
            if let Some(slot) = cell_index(i, j).and_then(|k| cells.get_mut(k)) {
                *slot = crate::light::linear(image.sample(p));
            }
        }
    }
    cells
}

/// True when the border cells are light and the ring cells dark: at least `min_border` of the
/// 24 border cells above the midpoint of the two levels and at least `min_ring` of the 16 ring
/// cells below it.
fn frame_ok(cells: &[f64; 49], ring: f64, border: f64, limits: (i32, i32)) -> bool {
    let (light, dark) = frame_counts(cells, ring, border);
    light >= limits.0 && dark >= limits.1
}

/// The numbers of light border cells and dark ring cells of [`frame_ok`].
fn frame_counts(cells: &[f64; 49], ring: f64, border: f64) -> (i32, i32) {
    let mid = f64::midpoint(ring, border);
    let (mut light_border, mut dark_ring) = (0, 0);
    for j in -1i32..6 {
        for i in -1i32..6 {
            let v = cell_index(i, j).and_then(|k| cells.get(k)).copied().unwrap_or(0.0);
            if i == -1 || j == -1 || i == 5 || j == 5 {
                light_border += i32::from(v > mid);
            } else if (i == 0 || j == 0 || i == 4 || j == 4) && v < mid {
                dark_ring += 1;
            }
        }
    }
    (light_border, dark_ring)
}

/// Sub-pixel edges: on each side, the dark-to-light crossings of `threshold` along eight short
/// profiles across the edge, fitted with a line; the corners are the intersections of adjacent
/// lines. `None` when a side has fewer than four crossings or a corner moves more than a module.
fn refine(image: &LumaImage, local: &Homography, threshold: f64, pitch: f64) -> Option<[Point; 4]> {
    const NORMALS: [Point; 4] =
        [Point::new(0.0, -1.0), Point::new(1.0, 0.0), Point::new(0.0, 1.0), Point::new(-1.0, 0.0)];
    const STEPS: usize = 15;
    let mut lines: Vec<Line> = Vec::with_capacity(4);
    for side in 0..4 {
        let a = LOCAL.get(side).copied()?;
        let b = LOCAL.get((side + 1) % 4).copied()?;
        let n = NORMALS.get(side).copied()?;
        let mut points = Vec::with_capacity(8);
        for k in 0..8 {
            let tau = 0.75 + 0.5 * f64::from(k);
            let base = a.lerp(b, tau / 5.0);
            let mut profile = [0.0f64; STEPS];
            for (i, v) in profile.iter_mut().enumerate() {
                let d = -0.7 + 0.1 * count_f64(i);
                *v = image.sample(local.apply(base.add(n.scale(d))));
            }
            let mut best: Option<f64> = None;
            for i in 0..STEPS - 1 {
                let (v0, v1) = (profile.get(i).copied()?, profile.get(i + 1).copied()?);
                if v0 < threshold && v1 >= threshold && v1 > v0 {
                    let d = -0.7 + 0.1 * (count_f64(i) + (threshold - v0) / (v1 - v0));
                    if best.is_none_or(|b| d.abs() < b.abs()) {
                        best = Some(d);
                    }
                }
            }
            if let Some(d) = best {
                points.push(local.apply(base.add(n.scale(d))));
            }
        }
        if points.len() < 4 {
            return None;
        }
        let mut line = Line::fit(&points)?;
        let mut residuals: Vec<f64> = points.iter().map(|p| line.distance(*p).abs()).collect();
        let limit = (2.5 * crate::num::median(&mut residuals.clone())?).max(0.3);
        let kept: Vec<Point> =
            points.iter().copied().filter(|p| line.distance(*p).abs() <= limit).collect();
        if kept.len() >= 4 && kept.len() < points.len() {
            line = Line::fit(&kept)?;
        }
        residuals.clear();
        lines.push(line);
    }
    let mut corners = [Point::default(); 4];
    for (i, slot) in corners.iter_mut().enumerate() {
        let before = lines.get((i + 3) % 4)?;
        let after = lines.get(i)?;
        *slot = before.intersect(after)?;
        let old = local.apply(LOCAL.get(i).copied()?);
        if slot.dist(old) > pitch {
            return None;
        }
    }
    Some(corners)
}

/// Orders four points by increasing angle around their centroid (the local order).
fn order(points: [Point; 4]) -> [Point; 4] {
    let c = points.iter().fold(Point::default(), |s, p| s.add(*p)).scale(0.25);
    let mut sorted = points;
    sorted.sort_by(|a, b| {
        let aa = (a.y - c.y).atan2(a.x - c.x);
        let bb = (b.y - c.y).atan2(b.x - c.x);
        aa.total_cmp(&bb)
    });
    // Start at the corner with the smallest angle from the top-left direction, so that an
    // upright square gets the corners of `LOCAL`; any start gives a valid local frame.
    sorted
}

/// The quadrilateral of largest area through four of a blob's eight extreme pixels, as pixel
/// centres pushed half a pixel outward.
fn blob_quad(blob: &Blob) -> Option<[Point; 4]> {
    let pts: Vec<Point> = blob
        .extremes
        .iter()
        .map(|&(x, y)| Point::new(count_f64(x) + 0.5, count_f64(y) + 0.5))
        .collect();
    let mut best: Option<([Point; 4], f64)> = None;
    for a in 0..8 {
        for b in a + 1..8 {
            for c in b + 1..8 {
                for d in c + 1..8 {
                    let quad = [
                        pts.get(a).copied()?,
                        pts.get(b).copied()?,
                        pts.get(c).copied()?,
                        pts.get(d).copied()?,
                    ];
                    let area = signed_area2(&quad).abs();
                    if best.is_none_or(|(_, s)| area > s) {
                        best = Some((quad, area));
                    }
                }
            }
        }
    }
    let (quad, _) = best?;
    let quad = order(quad);
    let c = quad.iter().fold(Point::default(), |s, p| s.add(*p)).scale(0.25);
    Some(quad.map(|p| {
        let d = p.sub(c);
        let n = d.norm();
        if n > 1e-9 { p.add(d.scale(0.5 / n)) } else { p }
    }))
}

/// Whether the thresholded image is dark at `p`.
fn dark_at(image: &LumaImage, thresholds: &Thresholds, p: Point) -> bool {
    let (x, y) = (floor_i32(p.x), floor_i32(p.y));
    if x < 0 || y < 0 || x >= image.w() || y >= image.h() {
        return false;
    }
    let (Ok(xu), Ok(yu)) = (usize::try_from(x), usize::try_from(y)) else { return false };
    let v = image.pixels.get(yu * image.w_usize() + xu).copied().unwrap_or(255);
    v < thresholds.at(xu, yu)
}

/// Bilinear map of the local frame onto `quad`, without perspective: the cheap first test.
fn affine_point(quad: &[Point; 4], a: f64, b: f64) -> Point {
    let (al, be) = (a / 5.0, b / 5.0);
    quad[0]
        .scale((1.0 - al) * (1.0 - be))
        .add(quad[1].scale(al * (1.0 - be)))
        .add(quad[2].scale(al * be))
        .add(quad[3].scale((1.0 - al) * be))
}

/// The starting quadrilaterals of the refinement: `quad`; `quad` moved by half a module along
/// each of its axes; `quad` with one side pushed out by 1.5 modules, for each side; and `quad`
/// grown and shrunk by half a module on every side.
fn starts(quad: &[Point; 4], pitch: f64) -> Vec<[Point; 4]> {
    let c = quad.iter().fold(Point::default(), |s, p| s.add(*p)).scale(0.25);
    let mut out = vec![*quad];
    let unit = |d: Point| {
        let n = d.norm();
        if n > 1e-9 { d.scale(1.0 / n) } else { Point::default() }
    };
    let ax = unit(quad[1].sub(quad[0]).add(quad[2].sub(quad[3])));
    let ay = unit(quad[3].sub(quad[0]).add(quad[2].sub(quad[1])));
    for d in [ax, ax.scale(-1.0), ay, ay.scale(-1.0)] {
        out.push(quad.map(|p| p.add(d.scale(0.5 * pitch))));
    }
    for side in 0..4 {
        let (a, b) = (quad[side], quad[(side + 1) % 4]);
        let shift = unit(a.lerp(b, 0.5).sub(c)).scale(1.5 * pitch);
        let mut q = *quad;
        q[side] = a.add(shift);
        q[(side + 1) % 4] = b.add(shift);
        out.push(q);
    }
    for grow in [0.5, -0.5] {
        out.push(
            quad.map(|p| p.add(unit(p.sub(c)).scale(grow * pitch * core::f64::consts::SQRT_2))),
        );
    }
    out
}

/// Refines the finder outline from `start`: a loose frame test, two rounds of sub-pixel edges
/// at the midpoint of the dark and light plateaus, and the corners with their local frame.
fn refine_from(
    image: &LumaImage,
    start: &[Point; 4],
    pitch0: f64,
    min_contrast: f64,
) -> Option<([Point; 4], Homography)> {
    let mut local = Homography::from_square(start, 5.0)?;
    let cells = sample_cells(image, &local);
    let (ring, border) = levels(&cells);
    // The outline of a blurred square is rounded at its corners, so the first frame, from the
    // blob's outline, is only roughly placed: the test is loose here and strict in `finish`.
    let stored_contrast = crate::light::stored(border) - crate::light::stored(ring);
    if stored_contrast < min_contrast || !frame_ok(&cells, ring, border, (16, 11)) {
        return None;
    }
    let (_, fit) = classify(&inner_z(&cells, ring, border - ring));
    let (mut dark, mut light) = plateaus(ring, border, fit);
    let mut corners = *start;
    for _ in 0..2 {
        corners = refine(image, &local, crate::light::stored(f64::midpoint(dark, light)), pitch0)?;
        local = Homography::from_square(&corners, 5.0)?;
        let cells = sample_cells(image, &local);
        let (r, b) = levels(&cells);
        let (_, fit) = classify(&inner_z(&cells, r, b - r));
        (dark, light) = plateaus(r, b, fit);
    }
    Some((corners, local))
}

/// Border points of the cheap test on the thresholded image, in the local frame.
const QUICK_BORDER: [(f64, f64); 8] = [
    (-0.5, -0.5),
    (2.5, -0.5),
    (5.5, -0.5),
    (5.5, 2.5),
    (5.5, 5.5),
    (2.5, 5.5),
    (-0.5, 5.5),
    (-0.5, 2.5),
];
/// Ring points of the cheap test.
const QUICK_RING: [(f64, f64); 4] = [(0.5, 2.5), (2.5, 0.5), (4.5, 2.5), (2.5, 4.5)];
/// Quiet-zone points of the cheap test, per side: 1.5 modules out.
const QUICK_QUIET: [[(f64, f64); 3]; 4] = [
    [(0.5, -1.5), (2.5, -1.5), (4.5, -1.5)],
    [(6.5, 0.5), (6.5, 2.5), (6.5, 4.5)],
    [(0.5, 6.5), (2.5, 6.5), (4.5, 6.5)],
    [(-1.5, 0.5), (-1.5, 2.5), (-1.5, 4.5)],
];

/// The finder candidate of `blob` with the ends of its spans, or `None` when the blob is not a
/// dark square with a light border whose inner 3 × 3 looks like one of the four finders.
pub(crate) fn candidate(
    image: &LumaImage,
    thresholds: &Thresholds,
    blob: &Blob,
    spans: &[(usize, usize, usize)],
) -> Option<Finder> {
    // A finder is at most a third of the image's shorter side (a 20-module symbol filling the
    // frame), at most half along a diagonal; larger or elongated regions are background.
    let [bx0, by0, bx1, by1] = blob.bbox;
    let limit = image.w_usize().min(image.h_usize()) / 2 + 8;
    let (bw, bh) = (bx1 - bx0 + 1, by1 - by0 + 1);
    if bw > limit || bh > limit || bw > 5 * bh || bh > 5 * bw {
        return None;
    }
    // The smallest rectangle around the region's convex hull: a ring broken by blur still has
    // the whole square as its hull, and rounded corners do not move the straight sides.
    let quad = match crate::blobs::outline_rectangle(spans) {
        Some(q) => q,
        None => blob_quad(blob)?,
    };
    let area2 = signed_area2(&quad);
    if area2 < 2.0 * 16.0 {
        return None;
    }
    let sides = [0usize, 1, 2, 3].map(|i| quad[i].dist(quad[(i + 1) % 4]));
    let (min_side, max_side) =
        sides.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    if min_side < 4.0 || max_side > 4.0 * min_side || crate::num::u64_f64(blob.area) < 0.04 * area2
    {
        return None;
    }
    // Convexity: every turn has the same sign as the whole.
    if (0..4).any(|i| {
        quad[(i + 1) % 4].sub(quad[i]).cross(quad[(i + 2) % 4].sub(quad[(i + 1) % 4])) <= 0.0
    }) {
        return None;
    }
    // Cheap tests on the thresholded image: the border just outside is light, the ring just
    // inside is dark, and two adjacent sides face a quiet zone, light 1.5 modules out (5.2).
    let light_at = |&(a, b): &(f64, f64)| !dark_at(image, thresholds, affine_point(&quad, a, b));
    let light = QUICK_BORDER.iter().filter(|p| light_at(p)).count();
    let dark = QUICK_RING.iter().filter(|p| !light_at(p)).count();
    if light < 6 || dark < 2 {
        return None;
    }
    let quiet = QUICK_QUIET.map(|side| side.iter().filter(|p| light_at(p)).count() >= 2);
    if !(0..4).any(|s| quiet[s] && quiet[(s + 1) % 4]) {
        return None;
    }
    let pitch0 = sides.iter().sum::<f64>() / 20.0;
    let min_contrast = (3.0 * thresholds.noise).max(8.0);
    // A blurred one-module ring can break, so a blob may be only part of the finder and its
    // outline a side short: the refinement starts from the blob's outline and, when that fails
    // and the outline is roughly a finder frame (half the border light, half the ring dark),
    // from moved and grown copies of it.
    let all = starts(&quad, pitch0);
    let local = Homography::from_square(&quad, 5.0)?;
    let cells = sample_cells(image, &local);
    let (ring, border) = levels(&cells);
    let (light_cells, dark_cells) = frame_counts(&cells, ring, border);
    let tries = if light_cells >= 12 && dark_cells >= 8 { all.len() } else { 1 };
    all.iter().take(tries).find_map(|start| {
        let (corners, local) = refine_from(image, start, pitch0, min_contrast)?;
        finish(image, corners, local, min_contrast)
    })
}

/// The strict tests on a refined outline, and the finder.
fn finish(
    image: &LumaImage,
    corners: [Point; 4],
    local: Homography,
    min_contrast: f64,
) -> Option<Finder> {
    if signed_area2(&corners) <= 0.0 {
        return None;
    }
    let cells = sample_cells(image, &local);
    let (ring, border) = levels(&cells);
    let contrast = border - ring;
    let stored_contrast = crate::light::stored(border) - crate::light::stored(ring);
    if stored_contrast < min_contrast
        || contrast <= 0.0
        || !frame_ok(&cells, ring, border, (20, 13))
    {
        return None;
    }
    // A blob that is only part of a finder whose ring broke up under blur has pieces of the
    // ring in its border: a border cell nearly as dark as the ring. At most one border cell
    // may lie within a quarter of the contrast of the ring level.
    let dark_border = (-1i32..6)
        .flat_map(|j| (-1i32..6).map(move |i| (i, j)))
        .filter(|&(i, j)| {
            (i == -1 || j == -1 || i == 5 || j == 5)
                && cell_index(i, j)
                    .and_then(|k| cells.get(k))
                    .is_some_and(|&v| v < ring + 0.25 * contrast)
        })
        .count();
    if dark_border > 1 {
        return None;
    }
    let (ssd, fit) = classify(&inner_z(&cells, ring, contrast));
    let (dark, light) = plateaus(ring, border, fit);
    let quiet = quiet_corners(image, &local, crate::light::stored(f64::midpoint(dark, light)));
    if !(0..4).any(|j| quiet[(j + 3) % 4] && quiet[j]) {
        return None;
    }
    let finder = Finder {
        pitch: (0..4).map(|s| corners[s].dist(corners[(s + 1) % 4])).sum::<f64>() / 20.0,
        corners,
        local,
        ring,
        border,
        dark,
        light,
        ssd,
        quiet,
    };
    (finder.best_kind().1 <= SSD_ACCEPT && finder.pitch >= MIN_PITCH).then_some(finder)
}

/// A finder searched where a symbol's other finders predict it: `predicted` holds the four
/// corners in any cyclic order. The outline is refined on the grey image from the prediction
/// (and from shifted copies of it), without the thresholded image, so a finder whose ring
/// broke up under blur is still found. `noise` is the image's noise estimate.
pub(crate) fn guided(image: &LumaImage, noise: f64, predicted: &[Point; 4]) -> Option<Finder> {
    let quad = order(*predicted);
    if signed_area2(&quad) <= 0.0 {
        return None;
    }
    let pitch = (0..4).map(|i| quad[i].dist(quad[(i + 1) % 4])).sum::<f64>() / 20.0;
    if !(pitch >= MIN_PITCH && pitch.is_finite()) {
        return None;
    }
    let min_contrast = (3.0 * noise).max(8.0);
    // The prediction can be off by a module or two: the start moves over a grid of half
    // modules around it, nearest first.
    let unit = |d: Point| {
        let n = d.norm();
        if n > 1e-9 { d.scale(1.0 / n) } else { Point::default() }
    };
    let ax = unit(quad[1].sub(quad[0]).add(quad[2].sub(quad[3])));
    let ay = unit(quad[3].sub(quad[0]).add(quad[2].sub(quad[1])));
    let mut shifts: Vec<(f64, f64)> = Vec::new();
    for i in -4i32..=4 {
        for j in -4i32..=4 {
            shifts.push((0.5 * f64::from(i), 0.5 * f64::from(j)));
        }
    }
    shifts.sort_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)));
    shifts.iter().find_map(|&(dx, dy)| {
        let d = ax.scale(dx * pitch).add(ay.scale(dy * pitch));
        let start = quad.map(|p| p.add(d));
        let (corners, local) = refine_from(image, &start, pitch, min_contrast)?;
        finish(image, corners, local, min_contrast)
    })
}

/// For each local corner, whether both sides meeting there have light cells in the second ring
/// outside the finder (cell centres 1.5 modules out): at least 6 of the 7 cells along each side.
fn quiet_corners(image: &LumaImage, local: &Homography, threshold: f64) -> [bool; 4] {
    // Side s runs from local corner s to s + 1: 0 top (b = −1.5), 1 right (a = 6.5),
    // 2 bottom (b = 6.5), 3 left (a = −1.5).
    let mut light = [false; 4];
    for (s, slot) in light.iter_mut().enumerate() {
        let count = (-1i32..6)
            .filter(|&k| {
                let t = f64::from(k) + 0.5;
                let p = match s {
                    0 => Point::new(t, -1.5),
                    1 => Point::new(6.5, t),
                    2 => Point::new(t, 6.5),
                    _ => Point::new(-1.5, t),
                };
                image.sample(local.apply(p)) > threshold
            })
            .count();
        *slot = count >= 6;
    }
    light
}

/// The score table of a normalised inner 3 × 3 in local order, and the ring and border means,
/// in units of the dark and light plateaus, of the template that fits best.
fn classify(z: &[f64; 9]) -> ([[f64; 8]; 4], (f64, f64)) {
    let tables = tables();
    let mut out = [[f64::INFINITY; 8]; 4];
    let mut best = (f64::INFINITY, (0.0, 1.0));
    for (kind, row) in out.iter_mut().enumerate() {
        for (t, slot) in row.iter_mut().enumerate() {
            let Some(perm) = tables.perm.get(t) else { continue };
            let Some(per_sigma) = tables.templates.get(kind) else { continue };
            for (s, template) in per_sigma.iter().enumerate() {
                let ssd: f64 = perm
                    .iter()
                    .zip(template.iter())
                    .map(|(&local, &expected)| {
                        let d = z.get(local).copied().unwrap_or(0.0) - expected;
                        d * d
                    })
                    .sum();
                *slot = slot.min(ssd);
                if ssd < best.0 {
                    let levels = tables
                        .levels
                        .get(kind)
                        .and_then(|k| k.get(s))
                        .copied()
                        .unwrap_or((0.0, 1.0));
                    best = (ssd, levels);
                }
            }
        }
    }
    (out, best.1)
}

/// The dark and light plateaus from the measured ring and border means and the template's
/// (`rt`, `bt`): ring = D + (L − D) · rt and border = D + (L − D) · bt.
fn plateaus(ring: f64, border: f64, (rt, bt): (f64, f64)) -> (f64, f64) {
    let span = bt - rt;
    if span < 0.2 {
        return (ring, border);
    }
    let contrast = (border - ring) / span;
    let dark = ring - contrast * rt;
    (dark, dark + contrast)
}

/// Normalised inner 3 × 3 of a cell array.
fn inner_z(cells: &[f64; 49], ring: f64, contrast: f64) -> [f64; 9] {
    let mut z = [0.0f64; 9];
    for (q, slot) in z.iter_mut().enumerate() {
        let (p, r) = (i32::try_from(q % 3).unwrap_or(0), i32::try_from(q / 3).unwrap_or(0));
        let v = cell_index(1 + p, 1 + r).and_then(|k| cells.get(k)).copied().unwrap_or(0.0);
        *slot = (v - ring) / contrast.max(1e-9);
    }
    z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern_z(kind: usize, t: usize) -> [f64; 9] {
        // The inner cells of `kind` as a sharp capture seen through transform t.
        let perm = inner_perm(t);
        let corner = Corner::ALL[kind];
        let mut z = [0.0; 9];
        for ((p, r), &local) in
            (0u32..3).flat_map(|r| (0u32..3).map(move |p| (p, r))).zip(perm.iter())
        {
            let dark = corner.module(1 + p, 1 + r) == Some(true);
            z[local] = if dark { 0.0 } else { 1.0 };
        }
        z
    }

    #[test]
    fn transforms_are_the_eight_symmetries() {
        let mut seen = std::collections::BTreeSet::new();
        for t in 0..8 {
            let corners: Vec<usize> = (0..4).map(|m| local_corner(t, m)).collect();
            seen.insert(corners);
            let mut perm = inner_perm(t);
            perm.sort_unstable();
            assert_eq!(perm, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(transform(3, -1), 7);
    }

    #[test]
    fn sharp_patterns_classify_as_themselves() {
        for kind in 0..4 {
            for t in 0..8 {
                let (ssd, _) = classify(&pattern_z(kind, t));
                assert!(ssd[kind][t] < 1e-9, "{kind} {t}");
                for (other, row) in ssd.iter().enumerate() {
                    if other != kind {
                        let best = row.iter().copied().fold(f64::INFINITY, f64::min);
                        // 5.3.2 property 1: at least 4 modules apart, blur only narrows this.
                        assert!(best > 0.9, "{kind} as {other} under {t}: {best}");
                    }
                }
            }
        }
    }

    #[test]
    fn templates_fade_with_blur() {
        let sharp = template(TR, 0.0).0;
        let soft = template(TR, 0.6).0;
        // A light slot of TR (finder cell (2, 1), inner cell 1) is darker under blur.
        assert!((sharp[1] - 1.0).abs() < 1e-9);
        assert!(soft[1] < 0.8 && soft[1] > 0.1, "{}", soft[1]);
    }
}
