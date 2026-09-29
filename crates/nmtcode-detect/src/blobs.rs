//! Connected dark regions of the thresholded image, found by a scanline flood fill that keeps,
//! for each region, its pixel count, its bounding box and its extreme pixels in eight
//! directions.

use crate::LumaImage;
use crate::binarize::Thresholds;

/// Most span ends kept per region for its outline; a larger region is not a finder.
const MAX_OUTLINE: usize = 4096;

/// Largest stack of pending spans in one flood fill. A region that needs more is not a finder;
/// its unvisited pixels are seeds of later fills, so memory stays bounded and every pixel is
/// still visited once.
const MAX_STACK: usize = 1 << 18;

/// The eight directions of [`Blob::extremes`], in increasing angle in image axes (x right,
/// y down).
pub(crate) const DIRECTIONS: [(i64, i64); 8] =
    [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

/// One 4-connected dark region.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Blob {
    /// Number of pixels.
    pub area: u64,
    /// Bounding box: least x, least y, greatest x, greatest y (inclusive).
    pub bbox: [usize; 4],
    /// For each of [`DIRECTIONS`], a pixel (x, y) of the region that maximises dx · x + dy · y.
    pub extremes: [(usize, usize); 8],
}

/// Pixel states of the fill.
const LIGHT: u8 = 0;
const DARK: u8 = 1;
const DONE: u8 = 2;

/// Calls `visit` for every dark region of `image` under `thresholds` whose bounding box is at
/// least `min_side` pixels wide and high, with the ends of its spans as (left, right, row)
/// (empty for a region with more than 4096 spans). Stops early when `visit` returns false.
pub(crate) fn for_each_blob(
    image: &LumaImage,
    thresholds: &Thresholds,
    min_side: usize,
    mut visit: impl FnMut(&Blob, &[(usize, usize, usize)]) -> bool,
) {
    let w = image.w_usize();
    let h = image.h_usize();
    if w == 0 || h == 0 {
        return;
    }
    let mut map: Vec<u8> = Vec::with_capacity(w * h);
    for (y, row) in image.pixels.chunks(w).enumerate() {
        let brow = (y / crate::binarize::BLOCK) * thresholds.bw;
        for (bx, chunk) in row.chunks(crate::binarize::BLOCK).enumerate() {
            let t = thresholds.t.get(brow + bx).copied().unwrap_or(0);
            map.extend(chunk.iter().map(|&v| if v < t { DARK } else { LIGHT }));
        }
    }
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    for sy in 0..h {
        let mut sx = 0usize;
        // The next unvisited dark pixel of the row.
        while let Some(offset) =
            map.get(sy * w + sx..(sy + 1) * w).and_then(|row| row.iter().position(|&v| v == DARK))
        {
            sx += offset;
            let Some(blob) = fill(&mut map, w, h, sx, sy, &mut stack, &mut spans) else {
                sx += 1;
                continue;
            };
            sx += 1;
            let bw = blob.bbox[2] - blob.bbox[0] + 1;
            let bh = blob.bbox[3] - blob.bbox[1] + 1;
            if spans.len() >= MAX_OUTLINE {
                spans.clear();
            }
            if bw >= min_side && bh >= min_side && !visit(&blob, &spans) {
                return;
            }
            if sx >= w {
                break;
            }
        }
    }
}

/// Scanline flood fill of the dark region containing (`sx`, `sy`). `None` when the fill
/// needed more than [`MAX_STACK`] pending spans.
fn fill(
    map: &mut [u8],
    w: usize,
    h: usize,
    sx: usize,
    sy: usize,
    stack: &mut Vec<(usize, usize)>,
    spans: &mut Vec<(usize, usize, usize)>,
) -> Option<Blob> {
    spans.clear();
    let mut blob = Blob { area: 0, bbox: [sx, sy, sx, sy], extremes: [(sx, sy); 8] };
    let mut best = [i64::MIN; 8];
    let mut overflow = false;
    stack.clear();
    stack.push((sx, sy));
    while let Some((px, py)) = stack.pop() {
        let row = py * w;
        if map.get(row + px).copied() != Some(DARK) {
            continue;
        }
        let mut left = px;
        while left > 0 && map.get(row + left - 1).copied() == Some(DARK) {
            left -= 1;
        }
        let mut right = px;
        while right + 1 < w && map.get(row + right + 1).copied() == Some(DARK) {
            right += 1;
        }
        if let Some(span) = map.get_mut(row + left..=row + right) {
            span.fill(DONE);
        }
        blob.area += u64::try_from(right - left + 1).unwrap_or(0);
        if spans.len() < MAX_OUTLINE {
            spans.push((left, right, py));
        }
        blob.bbox = [
            blob.bbox[0].min(left),
            blob.bbox[1].min(py),
            blob.bbox[2].max(right),
            blob.bbox[3].max(py),
        ];
        let (l, r, y) = (
            i64::try_from(left).unwrap_or(0),
            i64::try_from(right).unwrap_or(0),
            i64::try_from(py).unwrap_or(0),
        );
        for ((&(dx, dy), score), extreme) in
            DIRECTIONS.iter().zip(best.iter_mut()).zip(blob.extremes.iter_mut())
        {
            let (x, xu) = if dx > 0 { (r, right) } else { (l, left) };
            let s = dx * x + dy * y;
            if s > *score {
                *score = s;
                *extreme = (xu, py);
            }
        }
        for ny in [py.wrapping_sub(1), py + 1] {
            if ny >= h {
                continue;
            }
            let next = ny * w;
            let mut cx = left;
            while cx <= right {
                if map.get(next + cx).copied() != Some(DARK) {
                    cx += 1;
                    continue;
                }
                if stack.len() < MAX_STACK {
                    stack.push((cx, ny));
                } else {
                    overflow = true;
                }
                while cx <= right && map.get(next + cx).copied() == Some(DARK) {
                    cx += 1;
                }
            }
        }
    }
    (!overflow).then_some(blob)
}

