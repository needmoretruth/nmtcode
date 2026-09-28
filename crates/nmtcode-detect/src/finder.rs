//! Finder patterns (specification 5.3): connected dark regions, then classification of each
//! candidate by its 7 × 7 cells.

use nmtcode_symbol::Corner;

use crate::LumaImage;
use crate::num::{count_f64, floor_i32};

/// Finder kinds, indexed by corner in the order of `Corner::ALL`.
pub(crate) const TL: usize = 0;
pub(crate) const TR: usize = 1;
pub(crate) const BL: usize = 2;
pub(crate) const BR: usize = 3;

/// Module (`x`, `y`) of the 5 × 5 finder `kind` in the upright symbol (5.3.1); `true` = dark.
/// The patterns are those of `nmtcode-symbol`, so the reader and the generator cannot drift
/// apart.
pub(crate) fn finder_module(kind: usize, x: usize, y: usize) -> bool {
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return false;
    };
    Corner::ALL.get(kind).and_then(|corner| corner.module(x, y)).unwrap_or(false)
}

/// The inner 3 × 3 of finder `kind`, row-major.
fn inner(kind: usize) -> [bool; 9] {
    let mut out = [false; 9];
    for (i, cell) in out.iter_mut().enumerate() {
        *cell = finder_module(kind, 1 + i % 3, 1 + i / 3);
    }
    out
}

/// Symbol corner of finder `kind`: (0, 0) top-left, (1, 0) top-right, (0, 1) bottom-left,
/// (1, 1) bottom-right.
pub(crate) fn corner(kind: usize) -> (u8, u8) {
    match kind {
        TL => (0, 0),
        TR => (1, 0),
        BL => (0, 1),
        _ => (1, 1),
    }
}

/// One of the eight rotations and mirror images, as a map from symbol orientation to image
/// orientation: first swap x and y (bit 2), then mirror x (bit 0), then mirror y (bit 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Transform(pub u8);

impl Transform {
    pub(crate) const ALL: [Self; 8] =
        [Self(0), Self(1), Self(2), Self(3), Self(4), Self(5), Self(6), Self(7)];

    pub(crate) fn swaps(self) -> bool {
        self.0 & 4 != 0
    }

    pub(crate) fn flips_x(self) -> bool {
        self.0 & 1 != 0
    }

    pub(crate) fn flips_y(self) -> bool {
        self.0 & 2 != 0
    }

    /// Image position of symbol position (`x`, `y`) in a square of side `last + 1`.
    pub(crate) fn apply(self, x: u32, y: u32, last_x: u32, last_y: u32) -> (u32, u32) {
        let (a, b) = if self.swaps() { (y, x) } else { (x, y) };
        let a = if self.flips_x() { last_x.saturating_sub(a) } else { a };
        let b = if self.flips_y() { last_y.saturating_sub(b) } else { b };
        (a, b)
    }

    /// Image corner of symbol corner `c`.
    pub(crate) fn corner(self, c: (u8, u8)) -> (u8, u8) {
        let (a, b) = self.apply(u32::from(c.0), u32::from(c.1), 1, 1);
        (u8::from(a != 0), u8::from(b != 0))
    }

    /// The finder kind whose corner lands on image corner `c`.
    pub(crate) fn kind_at(self, c: (u8, u8)) -> usize {
        (0..4).find(|&k| self.corner(corner(k)) == c).unwrap_or(TL)
    }

    /// The inner 3 × 3 of `kind` as it appears in the image.
    fn inner(self, kind: usize) -> [bool; 9] {
        let upright = inner(kind);
        let mut out = [false; 9];
        for (i, &v) in (0u32..).zip(upright.iter()) {
            let (x, y) = self.apply(i % 3, i / 3, 2, 2);
            if let Some(cell) = usize::try_from(y * 3 + x).ok().and_then(|j| out.get_mut(j)) {
                *cell = v;
            }
        }
        out
    }
}

/// Bit of (`kind`, `t`) in [`Finder::matches`].
pub(crate) fn match_bit(kind: usize, t: Transform) -> u32 {
    let k = u32::try_from(kind & 3).unwrap_or(0);
    1u32 << (k * 8 + u32::from(t.0 & 7))
}

/// A classified finder in the image.
#[derive(Clone, Debug)]
pub(crate) struct Finder {
    /// Outer edges in pixels, measured to a fraction of a pixel.
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    /// Midpoint between the ring's and the border's brightness.
    pub threshold: f64,
    /// Brightness of the border minus brightness of the ring.
    pub contrast: f64,
    /// Every (kind, transform) whose inner pattern equals this finder's, as [`match_bit`]s.
    pub matches: u32,
}

