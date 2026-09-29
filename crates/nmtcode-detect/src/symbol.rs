//! One symbol candidate, from its finders to its module grid (specification 5.11 and 2.7).
//!
//! 1. The finder corners give a homography of the symbol in coordinates normalised to its
//!    width and height, and the finders' own sizes in those coordinates give the finder
//!    estimates Ŵ and Ĥ (5.11 step 4).
//! 2. Copy A is read relative to TL and copy B relative to BR, when those finders were found
//!    (2.7 step 1), and decoded with the size tolerance of 5.11 as part of step 3.
//! 3. The chosen word's area is checked against the reader's largest area (2.7 step 5).
//! 4. With W and H known, the homography is fitted to the exact module positions of every
//!    finder corner, and a copy that did not decode is read again (5.11 step 3).
//! 5. The reference marks refine the grid (5.6, 5.11 step 7).
//! 6. W and H are confirmed against W ± 4 and H ± 4 on the module edges (5.11 step 4).
//! 7. Every module is sampled and decided.

use nmtcode_core::{Error, FormatSample, FormatWord, decode_format_with};
use nmtcode_symbol::{FORMAT_BITS, FORMAT_COPY_A, Layout, ModuleClass};

use crate::finder::{BL, BR, Finder, LOCAL, TL, TR, local_corner, outer_corner};
use crate::geom::{Homography, Line, Point};
use crate::group::Candidate;
use crate::marks::{Mapping, mapping};
use crate::num::count_f64;
use crate::sample::sample;
use crate::{DetectOptions, Found, LumaImage, Rejected};

/// Bits of a format copy with a confidence below this (0.5 is certain) may be erased.
const FORMAT_ERASURE_MARGIN: f64 = 0.15;

/// What reading one candidate gave.
pub(crate) enum Outcome {
    /// A symbol whose format word decoded and whose size was confirmed.
    Found(Found, [Point; 4]),
    /// A symbol that failed a check before any data module was read.
    Rejected(Rejected),
    /// Not a symbol after all: the finders do not make a plausible geometry.
    Nothing,
}

/// Finder point `m` (0 = (0, 0), 1 = (5, 0), 2 = (5, 5), 3 = (0, 5)).
fn finder_point(m: usize) -> Point {
    LOCAL.get(m % 4).copied().unwrap_or_default()
}

/// The image point of finder corner `m` of a member.
fn member_corner(f: &Finder, transform: usize, m: usize) -> Point {
    f.corner(local_corner(transform, m))
}

/// The members of a candidate with their finders.
struct Members<'a> {
    /// By kind: the finder and its transform.
    at: [Option<(&'a Finder, usize)>; 4],
    /// By kind: the outer corner the grouping predicted for a missing finder, used when the
    /// finders next to it cannot place it (two finders of one edge).
    predicted: [Option<Point>; 4],
}

impl Members<'_> {
    fn present(&self, kind: usize) -> bool {
        self.at.get(kind).is_some_and(Option::is_some)
    }

    /// Image point of finder corner `m` of the member of `kind`.
    fn corner(&self, kind: usize, m: usize) -> Option<Point> {
        let (f, t) = self.at.get(kind).copied().flatten()?;
        Some(member_corner(f, t, m))
    }

    /// Every finder corner: (kind, finder corner m, image point).
    fn all(&self) -> Vec<(usize, usize, Point)> {
        let mut out = Vec::with_capacity(16);
        for kind in 0..4 {
            for m in 0..4 {
                if let Some(p) = self.corner(kind, m) {
                    out.push((kind, m, p));
                }
            }
        }
        out
    }
}

/// The outer corner of the symbol at `kind`: the finder's own corner, or, for a missing
/// finder, the meeting point of the symbol's two outer edges there, each traced from the finder
/// next to it along the boundary of the quiet zone (see [`trace`]), or, when a trace fails, the
/// extension of that finder's outer side.
fn outer(image: &LumaImage, members: &Members<'_>, kind: usize) -> Option<Point> {
    if members.present(kind) {
        return members.corner(kind, outer_corner(kind));
    }
    // (neighbour, its outer corner m, its corner m along the edge toward the missing corner)
    let (vertical, horizontal) = match kind {
        TL => ((BL, 3, 0), (TR, 1, 0)),
        TR => ((BR, 2, 1), (TL, 0, 1)),
        BL => ((TL, 0, 3), (BR, 2, 3)),
        _ => ((TR, 1, 2), (BL, 3, 2)),
    };
    let edge = |(k, a, b): (usize, usize, usize)| -> Option<Line> {
        let (f, _) = members.at.get(k).copied().flatten()?;
        let (p0, p1) = (members.corner(k, a)?, members.corner(k, b)?);
        trace(image, f, p0, p1).map(|(line, _)| line).or_else(|| Line::through(p0, p1))
    };
    match (edge(vertical), edge(horizontal)) {
        (Some(v), Some(h)) => v.intersect(&h),
        _ => members.predicted.get(kind).copied().flatten(),
    }
}

