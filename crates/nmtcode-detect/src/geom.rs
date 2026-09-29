//! Points, lines and plane homographies.

use crate::num::solve;

/// A point in image pixels: x grows to the right and y downward. Pixel (i, j) covers
/// [i, i + 1) × [j, j + 1), so its centre is (i + 0.5, j + 0.5).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal position in pixels.
    pub x: f64,
    /// Vertical position in pixels.
    pub y: f64,
}

impl Point {
    /// The point (`x`, `y`).
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub(crate) fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }

    pub(crate) fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }

    pub(crate) fn scale(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s)
    }

    pub(crate) fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y
    }

    pub(crate) fn cross(self, o: Self) -> f64 {
        self.x * o.y - self.y * o.x
    }

    pub(crate) fn norm(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub(crate) fn dist(self, o: Self) -> f64 {
        self.sub(o).norm()
    }

    pub(crate) fn lerp(self, o: Self, t: f64) -> Self {
        self.add(o.sub(self).scale(t))
    }

    pub(crate) fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// A line through `point` with unit direction `dir`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line {
    pub point: Point,
    pub dir: Point,
}

impl Line {
    /// The line through two distinct points.
    pub(crate) fn through(a: Point, b: Point) -> Option<Self> {
        let d = b.sub(a);
        let n = d.norm();
        (n > 1e-9 && n.is_finite()).then(|| Self { point: a, dir: d.scale(1.0 / n) })
    }

    /// Total least squares fit through `points` (at least two, not all equal).
    pub(crate) fn fit(points: &[Point]) -> Option<Self> {
        if points.len() < 2 {
            return None;
        }
        let n = crate::num::count_f64(points.len());
        let c = points.iter().fold(Point::default(), |s, p| s.add(*p)).scale(1.0 / n);
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for p in points {
            let d = p.sub(c);
            sxx += d.x * d.x;
            sxy += d.x * d.y;
            syy += d.y * d.y;
        }
        // Principal direction of the 2 × 2 scatter matrix.
        let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let dir = Point::new(angle.cos(), angle.sin());
        (sxx + syy > 1e-12 && dir.is_finite() && c.is_finite()).then_some(Self { point: c, dir })
    }

    /// Signed distance of `p` from the line (positive to the left of `dir` in image axes).
    pub(crate) fn distance(&self, p: Point) -> f64 {
        self.dir.cross(p.sub(self.point))
    }

    /// Position of the projection of `p` along the line.
    pub(crate) fn along(&self, p: Point) -> f64 {
        self.dir.dot(p.sub(self.point))
    }

    /// Intersection with `other`; `None` for (nearly) parallel lines.
    pub(crate) fn intersect(&self, other: &Self) -> Option<Point> {
        let denom = self.dir.cross(other.dir);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = other.point.sub(self.point).cross(other.dir) / denom;
        let p = self.point.add(self.dir.scale(t));
        p.is_finite().then_some(p)
    }
}

/// A plane projective map (x, y) → (u, v), as a 3 × 3 matrix, row-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Homography {
    m: [f64; 9],
}

impl Homography {
    /// Applies the map. A point that maps to infinity gives a far, finite point.
    pub(crate) fn apply(&self, p: Point) -> Point {
        let [a, b, c, d, e, f, g, h, i] = self.m;
        let w = g * p.x + h * p.y + i;
        let w = if w.abs() < 1e-12 { 1e-12_f64.copysign(w) } else { w };
        let out = Point::new((a * p.x + b * p.y + c) / w, (d * p.x + e * p.y + f) / w);
        if out.is_finite() { out } else { Point::new(-1e9, -1e9) }
    }

    /// The Jacobian of the map at `p`: the images of unit steps in x and in y.
    pub(crate) fn jacobian(&self, p: Point) -> (Point, Point) {
        let h = 0.5;
        let dx = self.apply(Point::new(p.x + h, p.y)).sub(self.apply(Point::new(p.x - h, p.y)));
        let dy = self.apply(Point::new(p.x, p.y + h)).sub(self.apply(Point::new(p.x, p.y - h)));
        (dx, dy)
    }