impl Finder {
    pub(crate) fn module_x(&self) -> f64 {
        (self.right - self.left) / 5.0
    }

    pub(crate) fn module_y(&self) -> f64 {
        (self.bottom - self.top) / 5.0
    }

    pub(crate) fn module(&self) -> f64 {
        f64::midpoint(self.module_x(), self.module_y())
    }

    pub(crate) fn has(&self, kind: usize, t: Transform) -> bool {
        self.matches & match_bit(kind, t) != 0
    }
}

/// Bounding box of a 4-connected dark region; `x1`, `y1` exclusive.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

/// Largest stack of pending spans in one flood fill. A region that needs more is not a finder;
/// its unvisited pixels are seeds of later fills, so memory stays bounded and every pixel is
/// still visited once.
const MAX_STACK: usize = 1 << 20;

/// Dark pixels of an image and the pixels already assigned to a region.
struct Mask<'a> {
    image: &'a LumaImage,
    width: usize,
    height: usize,
    threshold: f64,
    visited: Vec<u64>,
}

impl Mask<'_> {
    /// Dark and not yet visited.
    fn open(&self, index: usize) -> bool {
        let dark = self.image.pixels.get(index).is_some_and(|&v| f64::from(v) < self.threshold);
        let seen = self.visited.get(index / 64).is_some_and(|word| word >> (index % 64) & 1 == 1);
        dark && !seen
    }

    fn visit(&mut self, index: usize) {
        if let Some(word) = self.visited.get_mut(index / 64) {
            *word |= 1u64 << (index % 64);
        }
    }

    /// Scanline flood fill of the open region containing (`sx`, `sy`). Returns its bounding box
    /// (inclusive) and pixel count, or `None` when the fill needed more than [`MAX_STACK`]
    /// pending spans.
    fn fill(
        &mut self,
        sx: usize,
        sy: usize,
        stack: &mut Vec<(usize, usize)>,
    ) -> Option<([usize; 4], u64)> {
        let mut bounds = [sx, sy, sx, sy];
        let mut count: u64 = 0;
        let mut overflow = false;
        stack.clear();
        stack.push((sx, sy));
        while let Some((px, py)) = stack.pop() {
            let row = py * self.width;
            if !self.open(row + px) {
                continue;
            }
            let mut left = px;
            while left > 0 && self.open(row + left - 1) {
                left -= 1;
            }
            let mut right = px;
            while right + 1 < self.width && self.open(row + right + 1) {
                right += 1;
            }
            for index in row + left..=row + right {
                self.visit(index);
            }
            count += u64::try_from(right - left + 1).unwrap_or(u64::MAX);
            bounds =
                [bounds[0].min(left), bounds[1].min(py), bounds[2].max(right), bounds[3].max(py)];
            for ny in [py.wrapping_sub(1), py + 1] {
                if ny >= self.height {
                    continue;
                }
                let next_row = ny * self.width;
                let mut cx = left;
                while cx <= right {
                    if !self.open(next_row + cx) {
                        cx += 1;
                        continue;
                    }
                    if stack.len() < MAX_STACK {
                        stack.push((cx, ny));
                    } else {
                        overflow = true;
                    }
                    while cx <= right && self.open(next_row + cx) {
                        cx += 1;
                    }
                }
            }
        }
        (!overflow).then_some((bounds, count))
    }
}

/// Every 4-connected region of pixels darker than `threshold` whose bounding box could hold a
/// finder: both sides at least 7 pixels, aspect ratio from 0.8 to 1.25, and at least 60% of the
/// box dark.
pub(crate) fn regions(image: &LumaImage, threshold: f64) -> Vec<Region> {
    let (Ok(width), Ok(height)) = (usize::try_from(image.width), usize::try_from(image.height))
    else {
        return Vec::new();
    };
    let mut mask = Mask {
        image,
        width,
        height,
        threshold: threshold.clamp(0.0, 256.0),
        visited: vec![0u64; (width * height).div_ceil(64)],
    };
    let mut out = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for sy in 0..height {
        for sx in 0..width {
            if !mask.open(sy * width + sx) {
                continue;
            }
            let Some(([bx0, by0, bx1, by1], count)) = mask.fill(sx, sy, &mut stack) else {
                continue;
            };
            let (bw, bh) = (bx1 - bx0 + 1, by1 - by0 + 1);
            let area = u64::try_from(bw * bh).unwrap_or(u64::MAX);
            let plausible =
                bw >= 7 && bh >= 7 && 4 * bw <= 5 * bh && 4 * bh <= 5 * bw && 5 * count >= 3 * area;
            if !plausible {
                continue;
            }
            if let (Ok(x0), Ok(y0), Ok(x1), Ok(y1)) = (
                i32::try_from(bx0),
                i32::try_from(by0),
                i32::try_from(bx1 + 1),
                i32::try_from(by1 + 1),
            ) {
                out.push(Region { x0, y0, x1, y1 });
            }
        }
    }
    out
}