/// The symbol's outer edge from a finder's outer corner `from` in the direction of that
/// finder's corner `toward` along the edge: the boundary between the quiet zone and the
/// outermost row or column of modules. Along the edge, every module, the outermost
/// dark-to-light crossing within 2.5 modules inside and 2 outside is found; half the edge
/// modules are dark, so the outermost crossings lie on the edge and the others inside it. The
/// line is fitted to the outer envelope of the crossings, and returned with the point of the
/// line level with the last crossing: where the edge ends, at the far corner. `None` with fewer
/// than 8 crossings.
pub(crate) fn trace(
    image: &LumaImage,
    finder: &Finder,
    from: Point,
    toward: Point,
) -> Option<(Line, Point)> {
    let along = toward.sub(from);
    let len = along.norm();
    if len < 1e-6 {
        return None;
    }
    let pitch = len / 5.0;
    let mut dir = along.scale(1.0 / len);
    let initial = dir;
    // Outward: away from the finder's centre.
    let side = Point::new(-dir.y, dir.x);
    let outward = if side.dot(finder.centre().sub(from)) > 0.0 { -1.0 } else { 1.0 };
    let threshold = finder.threshold();
    let origin = from;
    let mut points: Vec<Point> = Vec::new();
    let mut last_hit = 0u32;
    // At most 4108 modules along an edge.
    for step in 6..4200u32 {
        let normal = Point::new(-dir.y, dir.x).scale(outward);
        let base = origin.add(dir.scale(f64::from(step) * pitch));
        if base.x < -1.0
            || base.y < -1.0
            || base.x > f64::from(image.width) + 1.0
            || base.y > f64::from(image.height) + 1.0
        {
            break;
        }
        let mut prev: Option<f64> = None;
        let mut outermost: Option<f64> = None;
        for i in 0..46 {
            let d = -2.5 + 0.1 * f64::from(i);
            let v = image.sample(base.add(normal.scale(d * pitch)));
            if let Some(pv) = prev
                && pv < threshold
                && v >= threshold
            {
                outermost = Some(d - 0.1 + 0.1 * (threshold - pv) / (v - pv));
            }
            prev = Some(v);
        }
        if let Some(d) = outermost {
            points.push(base.add(normal.scale(d * pitch)));
            last_hit = step;
            // Follow the edge: every 8 crossings from 16 on, the direction is fitted again
            // through the finder's corner, within 3° of the finder's side, so a small error in
            // that side's direction does not carry the search off a long edge.
            if points.len() >= 16
                && points.len().is_multiple_of(8)
                && let Some(d) = pinned_direction(&points, from, dir, outward, pitch)
                && d.dot(initial) > 3.0f64.to_radians().cos()
            {
                dir = d;
            }
        }
        // Past the far corner the quiet zone gives no crossing: stop after 6 empty modules.
        if points.len() >= 8 && step > last_hit + 6 {
            break;
        }
    }
    if points.len() < 8 {
        return None;
    }
    let line = envelope(&points, dir, outward, pitch)?;
    let far = points
        .iter()
        .copied()
        .max_by(|a, b| a.sub(from).dot(dir).total_cmp(&b.sub(from).dot(dir)))?;
    let end = line.point.add(line.dir.scale(line.along(far)));
    Some((line, end))
}

/// The direction of the line through `from` along the outer envelope of `points` (see
/// [`envelope`]), oriented like `dir`.
fn pinned_direction(
    points: &[Point],
    from: Point,
    dir: Point,
    outward: f64,
    pitch: f64,
) -> Option<Point> {
    let mut d = dir;
    for _ in 0..4 {
        let left = Point::new(-d.y, d.x);
        let kept: Vec<Point> = points
            .iter()
            .map(|p| p.sub(from))
            .filter(|v| left.dot(*v) * outward >= -0.4 * pitch)
            .collect();
        if kept.len() < 8 {
            return None;
        }
        // Principal direction of the vectors from `from`, not centred.
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for v in &kept {
            sxx += v.x * v.x;
            sxy += v.x * v.y;
            syy += v.y * v.y;
        }
        let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let next = Point::new(angle.cos(), angle.sin());
        d = if next.dot(dir) < 0.0 { next.scale(-1.0) } else { next };
    }
    d.is_finite().then_some(d)
}

