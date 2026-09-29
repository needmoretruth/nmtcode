//! The sampling grid: a homography from the finders, refined by the reference marks
//! (specification 5.6) on a symbol with a side of 48 or more.
//!
//! Each mark is predicted from the homography and the marks already located around it, and
//! located by its outer edges: the crossings between the mark's dark 3 × 3 square and the light
//! modules next to it, searched within one module of the prediction (5.6.1 reader rules). The
//! light centre is not used. The marks and the finder centres form a mesh of displacements; a
//! mark whose displacement differs from its located neighbours by more than half a module is
//! dropped, and missing nodes take the mean of their neighbours.

use nmtcode_symbol::Layout;

use crate::LumaImage;
use crate::geom::{Homography, Point};
use crate::num::count_f64;

/// Module coordinates → image pixels.
#[derive(Clone, Debug)]
pub(crate) struct Mapping {
    pub g: Homography,
    mesh: Option<Mesh>,
}

#[derive(Clone, Debug)]
struct Mesh {
    xs: Vec<f64>,
    ys: Vec<f64>,
    /// Displacement per node, row-major (ys × xs).
    disp: Vec<Point>,
}

impl Mesh {
    fn node(&self, i: usize, j: usize) -> Point {
        self.disp.get(j * self.xs.len() + i).copied().unwrap_or_default()
    }

    /// Bilinear interpolation of the displacements, constant beyond the outer nodes.
    fn at(&self, u: f64, v: f64) -> Point {
        let (i, fu) = cell(&self.xs, u);
        let (j, fv) = cell(&self.ys, v);
        let top = self.node(i, j).lerp(self.node(i + 1, j), fu);
        let bottom = self.node(i, j + 1).lerp(self.node(i + 1, j + 1), fu);
        top.lerp(bottom, fv)
    }
}

/// The interval of `lines` holding `v` and the fraction within it, clamped to the ends.
fn cell(lines: &[f64], v: f64) -> (usize, f64) {
    let n = lines.len();
    if n < 2 {
        return (0, 0.0);
    }
    let first = lines.first().copied().unwrap_or(0.0);
    let last = lines.last().copied().unwrap_or(0.0);
    if v <= first {
        return (0, 0.0);
    }
    if v >= last {
        return (n - 2, 1.0);
    }
    let i = lines.partition_point(|&x| x <= v).saturating_sub(1).min(n - 2);
    let (a, b) = (lines.get(i).copied().unwrap_or(0.0), lines.get(i + 1).copied().unwrap_or(1.0));
    (i, ((v - a) / (b - a).max(1e-9)).clamp(0.0, 1.0))
}

impl Mapping {
    /// A mapping by the homography alone.
    pub(crate) fn plain(g: Homography) -> Self {
        Self { g, mesh: None }
    }

    /// The image point of module coordinates (`u`, `v`).
    pub(crate) fn map(&self, u: f64, v: f64) -> Point {
        let p = self.g.apply(Point::new(u, v));
        match &self.mesh {
            Some(mesh) => p.add(mesh.at(u, v)),
            None => p,
        }
    }

    /// The image steps of one module in u and in v at (`u`, `v`).
    pub(crate) fn jacobian(&self, u: f64, v: f64) -> (Point, Point) {
        let h = 0.5;
        let du = self.map(u + h, v).sub(self.map(u - h, v));
        let dv = self.map(u, v + h).sub(self.map(u, v - h));
        (du, dv)
    }
}

/// Levels of the dark and light modules around a mark, from its 5 × 5 neighbourhood.
struct Local {
    threshold: f64,
    light: [bool; 25],
}

/// Samples the 5 × 5 modules around mark centre (`cx`, `cy`) through `map` + `offset`.
fn local(
    image: &LumaImage,
    map: &dyn Fn(f64, f64) -> Point,
    cx: f64,
    cy: f64,
    min_contrast: f64,
) -> Option<Local> {
    let mut values = [0.0f64; 25];
    for (k, v) in values.iter_mut().enumerate() {
        let dx = count_f64(k % 5) - 2.0;
        let dy = count_f64(k / 5) - 2.0;
        *v = image.sample(map(cx + dx + 0.5, cy + dy + 0.5));
    }
    let ring: Vec<f64> = (0..25)
        .filter(|&k| {
            let (x, y) = (k % 5, k / 5);
            (1..=3).contains(&x) && (1..=3).contains(&y) && !(x == 2 && y == 2)
        })
        .filter_map(|k| values.get(k).copied())
        .collect();
    let dark = ring.iter().sum::<f64>() / count_f64(ring.len().max(1));
    let mut outer: Vec<f64> = (0..25)
        .filter(|&k| {
            let (x, y) = (k % 5, k / 5);
            x == 0 || y == 0 || x == 4 || y == 4
        })
        .filter_map(|k| values.get(k).copied())
        .collect();
    outer.sort_by(|a, b| b.total_cmp(a));
    let light = outer.iter().take(4).sum::<f64>() / 4.0;
    if light - dark < min_contrast {
        return None;
    }
    let threshold = crate::light::midpoint(dark, light);
    if ring.iter().any(|&v| v >= threshold) {
        return None;
    }
    let mut flags = [false; 25];
    for (f, v) in flags.iter_mut().zip(values.iter()) {
        *f = *v >= threshold;
    }
    Some(Local { threshold, light: flags })
}