/// First crossing of `threshold` between consecutive pixels `p` and `p + step`, for `p` from
/// `start` over `count` steps, at a fraction of a pixel. Pixel p's value sits at p + 0.5.
fn crossing(
    value: impl Fn(i32) -> f64,
    start: i32,
    step: i32,
    count: i32,
    threshold: f64,
) -> Option<f64> {
    let mut here = start;
    for _ in 0..count {
        let next = here.saturating_add(step);
        let (from, to) = (value(here), value(next));
        if (from < threshold) != (to < threshold) && (to - from).abs() > f64::EPSILON {
            let fraction = (threshold - from) / (to - from);
            return Some(f64::from(here) + 0.5 + fraction * f64::from(step));
        }
        here = next;
    }
    None
}

/// Offsets of the 4 × 4 sample points inside a cell.
const POINTS: [f64; 4] = [0.2, 0.4, 0.6, 0.8];
/// Largest share of sample points that may disagree with their cell's majority.
const MAX_DISAGREEMENT: f64 = 0.10;

/// The cell grid laid over a region: 5 × 5 cells in its box plus a one-cell border.
struct Cells<'a> {
    image: &'a LumaImage,
    region: Region,
    x0: f64,
    y0: f64,
    width: f64,
    height: f64,
}

impl Cells<'_> {
    fn new(image: &LumaImage, region: Region) -> Cells<'_> {
        Cells {
            image,
            region,
            x0: f64::from(region.x0),
            y0: f64::from(region.y0),
            width: f64::from(region.x1 - region.x0) / 5.0,
            height: f64::from(region.y1 - region.y0) / 5.0,
        }
    }

    /// Majority value of each of the 7 × 7 cells (index 0 is the border cell at −1), or `None`
    /// when a cell is split evenly or too many points disagree with their cell.
    fn majority(&self, threshold: f64) -> Option<[[bool; 7]; 7]> {
        let mut dark = [[false; 7]; 7];
        let mut disagree = 0usize;
        for (cj, row) in (-1i32..).zip(dark.iter_mut()) {
            for (ci, cell) in (-1i32..).zip(row.iter_mut()) {
                let mut n = 0usize;
                for dy in POINTS {
                    for dx in POINTS {
                        let px = floor_i32(self.x0 + (f64::from(ci) + dx) * self.width);
                        let py = floor_i32(self.y0 + (f64::from(cj) + dy) * self.height);
                        if f64::from(self.image.at(px, py)) < threshold {
                            n += 1;
                        }
                    }
                }
                if n == 8 {
                    return None;
                }
                *cell = n > 8;
                disagree += n.min(16 - n);
            }
        }
        (count_f64(disagree) <= MAX_DISAGREEMENT * 49.0 * 16.0).then_some(dark)
    }

    /// Mean brightness at the centres of the ring cells and of the border cells.
    fn levels(&self) -> (f64, f64) {
        let (mut ring_sum, mut ring_n, mut border_sum, mut border_n) = (0.0, 0usize, 0.0, 0usize);
        for cj in -1i32..=5 {
            for ci in -1i32..=5 {
                let value = self.image.bilinear(
                    self.x0 + (f64::from(ci) + 0.5) * self.width,
                    self.y0 + (f64::from(cj) + 0.5) * self.height,
                );
                if ci == -1 || cj == -1 || ci == 5 || cj == 5 {
                    border_sum += value;
                    border_n += 1;
                } else if ci == 0 || cj == 0 || ci == 4 || cj == 4 {
                    ring_sum += value;
                    ring_n += 1;
                }
            }
        }
        (ring_sum / count_f64(ring_n), border_sum / count_f64(border_n))
    }

    /// Outer edges (left, right, top, bottom) to a fraction of a pixel: the `threshold`
    /// crossings along the five ring rows and columns, searched from outside, averaged. Falls
    /// back to the box edge on a side without a crossing.
    fn edges(&self, threshold: f64) -> [f64; 4] {
        let region = self.region;
        let mut sums = [0.0f64; 4];
        let mut counts = [0usize; 4];
        for k in 0..5 {
            let centre = f64::from(k) + 0.5;
            let row = floor_i32(self.y0 + centre * self.height);
            let col = floor_i32(self.x0 + centre * self.width);
            let along_row = |p: i32| f64::from(self.image.at(p, row));
            let along_col = |p: i32| f64::from(self.image.at(col, p));
            let found = [
                crossing(along_row, region.x0 - 2, 1, 3, threshold),
                crossing(along_row, region.x1 + 1, -1, 3, threshold),
                crossing(along_col, region.y0 - 2, 1, 3, threshold),
                crossing(along_col, region.y1 + 1, -1, 3, threshold),
            ];
            for ((sum, n), value) in sums.iter_mut().zip(counts.iter_mut()).zip(found) {
                if let Some(v) = value {
                    *sum += v;
                    *n += 1;
                }
            }
        }
        let fallback = [
            f64::from(region.x0),
            f64::from(region.x1),
            f64::from(region.y0),
            f64::from(region.y1),
        ];
        let mut out = fallback;
        for ((edge, &sum), &n) in out.iter_mut().zip(sums.iter()).zip(counts.iter()) {
            if n > 0 {
                *edge = sum / count_f64(n);
            }
        }
        out
    }
}