/// The line along the outer envelope of edge crossings: fit, keep the crossings within 0.4
/// module inside the line, refit. Outward is the left of `dir` times `outward`.
fn envelope(points: &[Point], dir: Point, outward: f64, pitch: f64) -> Option<Line> {
    let mut line = Line::fit(points)?;
    for _ in 0..5 {
        let flip = if line.dir.dot(dir) < 0.0 { -1.0 } else { 1.0 };
        let out = |p: &Point| line.distance(*p) * flip * outward;
        let kept: Vec<Point> = points.iter().copied().filter(|p| out(p) >= -0.4 * pitch).collect();
        if kept.len() < 8 {
            break;
        }
        line = Line::fit(&kept)?;
    }
    Some(line)
}

/// Normalised coordinates of finder corner `m` of `kind` with finder fractions `a` = 5 / W and
/// `b` = 5 / H.
fn normalised(kind: usize, m: usize, a: f64, b: f64) -> Point {
    let p = finder_point(m);
    let (ox, oy) = match kind {
        TL => (0.0, 0.0),
        TR => (1.0 - a, 0.0),
        BL => (0.0, 1.0 - b),
        _ => (1.0 - a, 1.0 - b),
    };
    Point::new(ox + p.x * a / 5.0, oy + p.y * b / 5.0)
}

/// Module coordinates of finder corner `m` of `kind` in a `w` × `h` symbol.
fn module_point(kind: usize, m: usize, w: f64, h: f64) -> Point {
    let p = finder_point(m);
    let (ox, oy) = match kind {
        TL => (0.0, 0.0),
        TR => (w - 5.0, 0.0),
        BL => (0.0, h - 5.0),
        _ => (w - 5.0, h - 5.0),
    };
    Point::new(ox + p.x, oy + p.y)
}

/// The finder estimates: the homography of normalised coordinates and (a, b) = (5 / Ŵ, 5 / Ĥ).
fn estimate(image: &LumaImage, members: &Members<'_>) -> Option<(Homography, f64, f64)> {
    let corners = [
        outer(image, members, TL)?,
        outer(image, members, TR)?,
        outer(image, members, BR)?,
        outer(image, members, BL)?,
    ];
    let unit =
        [Point::new(0.0, 0.0), Point::new(1.0, 0.0), Point::new(1.0, 1.0), Point::new(0.0, 1.0)];
    let mut g = Homography::fit(&unit, &corners)?;
    let all = members.all();
    // The outer corner of a missing finder, where the outer sides of its neighbours meet,
    // holds the far end of the fit in place.
    let missing: Vec<(Point, Point)> = (0..4)
        .filter(|&k| !members.present(k))
        .filter_map(|k| {
            Some((
                unit.get([0usize, 1, 3, 2].get(k).copied()?).copied()?,
                corners.get([0usize, 1, 3, 2].get(k).copied()?).copied()?,
            ))
        })
        .collect();
    let mut dst: Vec<Point> = all.iter().map(|&(_, _, p)| p).collect();
    dst.extend(missing.iter().map(|&(_, p)| p));
    let (mut a, mut b) = fractions(&g, members)?;
    for _ in 0..4 {
        let mut src: Vec<Point> = all.iter().map(|&(k, m, _)| normalised(k, m, a, b)).collect();
        src.extend(missing.iter().map(|&(u, _)| u));
        g = Homography::fit(&src, &dst)?;
        (a, b) = fractions(&g, members)?;
    }
    Some((g, a, b))
}

/// The mean width and height of the member finders in the normalised coordinates of `g`.
fn fractions(g: &Homography, members: &Members<'_>) -> Option<(f64, f64)> {
    let inv = g.inverse()?;
    let (mut sa, mut sb, mut n) = (0.0, 0.0, 0usize);
    for kind in 0..4 {
        if !members.present(kind) {
            continue;
        }
        let q = |m: usize| members.corner(kind, m).map(|p| inv.apply(p));
        let (q0, q1, q2, q3) = (q(0)?, q(1)?, q(2)?, q(3)?);
        sa += f64::midpoint((q1.x - q0.x).abs(), (q2.x - q3.x).abs());
        sb += f64::midpoint((q3.y - q0.y).abs(), (q2.y - q1.y).abs());
        n += 1;
    }
    let (a, b) = (sa / count_f64(n.max(1)), sb / count_f64(n.max(1)));
    (a > 1e-4 && b > 1e-4 && a < 0.5 && b < 0.5).then_some((a, b))
}

