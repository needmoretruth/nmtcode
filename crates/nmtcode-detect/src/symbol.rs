//! From finders to symbols: grouping, size measurement, module sampling and the final check.

use nmtcode_core::{MAX_SIDE, MIN_SIDE, ModuleGrid, is_valid_side};

use crate::finder::{self, BL, BR, Finder, TL, TR, Transform, corner, finder_module};
use crate::num::{ceil_u32, count_f64, floor_i32, floor_u32, round_u8, u64_f64};
use crate::{Detection, LumaImage};

/// At most this many finder candidates are grouped; grouping is quadratic in their number.
const MAX_FINDERS: usize = 2048;
/// Two finders of one symbol have module sizes within this ratio.
const MODULE_RATIO: f64 = 1.25;
/// Largest mean distance, in modules, between the measured edges and the chosen module grid.
const MAX_GRID_ERROR: f64 = 0.22;
/// Lines scanned for edges when measuring a side.
const MAX_SCAN_LINES: u32 = 64;
/// Block of modules over which the local threshold is taken.
const BLOCK: u32 = 8;

/// Otsu's threshold, returned as the midpoint of the mean dark and mean light values so that a
/// two-level image splits halfway. `None` for an image without contrast.
fn global_threshold(image: &LumaImage) -> Option<f64> {
    let mut histogram = [0u64; 256];
    for &v in &image.pixels {
        if let Some(bin) = histogram.get_mut(usize::from(v)) {
            *bin += 1;
        }
    }
    let total: u64 = histogram.iter().sum();
    let sum: f64 = (0u32..).zip(histogram.iter()).map(|(v, &n)| f64::from(v) * u64_f64(n)).sum();
    let (mut w0, mut sum0, mut best, mut best_k) = (0u64, 0.0f64, -1.0f64, 0u32);
    for (k, &n) in (0u32..).zip(histogram.iter()) {
        w0 += n;
        sum0 += f64::from(k) * u64_f64(n);
        let w1 = total - w0;
        if w0 == 0 || w1 == 0 {
            continue;
        }
        let m0 = sum0 / u64_f64(w0);
        let m1 = (sum - sum0) / u64_f64(w1);
        let between = u64_f64(w0) * u64_f64(w1) * (m0 - m1) * (m0 - m1);
        if between > best {
            best = between;
            best_k = k;
        }
    }
    if best < 0.0 {
        return None;
    }
    let (mut n0, mut s0, mut n1, mut s1) = (0u64, 0.0, 0u64, 0.0);
    for (v, &n) in (0u32..).zip(histogram.iter()) {
        if v <= best_k {
            n0 += n;
            s0 += f64::from(v) * u64_f64(n);
        } else {
            n1 += n;
            s1 += f64::from(v) * u64_f64(n);
        }
    }
    if n0 == 0 || n1 == 0 {
        return None;
    }
    let dark = s0 / u64_f64(n0);
    let light = s1 / u64_f64(n1);
    (light - dark >= 20.0).then(|| f64::midpoint(dark, light))
}

/// A symbol found in the image: its grid in symbol orientation and its rectangle in the image,
/// (left, top, right, bottom) in pixels.
struct Found {
    grid: ModuleGrid,
    bounds: [f64; 4],
}

/// Every symbol in `image`, dark on light (chapter 5, 5.11), and the number of nested ones.
pub(crate) fn find(image: &LumaImage) -> Detection {
    if !image.is_usable() {
        return Detection::default();
    }
    let upright = find_upright(image);
    if !upright.is_empty() {
        return split_nested(upright, false);
    }
    // 1.4 and 5.11: a reader MAY try the inverted image. It is tried only when the image as
    // captured holds no symbol, so a dark-on-light symbol costs no second pass.
    let inverted = LumaImage {
        width: image.width,
        height: image.height,
        pixels: image.pixels.iter().map(|&v| u8::MAX - v).collect(),
    };
    split_nested(find_upright(&inverted), true)
}