    /// The inverse map, `None` when singular.
    pub(crate) fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f, g, h, i] = self.m;
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
        if det.abs() < 1e-18 || !det.is_finite() {
            return None;
        }
        let m = co.map(|v| v / det);
        m.iter().all(|v| v.is_finite()).then_some(Self { m })
    }

    /// The map that takes each `src[i]` closest to `dst[i]` in the least-squares sense of the
    /// direct linear transform, with both point sets normalised first. At least 4 pairs.
    pub(crate) fn fit(src: &[Point], dst: &[Point]) -> Option<Self> {
        if src.len() != dst.len() || src.len() < 4 {
            return None;
        }
        let (ts, src_n) = normalise(src)?;
        let (td, dst_n) = normalise(dst)?;
        // Unknowns h0..h7 with h8 = 1.
        let mut ata = [0.0f64; 64];
        let mut atb = [0.0f64; 8];
        for (p, q) in src_n.iter().zip(dst_n.iter()) {
            let rows = [
                ([p.x, p.y, 1.0, 0.0, 0.0, 0.0, -p.x * q.x, -p.y * q.x], q.x),
                ([0.0, 0.0, 0.0, p.x, p.y, 1.0, -p.x * q.y, -p.y * q.y], q.y),
            ];
            for (row, rhs) in rows {
                for (i, &ri) in row.iter().enumerate() {
                    if let Some(v) = atb.get_mut(i) {
                        *v += ri * rhs;
                    }
                    for (j, &rj) in row.iter().enumerate() {
                        if let Some(v) = ata.get_mut(i * 8 + j) {
                            *v += ri * rj;
                        }
                    }
                }
            }
        }
        solve(&mut ata, &mut atb, 8)?;
        let [h0, h1, h2, h3, h4, h5, h6, h7] = atb;
        let hn = Self { m: [h0, h1, h2, h3, h4, h5, h6, h7, 1.0] };
        // H = Td⁻¹ · Hn · Ts.
        let td_inv = td.inverse()?;
        let out = td_inv.compose(&hn).compose(&ts);
        out.normalised()
    }

    /// The map of the square with corners (0, 0), (`side`, 0), (`side`, `side`), (0, `side`)
    /// onto `corners` in that order.
    pub(crate) fn from_square(corners: &[Point; 4], side: f64) -> Option<Self> {
        let src = [
            Point::new(0.0, 0.0),
            Point::new(side, 0.0),
            Point::new(side, side),
            Point::new(0.0, side),
        ];
        Self::fit(&src, corners)
    }

    /// `self ∘ other`: first `other`, then `self`.
    fn compose(&self, other: &Self) -> Self {
        let a = &self.m;
        let b = &other.m;
        let mut m = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += a.get(r * 3 + k).copied().unwrap_or(0.0)
                        * b.get(k * 3 + c).copied().unwrap_or(0.0);
                }
                if let Some(v) = m.get_mut(r * 3 + c) {
                    *v = s;
                }
            }
        }
        Self { m }
    }

    fn normalised(self) -> Option<Self> {
        let s = self.m[8];
        let m = if s.abs() > 1e-12 { self.m.map(|v| v / s) } else { self.m };
        m.iter().all(|v| v.is_finite()).then_some(Self { m })
    }
}

/// Hartley normalisation: translate the centroid to the origin and scale the mean distance to
/// √2. Returns the normalising map and the normalised points.
fn normalise(points: &[Point]) -> Option<(Homography, Vec<Point>)> {
    let n = crate::num::count_f64(points.len());
    let c = points.iter().fold(Point::default(), |s, p| s.add(*p)).scale(1.0 / n);
    let mean = points.iter().map(|p| p.dist(c)).sum::<f64>() / n;
    if !(mean > 1e-12 && mean.is_finite()) {
        return None;
    }
    let s = core::f64::consts::SQRT_2 / mean;
    let t = Homography { m: [s, 0.0, -s * c.x, 0.0, s, -s * c.y, 0.0, 0.0, 1.0] };
    Some((t, points.iter().map(|p| t.apply(*p)).collect()))
}

/// Twice the signed area of the polygon `points` (positive for the orientation of the image
/// axes: x right, y down, corners in increasing angle).
pub(crate) fn signed_area2(points: &[Point]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let a = points.get(i).copied().unwrap_or_default();
            let b = points.get((i + 1) % n).copied().unwrap_or_default();
            a.cross(b)
        })
        .sum()
}

/// True when `p` lies inside the convex quadrilateral `quad` or on its boundary.
pub(crate) fn inside_convex(quad: &[Point; 4], p: Point) -> bool {
    let signs = [0usize, 1, 2, 3].map(|i| {
        let a = quad[i];
        let b = quad[(i + 1) % 4];
        b.sub(a).cross(p.sub(a))
    });
    signs.iter().all(|&s| s >= -1e-9) || signs.iter().all(|&s| s <= 1e-9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homography_from_four_points() {
        let corners = [
            Point::new(10.0, 20.0),
            Point::new(60.0, 25.0),
            Point::new(55.0, 80.0),
            Point::new(5.0, 70.0),
        ];
        let h = Homography::from_square(&corners, 5.0).unwrap();
        for (i, c) in [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)].into_iter().enumerate() {
            let p = h.apply(Point::new(c.0, c.1));
            assert!(p.dist(corners[i]) < 1e-6, "{p:?}");
        }
        let inv = h.inverse().unwrap();
        let q = inv.apply(h.apply(Point::new(1.3, 3.7)));
        assert!(q.dist(Point::new(1.3, 3.7)) < 1e-9);
        let (dx, dy) = h.jacobian(Point::new(2.5, 2.5));
        assert!(dx.cross(dy) > 0.0);
    }

    #[test]
    fn lines() {
        let a = Line::through(Point::new(0.0, 0.0), Point::new(10.0, 0.0)).unwrap();
        let b = Line::through(Point::new(3.0, -5.0), Point::new(3.0, 5.0)).unwrap();
        let p = a.intersect(&b).unwrap();
        assert!(p.dist(Point::new(3.0, 0.0)) < 1e-9);
        assert!((a.distance(Point::new(1.0, 2.0)) - 2.0).abs() < 1e-12);
        let fit =
            Line::fit(&[Point::new(0.0, 1.0), Point::new(1.0, 2.0), Point::new(2.0, 3.0)]).unwrap();
        assert!(fit.distance(Point::new(5.0, 6.0)).abs() < 1e-9);
    }

    #[test]
    fn quads() {
        let q = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        assert!(inside_convex(&q, Point::new(2.0, 2.0)));
        assert!(!inside_convex(&q, Point::new(5.0, 2.0)));
        assert!(signed_area2(&q) > 0.0);
    }
}