/// The size tolerance of 5.11: a side of `n` modules agrees with the finder estimate
/// `estimate` when they differ by at most max(4, n / 10) modules.
pub(crate) fn within_tolerance(n: u32, estimate: f64) -> bool {
    let n = f64::from(n);
    (n - estimate).abs() <= (n / 10.0).max(4.0)
}

/// Reference cells per copy, times c² (chapter 7, 7.5).
const REFERENCE_MODULES_PER_COPY: usize = 32;
/// The least number of colour codewords of a colour-profile-1 symbol (chapter 7, 7.8.2).
const COLOUR_MIN_CODEWORDS: usize = 16;

/// The number of colour codewords of `layout` with chroma cells of `c` × `c` modules (7.5,
/// 7.8.2): the cells aligned to multiples of c whose modules are all data modules, less the two
/// reference copies, in whole bytes. The same count as the decoder of the `nmtcode` crate.
fn colour_codewords(layout: &Layout, c: u32) -> usize {
    let c = c.max(1);
    let cells = (0..layout.height() / c)
        .flat_map(|j| (0..layout.width() / c).map(move |i| (i, j)))
        .filter(|&(i, j)| {
            (0..c).all(|dy| {
                (0..c).all(|dx| {
                    layout.module_class(c * i + dx, c * j + dy) == Some(ModuleClass::Data)
                })
            })
        })
        .count();
    let side = usize::try_from(c).unwrap_or(usize::MAX);
    let reference = 2 * (REFERENCE_MODULES_PER_COPY / (side * side).max(1));
    cells.saturating_sub(reference) / 8
}

/// Step 3 of 2.7 for a word with valid fields: W and H agree with the finder estimates within
/// the tolerance of 5.11, and a colour-profile-1 word leaves at least 16 colour codewords.
fn plausible(word: &FormatWord, w_est: f64, h_est: f64, max_area: u64) -> bool {
    if !within_tolerance(word.width(), w_est) || !within_tolerance(word.height(), h_est) {
        return false;
    }
    if word.colour_profile() == 0 {
        return true;
    }
    // A colour word above the largest area is left for step 5 to reject; its layout is not built.
    if u64::from(word.width()) * u64::from(word.height()) > max_area {
        return true;
    }
    Layout::new(word.width(), word.height()).is_ok_and(|layout| {
        colour_codewords(&layout, word.chroma_cell_side()) >= COLOUR_MIN_CODEWORDS
    })
}

/// Modules on a side of the patch read around a finder for its format copy: the copy lies
/// within 10 modules of the corner, and two more give its outer modules their neighbours.
const PATCH: usize = 12;

/// One copy read at the symbol corner of `kind` (TL for copy A, BR for copy B): the raw word
/// and the same word with its least certain bits (at most 4) erased. The corner's modules are
/// decided with the equaliser of the module sampling, the finder, separator and quiet zone
/// entering with their known values; blur of half a module otherwise flips format bits whose
/// neighbours are all of the other colour. `at(u, v)` maps symbol coordinates to the image.
fn read_copy(
    image: &LumaImage,
    at: &dyn Fn(f64, f64) -> Point,
    kind: usize,
    w: f64,
    h: f64,
) -> (FormatSample, FormatSample) {
    let corner = if kind == TL {
        nmtcode_symbol::Corner::TopLeft
    } else {
        nmtcode_symbol::Corner::BottomRight
    };
    let finder = |x: u32, y: u32| {
        let (fx, fy) = if kind == TL { (x, y) } else { (4 - x.min(4), 4 - y.min(4)) };
        corner.module(fx, fy) == Some(true)
    };
    // Corner coordinates: copy B is copy A turned by 180° about the symbol's centre (5.5).
    let local = |u: f64, v: f64| if kind == TL { at(u, v) } else { at(w - u, h - v) };
    let patch = crate::sample::corner_patch(image, &local, &finder, PATCH);
    let mut bits = 0u64;
    let mut margins: Vec<(f64, usize)> = Vec::with_capacity(FORMAT_BITS);
    for (i, &(x, y)) in FORMAT_COPY_A.iter().enumerate() {
        let index = usize::try_from(y).unwrap_or(0) * PATCH + usize::try_from(x).unwrap_or(0);
        let (dark, margin) = patch.get(index).copied().unwrap_or((false, 0.0));
        bits |= u64::from(dark) << (FORMAT_BITS - 1 - i);
        margins.push((margin, i));
    }
    margins.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut erasures = 0u64;
    for &(m, i) in margins.iter().take(4) {
        if m < FORMAT_ERASURE_MARGIN {
            erasures |= 1u64 << (FORMAT_BITS - 1 - i);
        }
    }
    (FormatSample::new(bits), FormatSample::with_erasures(bits, erasures))
}