/// The nesting rule of 5.11: a symbol whose rectangle lies inside another symbol's rectangle,
/// and that other symbol, are both left out and counted.
fn split_nested(found: Vec<Found>, inverted: bool) -> Detection {
    let inside =
        |a: &[f64; 4], b: &[f64; 4]| a[0] >= b[0] && a[1] >= b[1] && a[2] <= b[2] && a[3] <= b[3];
    let mut nested = vec![false; found.len()];
    for (i, a) in found.iter().enumerate() {
        for (j, b) in found.iter().enumerate() {
            if i != j && inside(&a.bounds, &b.bounds) {
                if let Some(flag) = nested.get_mut(i) {
                    *flag = true;
                }
                if let Some(flag) = nested.get_mut(j) {
                    *flag = true;
                }
            }
        }
    }
    let count = nested.iter().filter(|&&n| n).count();
    let symbols = found
        .into_iter()
        .zip(nested)
        .filter_map(|(f, is_nested)| (!is_nested).then_some(f.grid))
        .collect();
    Detection { symbols, nested: count, inverted }
}

/// Every symbol in `image` as captured, dark modules darker than light ones.
fn find_upright(image: &LumaImage) -> Vec<Found> {
    let Some(threshold) = global_threshold(image) else {
        return Vec::new();
    };
    let finders: Vec<Finder> = finder::regions(image, threshold)
        .iter()
        .filter_map(|region| finder::classify(image, region, threshold))
        .take(MAX_FINDERS)
        .collect();
    let mut used = vec![false; finders.len()];
    let mut out = Vec::new();
    for k in 0..finders.len() {
        if used.get(k).copied().unwrap_or(true) {
            continue;
        }
        let Some(anchor) = finders.get(k) else { continue };
        'transforms: for kind in [TL, TR, BL, BR] {
            for t in Transform::ALL {
                if !anchor.has(kind, t) {
                    continue;
                }
                if let Some((found, members)) = try_symbol(image, &finders, &used, k, kind, t) {
                    for m in members.into_iter().flatten() {
                        if let Some(u) = used.get_mut(m) {
                            *u = true;
                        }
                    }
                    out.push(found);
                    break 'transforms;
                }
            }
        }
    }
    out
}

/// Direction of a search from one finder to its neighbour along a symbol edge.
#[derive(Clone, Copy)]
enum Direction {
    Right,
    Left,
    Down,
    Up,
}

fn similar_size(a: &Finder, b: &Finder) -> bool {
    let ratio = a.module() / b.module();
    (1.0 / MODULE_RATIO..=MODULE_RATIO).contains(&ratio)
}

/// The nearest unused finder of (`kind`, `t`) from `from` in `direction`, level with it (same
/// top and bottom edges for a horizontal search, same left and right edges for a vertical one)
/// and at least 15 modules away centre to centre, the least a 20-module side allows.
fn nearest(
    finders: &[Finder],
    used: &[bool],
    from: usize,
    kind: usize,
    t: Transform,
    direction: Direction,
) -> Option<usize> {
    let a = finders.get(from)?;
    let mut best: Option<(usize, f64)> = None;
    for (j, b) in finders.iter().enumerate() {
        if j == from
            || used.get(j).copied().unwrap_or(true)
            || !b.has(kind, t)
            || !similar_size(a, b)
        {
            continue;
        }
        let m = a.module().min(b.module());
        let tol = 0.6 * m;
        let (level, gap) = match direction {
            Direction::Right | Direction::Left => (
                (b.top - a.top).abs() <= tol && (b.bottom - a.bottom).abs() <= tol,
                if matches!(direction, Direction::Right) {
                    b.left - a.left
                } else {
                    a.left - b.left
                },
            ),
            Direction::Down | Direction::Up => (
                (b.left - a.left).abs() <= tol && (b.right - a.right).abs() <= tol,
                if matches!(direction, Direction::Down) { b.top - a.top } else { a.top - b.top },
            ),
        };
        if !level || gap < 14.0 * m {
            continue;
        }
        if best.is_none_or(|(_, g)| gap < g) {
            best = Some((j, gap));
        }
    }
    best.map(|(j, _)| j)
}

fn horizontal(c: (u8, u8)) -> Direction {
    if c.0 == 0 { Direction::Right } else { Direction::Left }
}