/// Every (kind, transform) whose image pattern is `inner`, as [`match_bit`]s.
fn pattern_matches(inner_bits: [bool; 9]) -> u32 {
    let mut matches = 0u32;
    for kind in 0..4 {
        for t in Transform::ALL {
            if t.inner(kind) == inner_bits {
                matches |= match_bit(kind, t);
            }
        }
    }
    matches
}

/// Classifies `region` as a finder: its box is cut into 5 × 5 cells plus a one-cell border,
/// each cell sampled at 4 × 4 points against `threshold`. The border must be light, the ring
/// dark, and the inner 3 × 3 one of the four finder patterns under some transform.
pub(crate) fn classify(image: &LumaImage, region: &Region, threshold: f64) -> Option<Finder> {
    let cells = Cells::new(image, *region);
    let dark = cells.majority(threshold)?;
    let mut inner_bits = [false; 9];
    for (j, row) in dark.iter().enumerate() {
        for (i, &d) in row.iter().enumerate() {
            let border = i == 0 || j == 0 || i == 6 || j == 6;
            let ring = !border && (i == 1 || j == 1 || i == 5 || j == 5);
            if (border && d) || (ring && !d) {
                return None;
            }
            if !border
                && !ring
                && let Some(bit) = inner_bits.get_mut((j - 2) * 3 + (i - 2))
            {
                *bit = d;
            }
        }
    }
    let matches = pattern_matches(inner_bits);
    if matches == 0 {
        return None;
    }
    let (ring_level, border_level) = cells.levels();
    let contrast = border_level - ring_level;
    if contrast < 10.0 {
        return None;
    }
    let local = f64::midpoint(ring_level, border_level);
    let [left, right, top, bottom] = cells.edges(local);
    if right - left < 5.0 || bottom - top < 5.0 {
        return None;
    }
    Some(Finder { left, right, top, bottom, threshold: local, contrast, matches })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_patterns_of_5_3_1() {
        assert_eq!(inner(TL), [false, false, true, false, false, true, true, true, true]);
        assert_eq!(inner(TR), [true, false, false, true, true, true, true, false, false]);
        assert_eq!(inner(BL), [true; 9]);
        assert_eq!(inner(BR), [true, false, false, false, false, false, false, false, false]);
    }

    #[test]
    fn transforms_keep_kinds_apart() {
        // 5.3.2 property 1: different finders differ in at least 4 inner modules under any
        // transform.
        for f in 0..4 {
            for g in 0..4 {
                if f == g {
                    continue;
                }
                for t in Transform::ALL {
                    let a = inner(f);
                    let b = t.inner(g);
                    let d = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
                    assert!(d >= 4, "{f} vs {g} under {t:?}: {d}");
                }
            }
        }
    }

    #[test]
    fn rotation_by_90_moves_tl_to_top_right() {
        // Clockwise quarter turn: (u, v) → (1 − v, u) = swap, then mirror x.
        let t = Transform(4 | 1);
        assert_eq!(t.corner(corner(TL)), (1, 0));
        assert_eq!(t.kind_at((1, 0)), TL);
        // The TL hole moves to the inner top-right: rows "#oo", "#oo", "###".
        assert_eq!(t.inner(TL), [true, false, false, true, false, false, true, true, true]);
    }
}