/// Samples of both copies (`None` where the finder was not found) of a `w` × `h` symbol under
/// the map `at` of symbol coordinates.
fn read_copies(
    image: &LumaImage,
    members: &Members<'_>,
    at: &dyn Fn(f64, f64) -> Point,
    w: f64,
    h: f64,
) -> [Option<(FormatSample, FormatSample)>; 2] {
    let a = members.present(TL).then(|| read_copy(image, at, TL, w, h));
    let b = members.present(BR).then(|| read_copy(image, at, BR, w, h));
    [a, b]
}

/// Decodes the format word from `copies`, first without erasures and, when no copy decodes,
/// with them. Returns the result and which copies were given.
fn decode(
    copies: &[Option<(FormatSample, FormatSample)>; 2],
    accept: &dyn Fn(&FormatWord) -> bool,
) -> Result<nmtcode_core::FormatDecoded, Error> {
    let plain = copies.map(|c| c.map(|(p, _)| p));
    let first = decode_format_with(plain, accept);
    match first {
        Err(Error::FormatUnreadable) => {
            let erased = copies.map(|c| c.map(|(_, e)| e));
            if erased == plain {
                return first;
            }
            decode_format_with(erased, accept)
        }
        other => other,
    }
}

/// Contrast across the module edges less contrast across the module centres of a grid of `n`
/// modules along `at(t, c)`, t from 0 to 1 along the grid and c from 0 to 1 across it, on
/// `lines` lines, between the finders (modules 5 to n − 6). The image changes across a true
/// module edge half the time and never across a true centre, so the true grid scores highest.
fn edge_score(
    image: &LumaImage,
    at: &dyn Fn(f64, f64) -> Point,
    n: u32,
    lines: u32,
    band: (f64, f64),
) -> f64 {
    let nf = f64::from(n);
    let delta = 0.3 / nf;
    let (mut sum, mut count) = (0.0, 0usize);
    for li in 0..lines {
        let c = band.0 + (band.1 - band.0) * (f64::from(li) + 0.5) / f64::from(lines);
        let v = |t: f64| image.sample(at(t, c));
        for i in 5..n.saturating_sub(5) {
            let edge = f64::from(i) / nf;
            let centre = (f64::from(i) + 0.5) / nf;
            sum += (v(edge + delta) - v(edge - delta)).abs();
            sum -= (v(centre + delta) - v(centre - delta)).abs();
            count += 1;
        }
    }
    if count == 0 { 0.0 } else { sum / count_f64(count) }
}

/// Confirms `w` against `w − 4` and `w + 4` (`horizontal`) or `h` against `h ± 4` on the fitted
/// grid: the claimed size must have the higher [`edge_score`] on the module edges between the
/// two finders of each side whose finders were found, in the 12 modules along that side
/// (5.11 step 4, exact size).
fn confirm(
    image: &LumaImage,
    map: &Mapping,
    w: u32,
    h: u32,
    horizontal: bool,
    present: [bool; 4],
) -> bool {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let at =
        |t: f64, c: f64| if horizontal { map.map(t * wf, c * hf) } else { map.map(c * wf, t * hf) };
    let (along, across) = if horizontal { (w, h) } else { (h, w) };
    let depth = (12.0 / f64::from(across)).min(0.5);
    // The two sides of this direction and their finders: top (TL, TR) and bottom (BL, BR) for
    // widths, left (TL, BL) and right (TR, BR) for heights.
    let pairs = if horizontal { [(TL, TR), (BL, BR)] } else { [(TL, BL), (TR, BR)] };
    let found = |k: usize| present.get(k).copied().unwrap_or(false);
    let bands: Vec<(f64, f64)> = pairs
        .iter()
        .zip([(0.0, depth), (1.0 - depth, 1.0)])
        .filter(|&(&(a, b), _)| found(a) && found(b))
        .map(|(_, band)| band)
        .collect();
    if bands.is_empty() {
        return false;
    }
    let lines = across.min(12);
    let score =
        |n: u32| bands.iter().map(|&band| edge_score(image, &at, n, lines, band)).sum::<f64>();
    let own = score(along);
    own > score(along + 4) && own > score(along.saturating_sub(4).max(11))
}