fn vertical(c: (u8, u8)) -> Direction {
    if c.1 == 0 { Direction::Down } else { Direction::Up }
}

/// Index of an image corner in a `[_; 4]`: x + 2y.
fn slot(c: (u8, u8)) -> usize {
    usize::from(c.0) + 2 * usize::from(c.1)
}

/// Tries to build a symbol with finder `k` as `kind` seen through transform `t`. Returns the
/// symbol and the finders used, by image corner.
fn try_symbol(
    image: &LumaImage,
    finders: &[Finder],
    used: &[bool],
    anchor: usize,
    kind: usize,
    transform: Transform,
) -> Option<(Found, [Option<usize>; 4])> {
    let at_anchor = transform.corner(corner(kind));
    let at_side = (1 - at_anchor.0, at_anchor.1);
    let at_end = (at_anchor.0, 1 - at_anchor.1);
    let at_diagonal = (1 - at_anchor.0, 1 - at_anchor.1);
    let across = horizontal(at_anchor);
    let along = vertical(at_anchor);
    let side = nearest(finders, used, anchor, transform.kind_at(at_side), transform, across);
    let end = nearest(finders, used, anchor, transform.kind_at(at_end), transform, along);
    let diagonal_kind = transform.kind_at(at_diagonal);
    let diagonal = match (side, end) {
        (Some(side), Some(end)) => nearest(finders, used, side, diagonal_kind, transform, along)
            .filter(|&found| {
                nearest(finders, used, end, diagonal_kind, transform, across) == Some(found)
            }),
        (Some(side), None) => nearest(finders, used, side, diagonal_kind, transform, along),
        (None, Some(end)) => nearest(finders, used, end, diagonal_kind, transform, across),
        (None, None) => None,
    };
    let mut corners = [None; 4];
    for (c, found) in
        [(at_anchor, Some(anchor)), (at_side, side), (at_end, end), (at_diagonal, diagonal)]
    {
        if let Some(s) = corners.get_mut(slot(c)) {
            *s = found;
        }
    }
    if corners.iter().flatten().count() < 3 {
        return None;
    }
    let found = measure_and_sample(image, finders, &corners, transform)?;
    Some((found, corners))
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / count_f64(n))
}

/// The size tolerance of 5.11: a side of `n` modules agrees with the finder estimate `estimate`
/// when they differ by at most max(4, n / 10) modules.
pub(crate) fn within_tolerance(n: u32, estimate: f64) -> bool {
    let n = f64::from(n);
    (n - estimate).abs() <= (n / 10.0).max(4.0)
}

/// Chooses the number of modules along one side: among the multiples of 4 from 20 to 4108
/// that agree with the finder estimate `(hi − lo) / module` within the tolerance of 5.11
/// ([`within_tolerance`]), the one whose grid lines lie closest, on average, to the
/// `threshold` crossings found along up to 64 scan lines spread between `cross_lo` and
/// `cross_hi`. `value(line, p)` reads pixel `p` of scan line `line`; `lo` and `hi` are the
/// symbol's outer edges along the side. `None` when no candidate fits within
/// [`MAX_GRID_ERROR`].
fn count_modules(
    value: impl Fn(i32, i32) -> f64,
    lo: f64,
    hi: f64,
    cross_lo: f64,
    cross_hi: f64,
    module: f64,
    threshold: f64,
) -> Option<u32> {
    let span = hi - lo;
    if span <= 0.0 || module <= 0.0 {
        return None;
    }
    let rough = span / module;
    // Every n within the tolerance lies between rough / 1.1 − 4 and rough / 0.9 + 4.
    let first = ceil_u32(rough / 1.1 - 4.0).max(MIN_SIDE).next_multiple_of(4);
    let last = floor_u32(rough / 0.9 + 4.0).min(MAX_SIDE);
    if first > last {
        return None;
    }
    let cross_modules = (cross_hi - cross_lo) / module;
    let lines = floor_u32(cross_modules).clamp(8, MAX_SCAN_LINES);
    let mut positions = Vec::new();
    let p0 = floor_i32(lo - 0.5 * module);
    let p1 = floor_i32(hi + 0.5 * module);
    for i in 0..lines {
        let c = cross_lo + (f64::from(i) + 0.5) * (cross_hi - cross_lo) / f64::from(lines);
        let line = floor_i32(c);
        let mut prev = value(line, p0);
        let mut p = p0;
        while p < p1 {
            let q = p.saturating_add(1);
            let next = value(line, q);
            if (prev < threshold) != (next < threshold) && (next - prev).abs() > f64::EPSILON {
                let x = f64::from(p) + 0.5 + (threshold - prev) / (next - prev);
                positions.push((x - lo) / span);
            }
            prev = next;
            p = q;
        }
    }
    if positions.is_empty() {
        return None;
    }
    let mut best: Option<(u32, f64)> = None;
    for n in (first..=last).step_by(4).filter(|&n| within_tolerance(n, rough)) {
        let nf = f64::from(n);
        let error: f64 = positions
            .iter()
            .map(|&u| {
                let g = u * nf;
                (g - g.round()).abs()
            })
            .sum::<f64>()
            / count_f64(positions.len());
        if best.is_none_or(|(_, e)| error < e) {
            best = Some((n, error));
        }
    }
    best.filter(|&(_, e)| e <= MAX_GRID_ERROR).map(|(n, _)| n)
}