/// The crossing of `threshold` nearest to 0 along `profile(d)` for d in [−1, 1] module, with the
/// dark side at smaller d when `dark_first`.
fn crossing(profile: &dyn Fn(f64) -> f64, threshold: f64, dark_first: bool) -> Option<f64> {
    const N: usize = 21;
    let mut values = [0.0f64; N];
    for (i, v) in values.iter_mut().enumerate() {
        *v = profile(-1.0 + 0.1 * count_f64(i));
    }
    let mut best: Option<f64> = None;
    for i in 0..N - 1 {
        let (a, b) = (values.get(i).copied()?, values.get(i + 1).copied()?);
        let rising = a < threshold && b >= threshold;
        let falling = a >= threshold && b < threshold;
        if (dark_first && rising) || (!dark_first && falling) {
            let d = -1.0 + 0.1 * (count_f64(i) + (threshold - a) / (b - a));
            if best.is_none_or(|x| d.abs() < x.abs()) {
                best = Some(d);
            }
        }
    }
    best.filter(|d| d.abs() < 0.9)
}

/// Locates the mark centred on module (`cx`, `cy`) through `map`: its offset in modules along
/// u and along v from where `map` puts it, each `None` when no outer edge is visible.
fn locate(
    image: &LumaImage,
    map: &dyn Fn(f64, f64) -> Point,
    cx: f64,
    cy: f64,
    min_contrast: f64,
) -> Option<(Option<f64>, Option<f64>)> {
    let loc = local(image, map, cx, cy, min_contrast)?;
    let light = |dx: usize, dy: usize| loc.light.get(dy * 5 + dx).copied().unwrap_or(false);
    let (mut su, mut nu, mut sv, mut nv) = (0.0, 0usize, 0.0, 0usize);
    for r in 1..=3usize {
        let v = cy + count_f64(r) - 2.0 + 0.5;
        // Left edge at u = cx − 1, light at smaller u.
        if light(0, r) {
            let edge = cx - 1.0;
            if let Some(d) = crossing(&|d| image.sample(map(edge + d, v)), loc.threshold, false) {
                su += d;
                nu += 1;
            }
        }
        if light(4, r) {
            let edge = cx + 2.0;
            if let Some(d) = crossing(&|d| image.sample(map(edge + d, v)), loc.threshold, true) {
                su += d;
                nu += 1;
            }
        }
        let u = cx + count_f64(r) - 2.0 + 0.5;
        if light(r, 0) {
            let edge = cy - 1.0;
            if let Some(d) = crossing(&|d| image.sample(map(u, edge + d)), loc.threshold, false) {
                sv += d;
                nv += 1;
            }
        }
        if light(r, 4) {
            let edge = cy + 2.0;
            if let Some(d) = crossing(&|d| image.sample(map(u, edge + d)), loc.threshold, true) {
                sv += d;
                nv += 1;
            }
        }
    }
    let du = (nu > 0).then(|| su / count_f64(nu));
    let dv = (nv > 0).then(|| sv / count_f64(nv));
    Some((du, dv))
}