/// The valid sides within the 5.11 tolerance of `estimate`, best first by [`edge_score`] on the
/// normalised map `g` (`horizontal` for widths), on `bands` across the symbol: the strips along
/// the edges whose two finders were found, where `g` is surest.
fn ranked_sides(
    image: &LumaImage,
    g: &Homography,
    estimate: f64,
    horizontal: bool,
    bands: &[(f64, f64)],
) -> Vec<u32> {
    let lo = ((estimate / 1.1 - 4.0).max(20.0) / 4.0).floor();
    let hi = (estimate / 0.9 + 4.0).min(4108.0) / 4.0;
    let mut sides: Vec<(u32, f64)> = Vec::new();
    let mut n4 = lo;
    while n4 <= hi {
        let n = crate::num::floor_u32(n4) * 4;
        n4 += 1.0;
        if !nmtcode_core::is_valid_side(n) || !within_tolerance(n, estimate) {
            continue;
        }
        let at = |t: f64, c: f64| {
            if horizontal { g.apply(Point::new(t, c)) } else { g.apply(Point::new(c, t)) }
        };
        let score: f64 = bands.iter().map(|&band| edge_score(image, &at, n, 8, band)).sum();
        sides.push((n, score));
    }
    sides.sort_by(|a, b| b.1.total_cmp(&a.1));
    sides.into_iter().map(|(n, _)| n).collect()
}

/// The homography of module coordinates of a `w` × `h` symbol from every finder corner.
fn exact(image: &LumaImage, members: &Members<'_>, w: u32, h: u32) -> Option<Homography> {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let all = members.all();
    let mut src: Vec<Point> = all.iter().map(|&(k, m, _)| module_point(k, m, wf, hf)).collect();
    let mut dst: Vec<Point> = all.iter().map(|&(_, _, p)| p).collect();
    for kind in (0..4).filter(|&k| !members.present(k)) {
        if let Some(p) = outer(image, members, kind) {
            src.push(module_point(kind, outer_corner(kind), wf, hf));
            dst.push(p);
        }
    }
    Homography::fit(&src, &dst)
}

/// The transform of a finder found at `predicted` (finder corners m = 0 to 3 in order): each
/// predicted corner goes to the nearest corner of the finder.
fn transform_of(f: &Finder, predicted: &[Point; 4]) -> Option<(usize, i32)> {
    let nearest =
        |p: Point| (0..4).min_by(|&a, &b| f.corner(a).dist(p).total_cmp(&f.corner(b).dist(p)));
    let j: Vec<usize> = predicted.iter().filter_map(|&p| nearest(p)).collect();
    let (&j0, &j1, &j3) = (j.first()?, j.get(1)?, j.get(3)?);
    let sigma = if j1 == (j0 + 1) % 4 && j3 == (j0 + 3) % 4 {
        1
    } else if j1 == (j0 + 3) % 4 && j3 == (j0 + 1) % 4 {
        -1
    } else {
        return None;
    };
    Some((crate::finder::transform(j0, sigma), sigma))
}

/// The members of a candidate, owned: by kind, the finder and its transform.
type Owned = [Option<(Finder, usize)>; 4];

/// The missing finders of `owned` searched where `predict(kind, m)` puts their corners
/// m = 0 to 3; a finder found there counts when its orientation is the candidate's (`sigma`),
/// its pattern is the kind's and its outer corner faces the quiet zone.
fn search_missing(
    image: &LumaImage,
    noise: f64,
    owned: &mut Owned,
    sigma: i32,
    predict: &dyn Fn(usize, usize) -> Point,
) {
    for kind in 0..4 {
        if owned.get(kind).is_some_and(Option::is_some) {
            continue;
        }
        let predicted = [0usize, 1, 2, 3].map(|m| predict(kind, m));
        let Some(f) = crate::finder::guided(image, noise, &predicted) else { continue };
        let Some((t, s)) = transform_of(&f, &predicted) else { continue };
        let score = f.ssd.get(kind).and_then(|row| row.get(t)).copied().unwrap_or(f64::INFINITY);
        if s == sigma
            && score <= crate::group::SSD_ASSIGN
            && f.quiet_corner(local_corner(t, outer_corner(kind)))
            && let Some(slot) = owned.get_mut(kind)
        {
            *slot = Some((f, t));
        }
    }
}