/// Measures the symbol framed by `corners` (image corners, x + 2y), samples every module and
/// checks the finders in the sampled grid.
fn measure_and_sample(
    image: &LumaImage,
    finders: &[Finder],
    corners: &[Option<usize>; 4],
    t: Transform,
) -> Option<Found> {
    let at = |s: usize| corners.get(s).copied().flatten().and_then(|i| finders.get(i));
    let present = || corners.iter().flatten().filter_map(|&i| finders.get(i));
    let left = mean([at(0), at(2)].into_iter().flatten().map(|f| f.left))?;
    let right = mean([at(1), at(3)].into_iter().flatten().map(|f| f.right))?;
    let top = mean([at(0), at(1)].into_iter().flatten().map(|f| f.top))?;
    let bottom = mean([at(2), at(3)].into_iter().flatten().map(|f| f.bottom))?;
    let module_x = mean(present().map(Finder::module_x))?;
    let module_y = mean(present().map(Finder::module_y))?;
    let threshold = mean(present().map(|f| f.threshold))?;
    let contrast = mean(present().map(|f| f.contrast))?;

    let columns = count_modules(
        |row, x| f64::from(image.at(x, row)),
        left,
        right,
        top,
        bottom,
        module_x,
        threshold,
    )?;
    let rows = count_modules(
        |col, y| f64::from(image.at(col, y)),
        top,
        bottom,
        left,
        right,
        module_y,
        threshold,
    )?;

    // Module centres in image orientation.
    let pitch_x = (right - left) / f64::from(columns);
    let pitch_y = (bottom - top) / f64::from(rows);
    let wi = usize::try_from(columns).ok()?;
    let hi = usize::try_from(rows).ok()?;
    let mut samples = vec![0u8; wi * hi];
    for (yi, row) in (0u32..).zip(samples.chunks_mut(wi)) {
        let y = top + (f64::from(yi) + 0.5) * pitch_y;
        for (xi, s) in (0u32..).zip(row.iter_mut()) {
            let x = left + (f64::from(xi) + 0.5) * pitch_x;
            *s = round_u8(image.bilinear(x, y));
        }
    }

    // Local threshold: midpoint of the darkest and lightest centres in the 3 × 3 blocks of
    // 8 × 8 modules around each module; the finders' threshold where that range is flat.
    let bw = columns.div_ceil(BLOCK);
    let bh = rows.div_ceil(BLOCK);
    let bwu = usize::try_from(bw).ok()?;
    let mut lows = vec![u8::MAX; bwu * usize::try_from(bh).ok()?];
    let mut highs = vec![0u8; lows.len()];
    for (yi, row) in (0u32..).zip(samples.chunks(wi)) {
        for (xi, &s) in (0u32..).zip(row.iter()) {
            let b = usize::try_from((yi / BLOCK) * bw + xi / BLOCK).ok()?;
            if let (Some(lo), Some(hi)) = (lows.get_mut(b), highs.get_mut(b)) {
                *lo = (*lo).min(s);
                *hi = (*hi).max(s);
            }
        }
    }
    let block_threshold = |bx: u32, by: u32| -> f64 {
        let (mut lo, mut hi) = (u8::MAX, 0u8);
        for ny in by.saturating_sub(1)..=(by + 1).min(bh - 1) {
            for nx in bx.saturating_sub(1)..=(bx + 1).min(bw - 1) {
                let b = usize::try_from(ny * bw + nx).unwrap_or(usize::MAX);
                if let (Some(&l), Some(&h)) = (lows.get(b), highs.get(b)) {
                    lo = lo.min(l);
                    hi = hi.max(h);
                }
            }
        }
        if f64::from(hi) - f64::from(lo) >= 0.5 * contrast {
            f64::midpoint(f64::from(lo), f64::from(hi))
        } else {
            threshold
        }
    };
    let mut thresholds = vec![threshold; lows.len()];
    for by in 0..bh {
        for bx in 0..bw {
            if let Some(slot) =
                usize::try_from(by * bw + bx).ok().and_then(|b| thresholds.get_mut(b))
            {
                *slot = block_threshold(bx, by);
            }
        }
    }

    // Back to the symbol's own orientation.
    let (width, height) = if t.swaps() { (rows, columns) } else { (columns, rows) };
    if !is_valid_side(width) || !is_valid_side(height) {
        return None;
    }
    let mut grid = ModuleGrid::new(width, height)?;
    for v in 0..height {
        for u in 0..width {
            let (xi, yi) = t.apply(u, v, columns - 1, rows - 1);
            let index = usize::try_from(u64::from(yi) * u64::from(columns) + u64::from(xi)).ok()?;
            let b = usize::try_from((yi / BLOCK) * bw + xi / BLOCK).ok()?;
            let (Some(&s), Some(&thr)) = (samples.get(index), thresholds.get(b)) else {
                return None;
            };
            grid.set(u, v, f64::from(s) < thr);
        }
    }
    (finders_intact(&grid) >= 3).then_some(Found { grid, bounds: [left, top, right, bottom] })
}