/// The mapping of a symbol of `layout` with finder homography `g`: `g` alone when both sides
/// are below 48, otherwise `g` refined by the reference marks. `finders_present[k]` says which
/// corner finders were found (TL, TR, BL, BR); their centres are fixed nodes of the mesh.
// Prediction, location, outlier test and filling of the mesh, in one pass over the nodes.
#[allow(clippy::too_many_lines)]
pub(crate) fn mapping(
    image: &LumaImage,
    g: Homography,
    layout: &Layout,
    finders_present: [bool; 4],
    min_contrast: f64,
) -> Mapping {
    if layout.reference_mark_count() == 0 {
        return Mapping::plain(g);
    }
    let xl = layout.x_lines();
    let yl = layout.y_lines();
    let (nx, ny) = (xl.len(), yl.len());
    let xs: Vec<f64> = xl.iter().map(|&v| f64::from(v) + 0.5).collect();
    let ys: Vec<f64> = yl.iter().map(|&v| f64::from(v) + 0.5).collect();
    let mut disp = vec![Point::default(); nx * ny];
    let mut known = vec![false; nx * ny];
    let corner_nodes = [(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)];
    for (k, &(i, j)) in corner_nodes.iter().enumerate() {
        if finders_present.get(k).copied().unwrap_or(false)
            && let Some(slot) = known.get_mut(j * nx + i)
        {
            *slot = true;
        }
    }
    // Breadth-first order from the known corners over the node grid (8-neighbourhood).
    let mut order: Vec<usize> = Vec::with_capacity(nx * ny);
    let mut seen = known.clone();
    let mut frontier: Vec<usize> =
        (0..nx * ny).filter(|&k| known.get(k).copied().unwrap_or(false)).collect();
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for &k in &frontier {
            let (i, j) = (k % nx, k / nx);
            for dj in -1i64..=1 {
                for di in -1i64..=1 {
                    let (Ok(ni), Ok(nj)) = (
                        usize::try_from(i64::try_from(i).unwrap_or(0) + di),
                        usize::try_from(i64::try_from(j).unwrap_or(0) + dj),
                    ) else {
                        continue;
                    };
                    if ni >= nx || nj >= ny {
                        continue;
                    }
                    let nk = nj * nx + ni;
                    if seen.get(nk).copied() == Some(false) {
                        if let Some(s) = seen.get_mut(nk) {
                            *s = true;
                        }
                        order.push(nk);
                        next.push(nk);
                    }
                }
            }
        }
        frontier = next;
    }
    let neighbours = |k: usize, known: &[bool], disp: &[Point]| -> Vec<Point> {
        let (i, j) = (k % nx, k / nx);
        let mut out = Vec::new();
        for dj in -1i64..=1 {
            for di in -1i64..=1 {
                if di == 0 && dj == 0 {
                    continue;
                }
                let (Ok(ni), Ok(nj)) = (
                    usize::try_from(i64::try_from(i).unwrap_or(0) + di),
                    usize::try_from(i64::try_from(j).unwrap_or(0) + dj),
                ) else {
                    continue;
                };
                if ni >= nx || nj >= ny {
                    continue;
                }
                let nk = nj * nx + ni;
                if known.get(nk).copied() == Some(true) {
                    out.push(disp.get(nk).copied().unwrap_or_default());
                }
            }
        }
        out
    };
    let mean = |v: &[Point]| -> Point {
        if v.is_empty() {
            Point::default()
        } else {
            v.iter().fold(Point::default(), |s, p| s.add(*p)).scale(1.0 / count_f64(v.len()))
        }
    };
    for &k in &order {
        let (i, j) = (k % nx, k / nx);
        let is_corner = (i == 0 || i == nx - 1) && (j == 0 || j == ny - 1);
        if is_corner {
            continue;
        }
        let (Some(&cx), Some(&cy)) = (xl.get(i), yl.get(j)) else { continue };
        let near = neighbours(k, &known, &disp);
        let predicted = mean(&near);
        let map = |u: f64, v: f64| g.apply(Point::new(u, v)).add(predicted);
        let Some((du, dv)) = locate(image, &map, f64::from(cx), f64::from(cy), min_contrast) else {
            continue;
        };
        if du.is_none() && dv.is_none() {
            continue;
        }
        let centre = Point::new(f64::from(cx) + 0.5, f64::from(cy) + 0.5);
        let (ju, jv) = g.jacobian(centre);
        let d = predicted.add(ju.scale(du.unwrap_or(0.0))).add(jv.scale(dv.unwrap_or(0.0)));
        let pitch = f64::midpoint(ju.norm(), jv.norm());
        if !near.is_empty() && d.dist(predicted) > 0.5 * pitch + near_spread(&near) {
            continue;
        }
        if let (Some(slot), Some(flag)) = (disp.get_mut(k), known.get_mut(k)) {
            *slot = d;
            *flag = true;
        }
    }
    // Fill the nodes that were not located from their neighbours.
    for _ in 0..nx.max(ny) {
        let mut changed = false;
        let snapshot = known.clone();
        for k in 0..nx * ny {
            if snapshot.get(k).copied() == Some(true) {
                continue;
            }
            let near = neighbours(k, &snapshot, &disp);
            if !near.is_empty() {
                if let (Some(slot), Some(flag)) = (disp.get_mut(k), known.get_mut(k)) {
                    *slot = mean(&near);
                    *flag = true;
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Mapping { g, mesh: Some(Mesh { xs, ys, disp }) }
}

/// Spread of neighbour displacements: the prediction is only as sure as they agree.
fn near_spread(near: &[Point]) -> f64 {
    if near.len() < 2 {
        return 0.0;
    }
    let c = near.iter().fold(Point::default(), |s, p| s.add(*p)).scale(1.0 / count_f64(near.len()));
    near.iter().map(|p| p.dist(c)).fold(0.0, f64::max)
}