/// A borrowed view of `owned`.
fn view(owned: &Owned, predicted: [Option<Point>; 4]) -> Members<'_> {
    Members { at: owned.each_ref().map(|o| o.as_ref().map(|(f, t)| (f, *t))), predicted }
}

/// The bands along the edges whose two finders were found: for widths the top (TL, TR) and
/// bottom (BL, BR) strips of `depth`, for heights the left (TL, BL) and right (TR, BR) ones.
fn edge_bands(members: &Members<'_>, horizontal: bool, depth: f64) -> Vec<(f64, f64)> {
    let pairs = if horizontal { [(TL, TR), (BL, BR)] } else { [(TL, BL), (TR, BR)] };
    let depth = depth.min(0.5);
    pairs
        .iter()
        .zip([(0.0, depth), (1.0 - depth, 1.0)])
        .filter(|&(&(a, b), _)| members.present(a) && members.present(b))
        .map(|(_, band)| band)
        .collect()
}

/// Reads candidate `candidate` of `finders`.
// The steps of 5.11 and 2.7 in their order, in one place.
#[allow(clippy::too_many_lines)]
pub(crate) fn read(
    image: &LumaImage,
    noise: f64,
    finders: &[Finder],
    candidate: &Candidate,
    options: &DetectOptions,
) -> Outcome {
    let predicted: [Option<Point>; 4] = [0usize, 1, 2, 3].map(|k| {
        let missing = candidate.members.get(k).is_some_and(Option::is_none);
        missing.then(|| candidate.outer.get(k).copied()).flatten()
    });
    let mut owned: Owned = [None, None, None, None];
    for (slot, member) in owned.iter_mut().zip(candidate.members.iter()) {
        if let Some(m) = member {
            *slot = finders.get(m.finder).map(|f| (f.clone(), m.transform));
        }
    }
    // A finder that the threshold missed is searched where the others put it.
    if owned.iter().flatten().count() < 4 {
        let Some((g_norm, a, b)) = estimate(image, &view(&owned, predicted)) else {
            return Outcome::Nothing;
        };
        search_missing(image, noise, &mut owned, candidate.sigma, &|kind, m| {
            g_norm.apply(normalised(kind, m, a, b))
        });
    }
    // 5.11: three finders at least.
    if owned.iter().flatten().count() < 3 {
        return Outcome::Nothing;
    }
    let members = view(&owned, predicted);
    let Some((g_norm, a, b)) = estimate(image, &members) else { return Outcome::Nothing };
    let (w_est, h_est) = (5.0 / a, 5.0 / b);
    if !(15.0..=4600.0).contains(&w_est) || !(15.0..=4600.0).contains(&h_est) {
        return Outcome::Nothing;
    }
    // The area of 5.11 step 6: the outer corners of the finders, the fourth completing a
    // parallelogram when missing.
    let Some(area) = nesting_area(&members) else { return Outcome::Nothing };
    let rejected = |error: Error| Outcome::Rejected(Rejected { corners: area, error });
    let accept = |word: &FormatWord| plausible(word, w_est, h_est, options.max_area);
    // 5.11 step 3: the pitch from one finder's size reaches the far format bits with about ten
    // times its error, and blur moves the edges of the one-module ring. The sizes near the
    // estimates are ranked on the module edges along the edges whose finders were found, and
    // the copies read with the homography of every finder corner at the best sizes. The copies
    // lie within 10 modules of their finders, so a size near the truth reads them; the word
    // they give must pass the tolerance of 5.11 against the finder estimates.
    let widths =
        ranked_sides(image, &g_norm, w_est, true, &edge_bands(&members, true, 12.0 / h_est));
    let heights =
        ranked_sides(image, &g_norm, h_est, false, &edge_bands(&members, false, 12.0 / w_est));
    let read_at = |members: &Members<'_>,
                   w: u32,
                   h: u32|
     -> Option<[Option<(FormatSample, FormatSample)>; 2]> {
        let g = exact(image, members, w, h)?;
        let at = |u: f64, v: f64| g.apply(Point::new(u, v));
        Some(read_copies(image, members, &at, f64::from(w), f64::from(h)))
    };
    let mut chosen: Option<FormatWord> = None;
    let mut first_error: Option<Error> = None;
    'hypotheses: for &w in widths.iter().take(3) {
        for &h in heights.iter().take(3) {
            let Some(copies) = read_at(&members, w, h) else { continue };
            match decode(&copies, &accept) {
                Ok(d) => {
                    chosen = Some(d.word);
                    break 'hypotheses;
                }
                Err(Error::FormatUnreadable) => {}
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
    }
    let Some(word) = chosen else {
        return rejected(first_error.unwrap_or(Error::FormatUnreadable));
    };
    // 2.7 step 5 and 5.11 step 5: the largest area, before any data module is read.
    if u64::from(word.width()) * u64::from(word.height()) > options.max_area {
        return rejected(Error::SizeLimit);
    }
    // With W and H known, a finder still missing is searched where the exact geometry puts
    // it, and both copies are read again with that geometry (5.11 step 3). A reading that
    // decodes to another word is a conflict; one that decodes nothing leaves the first word.
    let (w, h) = (word.width(), word.height());
    let (wf, hf) = (f64::from(w), f64::from(h));
    let g_known = exact(image, &members, w, h);
    if owned.iter().flatten().count() < 4
        && let Some(g) = g_known
    {
        search_missing(image, noise, &mut owned, candidate.sigma, &|kind, m| {
            g.apply(module_point(kind, m, wf, hf))
        });
    }
    let members = view(&owned, predicted);
    let exact_size = |word: &FormatWord| word.width() == w && word.height() == h && accept(word);
    if let Some(copies) = read_at(&members, w, h) {
        match decode(&copies, &exact_size) {
            Ok(d) if d.word != word => return rejected(Error::FormatConflict),
            Ok(_) | Err(Error::FormatUnreadable) => {}
            Err(e) => return rejected(e),
        }
    }
    let Some(g) = exact(image, &members, w, h) else { return Outcome::Nothing };
    let Ok(layout) = Layout::new(w, h) else { return rejected(Error::FormatUnreadable) };
    let contrast = members
        .at
        .iter()
        .flatten()
        .map(|(f, _)| crate::light::stored(f.border) - crate::light::stored(f.ring))
        .fold(f64::INFINITY, f64::min);
    let present =
        [members.present(TL), members.present(TR), members.present(BL), members.present(BR)];
    // The reference marks refine the grid (5.6, 5.11 step 7); a mark misplaced under heavy
    // blur can bend it instead, so the mesh is kept only when the module edges of the whole
    // symbol fit it at least as well as the finders' homography alone.
    let meshed = mapping(image, g, &layout, present, 0.3 * contrast);
    let fit = |m: &Mapping| {
        let at_x = |t: f64, c: f64| m.map(t * wf, c * hf);
        let at_y = |t: f64, c: f64| m.map(c * wf, t * hf);
        edge_score(image, &at_x, w, 16, (0.0, 1.0)) + edge_score(image, &at_y, h, 16, (0.0, 1.0))
    };
    let plain = Mapping::plain(g);
    let map = if layout.reference_mark_count() > 0 && fit(&meshed) < fit(&plain) {
        plain
    } else {
        meshed
    };
    // 5.11 step 4, exact size: after the grid is fitted and before any data module is read.
    if !confirm(image, &map, w, h, true, present) || !confirm(image, &map, w, h, false, present) {
        return rejected(Error::FormatUnreadable);
    }
    let Some(sampled) = sample(image, &map, &layout) else { return Outcome::Nothing };
    let corners = [map.map(0.0, 0.0), map.map(wf, 0.0), map.map(wf, hf), map.map(0.0, hf)];
    let found = Found {
        grid: sampled.grid,
        uncertain: sampled.uncertain,
        corners,
        mirrored: candidate.sigma < 0,
        inverted: false,
        finders: u8::try_from(owned.iter().flatten().count()).unwrap_or(4),
    };
    Outcome::Found(found, area)
}

/// The area of 5.11 step 6 in the symbol's corner order TL, TR, BR, BL.
fn nesting_area(members: &Members<'_>) -> Option<[Point; 4]> {
    let corner =
        |k: usize| if members.present(k) { members.corner(k, outer_corner(k)) } else { None };
    let (tl, tr, bl, br) = (corner(TL), corner(TR), corner(BL), corner(BR));
    let area = match (tl, tr, bl, br) {
        (Some(tl), Some(tr), Some(bl), Some(br)) => [tl, tr, br, bl],
        (None, Some(tr), Some(bl), Some(br)) => [tr.add(bl).sub(br), tr, br, bl],
        (Some(tl), None, Some(bl), Some(br)) => [tl, tl.add(br).sub(bl), br, bl],
        (Some(tl), Some(tr), None, Some(br)) => [tl, tr, br, tl.add(br).sub(tr)],
        (Some(tl), Some(tr), Some(bl), None) => [tl, tr, tr.add(bl).sub(tl), bl],
        _ => return None,
    };
    Some(area)
}