/// Number of the four finders (5.3) that, with their separators (5.4), are exact in `grid`.
fn finders_intact(grid: &ModuleGrid) -> usize {
    let (w, h) = (grid.width(), grid.height());
    [TL, TR, BL, BR]
        .into_iter()
        .filter(|&kind| {
            let (cx, cy) = corner(kind);
            let fx = if cx == 0 { 0 } else { w - 5 };
            let fy = if cy == 0 { 0 } else { h - 5 };
            // Separator column and row on the sides facing the inside (5.4).
            let sx = if cx == 0 { 5 } else { w - 6 };
            let sy = if cy == 0 { 5 } else { h - 6 };
            let finder_ok = (0u32..5).zip(0usize..).all(|(y, yu)| {
                (0u32..5)
                    .zip(0usize..)
                    .all(|(x, xu)| grid.get(fx + x, fy + y) == Some(finder_module(kind, xu, yu)))
            });
            let span_y = if cy == 0 { 0..=5 } else { h - 6..=h - 1 };
            let span_x = if cx == 0 { 0..=5 } else { w - 6..=w - 1 };
            let separator_ok = span_y.clone().all(|y| grid.get(sx, y) == Some(false))
                && span_x.clone().all(|x| grid.get(x, sy) == Some(false));
            finder_ok && separator_ok
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::within_tolerance;

    #[test]
    fn size_tolerance_of_5_11() {
        // max(4, W / 10): 4 modules up to W = 40, then 10%.
        assert!(within_tolerance(20, 24.0) && within_tolerance(20, 16.0));
        assert!(!within_tolerance(20, 24.1) && !within_tolerance(20, 15.9));
        assert!(within_tolerance(40, 44.0) && !within_tolerance(40, 44.1));
        assert!(within_tolerance(100, 110.0) && !within_tolerance(100, 110.1));
        assert!(within_tolerance(100, 90.0) && !within_tolerance(100, 89.9));
        assert!(within_tolerance(4108, 3697.25) && !within_tolerance(4108, 3697.15));
    }
}