/// The smallest-area rectangle around the pixels of a region given by its spans (left, right,
/// row), as four corners in increasing angle; `None` for fewer than 3 hull points. The pixel
/// corners are used, so the rectangle lies on the region's outer pixel edges.
pub(crate) fn outline_rectangle(
    spans: &[(usize, usize, usize)],
) -> Option<[crate::geom::Point; 4]> {
    use crate::geom::Point;
    use crate::num::count_f64;
    // The leftmost and rightmost pixel of each row; the hull needs no other point.
    let (y0, y1) =
        spans.iter().fold((usize::MAX, 0usize), |(a, b), &(_, _, y)| (a.min(y), b.max(y)));
    if y0 > y1 || y1 - y0 > 2048 {
        return None;
    }
    let mut rows = vec![(usize::MAX, 0usize); y1 - y0 + 1];
    for &(l, r, y) in spans {
        if let Some(row) = rows.get_mut(y - y0) {
            *row = (row.0.min(l), row.1.max(r));
        }
    }
    // Points sorted by y, then x: Andrew's chain works on any lexicographic order.
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(rows.len() * 4);
    for (i, &(l, r)) in rows.iter().enumerate() {
        if l > r {
            continue;
        }
        let y = count_f64(y0 + i);
        let (l, r) = (count_f64(l), count_f64(r) + 1.0);
        pts.extend_from_slice(&[(l, y), (r, y), (l, y + 1.0), (r, y + 1.0)]);
    }
    let key = |p: &(f64, f64)| (p.1, p.0);
    pts.sort_by(|a, b| key(a).0.total_cmp(&key(b).0).then(key(a).1.total_cmp(&key(b).1)));
    pts.dedup();
    if pts.len() < 3 {
        return None;
    }
    // Andrew's monotone chain.
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut hull: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &(f64, f64)>> =
            if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in iter {
            while hull.len() >= start + 2 {
                let (Some(&a), Some(&b)) = (hull.get(hull.len() - 2), hull.last()) else { break };
                if cross(a, b, p) <= 0.0 {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(p);
        }
        hull.pop();
    }
    if hull.len() < 3 {
        return None;
    }
    // Rotating calipers, by brute force over the hull's edge directions.
    let mut best: Option<(f64, Point, Point, [f64; 4])> = None;
    for i in 0..hull.len() {
        let (Some(&a), Some(&b)) = (hull.get(i), hull.get((i + 1) % hull.len())) else { continue };
        let d = Point::new(b.0 - a.0, b.1 - a.1);
        let n = d.norm();
        if n < 1e-9 {
            continue;
        }
        let u = d.scale(1.0 / n);
        let v = Point::new(-u.y, u.x);
        let (mut lo_u, mut hi_u, mut lo_v, mut hi_v) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for &(x, y) in &hull {
            let p = Point::new(x, y);
            let (pu, pv) = (u.dot(p), v.dot(p));
            lo_u = lo_u.min(pu);
            hi_u = hi_u.max(pu);
            lo_v = lo_v.min(pv);
            hi_v = hi_v.max(pv);
        }
        let area = (hi_u - lo_u) * (hi_v - lo_v);
        if best.is_none_or(|(s, ..)| area < s) {
            best = Some((area, u, v, [lo_u, hi_u, lo_v, hi_v]));
        }
    }
    let (_, u, v, [lo_u, hi_u, lo_v, hi_v]) = best?;
    let corner = |pu: f64, pv: f64| u.scale(pu).add(v.scale(pv));
    let quad = [corner(lo_u, lo_v), corner(hi_u, lo_v), corner(hi_u, hi_v), corner(lo_u, hi_v)];
    // Increasing angle in image axes: flip when the order runs the other way.
    Some(if crate::geom::signed_area2(&quad) >= 0.0 {
        quad
    } else {
        [quad[0], quad[3], quad[2], quad[1]]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_square_and_its_corners() {
        let mut pixels = vec![255u8; 40 * 40];
        for y in 10..20 {
            for x in 5..25 {
                pixels[y * 40 + x] = 0;
            }
        }
        let image = LumaImage::new(40, 40, pixels).unwrap();
        let thresholds = crate::binarize::thresholds(&image, 0.5);
        let mut blobs = Vec::new();
        for_each_blob(&image, &thresholds, 3, |b, spans| {
            assert_eq!(spans.len(), 10);
            blobs.push(*b);
            true
        });
        assert_eq!(blobs.len(), 1);
        let b = blobs[0];
        assert_eq!(b.area, 200);
        assert_eq!(b.bbox, [5, 10, 24, 19]);
        // Direction (1, 1) reaches the bottom-right pixel, (−1, −1) the top-left one.
        assert_eq!(b.extremes[1], (24, 19));
        assert_eq!(b.extremes[5], (5, 10));
    }
}
