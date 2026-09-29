//! From finders to symbol candidates.
//!
//! Two finders on one edge of a symbol share an outer edge line: the top sides of TL and TR lie
//! on the symbol's top edge, and a perspective projection keeps lines straight. A *link* joins
//! two finders whose sides lie on one line with a gap between them. Three finders of one symbol
//! always form an L: the finder at the corner of the L links along two of its sides, which
//! meet at the symbol's outer corner. The fourth finder, when found, sits where the far outer
//! sides of the other two meet.
//!
//! The geometry leaves eight ways to lay the symbol on the finders (which corner the L's corner
//! is, and whether it is mirrored); each gives every finder a kind and a transform, and the
//! classification scores of 5.3 pick the one that fits (5.11 step 2).

use crate::finder::{BL, BR, Finder, TL, TR, local_corner, outer_corner, transform};
use crate::geom::{Line, Point};

/// Largest number of finders grouped; grouping is quadratic in their number.
pub(crate) const MAX_FINDERS: usize = 256;
/// Partners kept per side and direction.
const PARTNERS: usize = 4;
/// Largest classification score of a finder under the kind and transform a symbol gives it.
pub(crate) const SSD_ASSIGN: f64 = 1.2;
/// Least margin between the best and the second-best way to lay a symbol on its finders.
const HYPOTHESIS_MARGIN: f64 = 0.5;
/// Most candidates from pairs of opposite finders.
const MAX_PAIRS: usize = 32;

/// A finder of a candidate with its role.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Member {
    /// Index into the finder list.
    pub finder: usize,
    /// The finder's transform (see [`crate::finder`]).
    pub transform: usize,
}

/// A group of three or four finders with a kind and transform for each.
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    /// The members, by kind: `members[TL]` is the TL finder, when found.
    pub members: [Option<Member>; 4],
    /// The outer corners of the outline by kind: a member's corner, or the predicted point of a
    /// missing finder.
    pub outer: [Point; 4],
    /// Sum of the classification scores of the members.
    pub score: f64,
    /// −1 for a mirrored symbol, +1 otherwise.
    pub sigma: i32,
}

impl Candidate {
    /// Number of members.
    pub(crate) fn count(&self) -> usize {
        self.members.iter().flatten().count()
    }

    /// The finder indices of the members.
    pub(crate) fn finders(&self) -> impl Iterator<Item = usize> + '_ {
        self.members.iter().flatten().map(|m| m.finder)
    }
}

/// A link from a finder, along one of its sides, to another finder.
#[derive(Clone, Copy, Debug)]
struct Link {
    other: usize,
    /// This finder's corner at the symbol corner (the end of the shared side away from `other`).
    corner: usize,
    /// The other finder's corner at its symbol corner (its end away from this finder).
    other_corner: usize,
    /// The gap between the two finders along the line, in pixels.
    gap: f64,
}

/// Links of one finder: `links[side][forward]`.
type Links = [[Vec<Link>; 2]; 4];

fn axis(f: &Finder, s: usize) -> Point {
    let d = f.corner(s + 1).sub(f.corner(s));
    let n = d.norm();
    if n > 1e-9 { d.scale(1.0 / n) } else { Point::new(1.0, 0.0) }
}

/// Tests whether side `sa` of `a` and side `sb` of `b` lie on one line with `b` beyond one end
/// of `a`'s side. Returns (forward, link) where forward means beyond corner `sa + 1`.
fn link(finders: &[Finder], ia: usize, sa: usize, ib: usize, sb: usize) -> Option<(bool, Link)> {
    let a = finders.get(ia)?;
    let b = finders.get(ib)?;
    // The shared line is an outer edge of the symbol: both sides face the quiet zone.
    if !a.quiet_side(sa) || !b.quiet_side(sb) {
        return None;
    }
    let line = a.side_line(sa)?;
    let (a0, a1) = (a.corner(sa), a.corner(sa + 1));
    let (b0, b1) = (b.corner(sb), b.corner(sb + 1));
    if axis(a, sa).cross(axis(b, sb)).abs() > 0.35 {
        return None;
    }
    let len = a0.dist(a1);
    let (t0, t1) = (line.along(b0), line.along(b1));
    let pitch = a.pitch.max(b.pitch);
    let min_gap = 6.0 * a.pitch.min(b.pitch);
    let (forward, gap) = if t0.min(t1) > len + min_gap {
        (true, t0.min(t1) - len)
    } else if t0.max(t1) < -min_gap {
        (false, -t0.max(t1))
    } else {
        return None;
    };
    let ratio = a.side_len(sa) / b.side_len(sb).max(1e-9);
    if !(0.5..=2.0).contains(&ratio) {
        return None;
    }
    // All four ends on one line.
    let fit = Line::fit(&[a0, a1, b0, b1])?;
    let tol = 0.6 * pitch + 0.01 * gap;
    if [a0, a1, b0, b1].iter().any(|p| fit.distance(*p).abs() > tol) {
        return None;
    }
    // Both squares on the same side of the line.
    let (da, db) = (fit.distance(a.centre()), fit.distance(b.centre()));
    if da * db <= 0.0 || da.abs() < 1.5 * a.pitch || db.abs() < 1.5 * b.pitch {
        return None;
    }
    let corner = if forward { sa } else { (sa + 1) % 4 };
    // The end of b's side away from a.
    let other_corner = if forward == (t1 > t0) { (sb + 1) % 4 } else { sb };
    Some((forward, Link { other: ib, corner, other_corner, gap }))
}

fn all_links(finders: &[Finder]) -> Vec<Links> {
    let mut out: Vec<Links> = finders.iter().map(|_| Links::default()).collect();
    for (ia, a) in finders.iter().enumerate() {
        for (ib, b) in finders.iter().enumerate() {
            if ia == ib {
                continue;
            }
            let ratio = a.pitch / b.pitch.max(1e-9);
            if !(0.5..=2.0).contains(&ratio) {
                continue;
            }
            let d = b.centre().sub(a.centre());
            let dn = d.norm();
            if dn < 10.0 * a.pitch.min(b.pitch) {
                continue;
            }
            let d = d.scale(1.0 / dn);
            for sa in 0..4 {
                if axis(a, sa).cross(d).abs() > 0.45 {
                    continue;
                }
                for sb in 0..4 {
                    if axis(b, sb).cross(d).abs() > 0.45 {
                        continue;
                    }
                    let Some((forward, found)) = link(finders, ia, sa, ib, sb) else { continue };
                    let Some(list) = out
                        .get_mut(ia)
                        .and_then(|l| l.get_mut(sa))
                        .and_then(|l| l.get_mut(usize::from(forward)))
                    else {
                        continue;
                    };
                    list.push(found);
                }
            }
        }
    }
    for links in &mut out {
        for per_side in links.iter_mut() {
            for list in per_side.iter_mut() {
                list.sort_by(|x, y| x.gap.total_cmp(&y.gap));
                list.truncate(PARTNERS);
            }
        }
    }
    out
}

/// Direction (+1 or −1 in corner index) from corner `j` of `f` along the side that points
/// toward `target`.
fn toward(f: &Finder, j: usize, target: Point) -> i32 {
    let c = f.corner(j);
    let next = f.corner(j + 1).sub(c);
    let prev = f.corner(j + 3).sub(c);
    let t = target.sub(c);
    let cos = |v: Point| v.dot(t) / (v.norm() * t.norm()).max(1e-12);
    if cos(next) >= cos(prev) { 1 } else { -1 }
}

/// Index step from the outer corner of `kind` to its corner toward the x-neighbour.
const fn step_x(kind: usize) -> i32 {
    match kind {
        TL | BR => 1,
        _ => -1,
    }
}

const fn flip_x(kind: usize) -> usize {
    match kind {
        TL => TR,
        TR => TL,
        BL => BR,
        _ => BL,
    }
}

const fn flip_y(kind: usize) -> usize {
    match kind {
        TL => BL,
        BL => TL,
        TR => BR,
        _ => TR,
    }
}

/// A geometric group: the four corners of a symbol outline in cyclic order, each a finder
/// (index and its corner at the outline) or only a predicted point.
#[derive(Clone, Copy)]
struct Shape {
    q: [(Option<(usize, usize)>, Point); 4],
}

/// The best way to lay a symbol on `shape`, or `None` when no way fits well enough or two fit
/// equally.
fn orient(finders: &[Finder], shape: &Shape) -> Option<Candidate> {
    let point = |i: usize| shape.q.get(i % 4).map_or(Point::default(), |c| c.1);
    let mut results: Vec<Candidate> = Vec::with_capacity(8);
    for kind0 in 0..4 {
        for b_is_x in [true, false] {
            // Edges alternate x and y around the cycle: Q0–Q1 is x when `b_is_x`.
            let kinds = if b_is_x {
                [kind0, flip_x(kind0), flip_x(flip_y(kind0)), flip_y(kind0)]
            } else {
                [kind0, flip_y(kind0), flip_x(flip_y(kind0)), flip_x(kind0)]
            };
            let mut members: [Option<Member>; 4] = [None; 4];
            let mut outer = [Point::default(); 4];
            let mut score = 0.0;
            let mut sigma_all: Option<i32> = None;
            let mut ok = true;
            for (i, (&kind, &(role, p))) in kinds.iter().zip(shape.q.iter()).enumerate() {
                if let Some(slot) = outer.get_mut(kind) {
                    *slot = p;
                }
                let Some((fi, j)) = role else { continue };
                let Some(f) = finders.get(fi) else {
                    ok = false;
                    break;
                };
                if !f.quiet_corner(j) {
                    ok = false;
                    break;
                }
                // The x-neighbour of corner i.
                let xn = if (i % 2 == 0) == b_is_x { point(i + 1) } else { point(i + 3) };
                let sigma = toward(f, j, xn) * step_x(kind);
                if sigma_all.is_some_and(|s| s != sigma) {
                    ok = false;
                    break;
                }
                sigma_all = Some(sigma);
                let m = outer_corner(kind);
                let jm = i32::try_from(j).unwrap_or(0) - sigma * i32::try_from(m).unwrap_or(0);
                let r = usize::try_from(jm.rem_euclid(4)).unwrap_or(0);
                let t = transform(r, sigma);
                debug_assert_eq!(local_corner(t, m), j);
                let s =
                    f.ssd.get(kind).and_then(|row| row.get(t)).copied().unwrap_or(f64::INFINITY);
                if s > SSD_ASSIGN {
                    ok = false;
                    break;
                }
                score += s;
                if let Some(slot) = members.get_mut(kind) {
                    *slot = Some(Member { finder: fi, transform: t });
                }
            }
            if ok && let Some(sigma) = sigma_all {
                results.push(Candidate { members, outer, score, sigma });
            }
        }
    }
    results.sort_by(|x, y| x.score.total_cmp(&y.score));
    let best = results.first()?.clone();
    if results.get(1).is_some_and(|second| second.score < best.score + HYPOTHESIS_MARGIN) {
        return None;
    }
    Some(best)
}

/// The line of the side at corner `j` of `f` that is not side `other` (the two sides at a
/// corner are side j and side j − 1).
fn side_at(f: &Finder, j: usize, first: bool) -> Option<Line> {
    f.side_line(if first { j } else { (j + 3) % 4 })
}

/// Where the far outer sides of B and C meet: the predicted outer corner of D.
fn predict_d(
    finders: &[Finder],
    b: (usize, usize, usize),
    c: (usize, usize, usize),
) -> Option<Point> {
    // (finder, outer corner, shared side)
    let far_side = |(fi, j, shared): (usize, usize, usize)| -> Option<Line> {
        let f = finders.get(fi)?;
        side_at(f, j, shared != j)
    };
    far_side(b)?.intersect(&far_side(c)?)
}

/// The outline through two finders at opposite corners: each outer side line of one meets the
/// outer side line of the other that is not parallel to it. Returns the cyclic shape.
fn diagonal(
    finders: &[Finder],
    (ia, ja): (usize, usize),
    (ib, jb): (usize, usize),
) -> Option<Shape> {
    let a = finders.get(ia)?;
    let b = finders.get(ib)?;
    let (pa, pb) = (a.corner(ja), b.corner(jb));
    let span = pa.dist(pb);
    if span < 12.0 * a.pitch.max(b.pitch) {
        return None;
    }
    let (a1, a0) = (side_at(a, ja, true)?, side_at(a, ja, false)?);
    let (b1, b0) = (side_at(b, jb, true)?, side_at(b, jb, false)?);
    // Pair each side of a with the side of b it is least parallel to.
    let (x, y) = if a1.dir.cross(b1.dir).abs() < a1.dir.cross(b0.dir).abs() {
        (a1.intersect(&b0)?, a0.intersect(&b1)?)
    } else {
        (a1.intersect(&b1)?, a0.intersect(&b0)?)
    };
    let quad = [pa, x, pb, y];
    // A convex outline with the two finders inside it, at their corners.
    let area = crate::geom::signed_area2(&quad);
    let ordered = if area > 0.0 { quad } else { [pa, y, pb, x] };
    for i in 0..4 {
        let (p0, p1, p2) = (ordered[i], ordered[(i + 1) % 4], ordered[(i + 2) % 4]);
        if p1.sub(p0).cross(p2.sub(p1)) <= 0.0 {
            return None;
        }
    }
    if !crate::geom::inside_convex(&ordered, a.centre())
        || !crate::geom::inside_convex(&ordered, b.centre())
    {
        return None;
    }
    // Both predicted corners far from both finders: at least 12 modules along each side.
    for p in [x, y] {
        if p.dist(pa) < 12.0 * a.pitch || p.dist(pb) < 12.0 * b.pitch {
            return None;
        }
    }
    let (qx, qy) = if area > 0.0 { (x, y) } else { (y, x) };
    Some(Shape { q: [(Some((ia, ja)), pa), (None, qx), (Some((ib, jb)), pb), (None, qy)] })
}

/// Every symbol candidate of three or four finders among `finders`, best first: four-finder
/// groups first, then by score.
pub(crate) fn candidates(finders: &[Finder]) -> Vec<Candidate> {
    let links = all_links(finders);
    let mut out: Vec<Candidate> = Vec::new();
    for (ia, per_finder) in links.iter().enumerate() {
        for corner in 0..4 {
            // B beyond corner + 1 along side `corner`; C beyond corner − 1 along side corner − 1.
            let side_b = corner;
            let side_c = (corner + 3) % 4;
            let Some(bs) = per_finder.get(side_b).and_then(|l| l.get(1)) else { continue };
            let Some(cs) = per_finder.get(side_c).and_then(|l| l.first()) else { continue };
            let Some(pa) = finders.get(ia).map(|f| f.corner(corner)) else { continue };
            for lb in bs.iter().filter(|l| l.corner == corner) {
                for lc in cs.iter().filter(|l| l.corner == corner && l.other != lb.other) {
                    let centre = finders.get(ia).map(Finder::centre);
                    let shared_b = shared_side(finders, lb.other, lb.other_corner, centre);
                    let shared_c = shared_side(finders, lc.other, lc.other_corner, centre);
                    let (Some(sb), Some(sc)) = (shared_b, shared_c) else { continue };
                    let Some(pd) = predict_d(
                        finders,
                        (lb.other, lb.other_corner, sb),
                        (lc.other, lc.other_corner, sc),
                    ) else {
                        continue;
                    };
                    let d = find_d(finders, pd, [ia, lb.other, lc.other]);
                    let (Some(pb), Some(pc)) = (
                        finders.get(lb.other).map(|f| f.corner(lb.other_corner)),
                        finders.get(lc.other).map(|f| f.corner(lc.other_corner)),
                    ) else {
                        continue;
                    };
                    let pd = d.and_then(|(i, j)| finders.get(i).map(|f| f.corner(j))).unwrap_or(pd);
                    let shape = Shape {
                        q: [
                            (Some((ia, corner)), pa),
                            (Some((lb.other, lb.other_corner)), pb),
                            (d, pd),
                            (Some((lc.other, lc.other_corner)), pc),
                        ],
                    };
                    // A fourth finder found near the predicted corner may be another square:
                    // without it the three still form a candidate.
                    let three = Shape { q: [shape.q[0], shape.q[1], (None, pd), shape.q[3]] };
                    if let Some(c) =
                        orient(finders, &shape).or_else(|| d.and_then(|_| orient(finders, &three)))
                    {
                        out.push(c);
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| y.count().cmp(&x.count()).then(x.score.total_cmp(&y.score)));
    out
}

/// Candidates of two finders at opposite corners, among the finders not marked in `taken`,
/// best first. Such a candidate becomes a symbol only when the finders it predicts are found
/// (see `symbol::read`).
pub(crate) fn pairs(finders: &[Finder], taken: &[bool]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let free: Vec<usize> =
        (0..finders.len()).filter(|&i| !taken.get(i).copied().unwrap_or(true)).collect();
    let mut pairs = 0usize;
    'pairs: for (n, &ia) in free.iter().enumerate() {
        for &ib in free.iter().skip(n + 1) {
            let (Some(a), Some(b)) = (finders.get(ia), finders.get(ib)) else { continue };
            let ratio = a.pitch / b.pitch.max(1e-9);
            if !(0.5..=2.0).contains(&ratio) {
                continue;
            }
            for ja in (0..4).filter(|&j| a.quiet_corner(j)) {
                for jb in (0..4).filter(|&j| b.quiet_corner(j)) {
                    let Some(shape) = diagonal(finders, (ia, ja), (ib, jb)) else { continue };
                    if let Some(c) = orient(finders, &shape) {
                        out.push(c);
                        pairs += 1;
                        if pairs >= MAX_PAIRS {
                            break 'pairs;
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| y.count().cmp(&x.count()).then(x.score.total_cmp(&y.score)));
    out
}

/// Candidates of two finders on one edge, among the finders not marked in `taken`: the
/// fallback of 5.11 step 2. The symbol's two edges that leave the shared edge are traced along
/// the quiet zone from each finder to their far ends, which give the two missing corners.
pub(crate) fn edge_pairs(
    image: &crate::LumaImage,
    finders: &[Finder],
    taken: &[bool],
) -> Vec<Candidate> {
    let links = all_links(finders);
    let mut out = Vec::new();
    let free = |i: usize| !taken.get(i).copied().unwrap_or(true);
    for (ia, per_finder) in links.iter().enumerate() {
        if !free(ia) {
            continue;
        }
        let Some(a) = finders.get(ia) else { continue };
        for (side, per_side) in per_finder.iter().enumerate() {
            let Some(forward) = per_side.get(1) else { continue };
            for l in forward.iter().filter(|l| free(l.other)).take(2) {
                let Some(b) = finders.get(l.other) else { continue };
                let (ca, cb) = (l.corner, l.other_corner);
                // A's other outer side runs from corner ca toward ca − 1 (side is ca).
                let _ = side;
                let Some((_, end_a)) =
                    crate::symbol::trace(image, a, a.corner(ca), a.corner(ca + 3))
                else {
                    continue;
                };
                let Some(sb) = shared_side(finders, l.other, cb, Some(a.centre())) else {
                    continue;
                };
                let toward_b = if sb == cb { b.corner(cb + 3) } else { b.corner(cb + 1) };
                let Some((_, end_b)) = crate::symbol::trace(image, b, b.corner(cb), toward_b)
                else {
                    continue;
                };
                let shape = Shape {
                    q: [
                        (Some((ia, ca)), a.corner(ca)),
                        (Some((l.other, cb)), b.corner(cb)),
                        (None, end_b),
                        (None, end_a),
                    ],
                };
                if let Some(c) = orient(finders, &shape) {
                    out.push(c);
                }
            }
        }
    }
    out.sort_by(|x, y| x.score.total_cmp(&y.score));
    out.truncate(MAX_PAIRS);
    out
}

/// Most candidates from a single finder.
const MAX_SINGLES: usize = 8;

/// Candidates from one finder among those not marked in `taken`: the two edges that leave its
/// quiet corner are traced to their ends, which give two more corners; the fourth completes a
/// parallelogram. Which of the two edges is the symbol's x direction is tried both ways; the
/// finder's classification must fit the way tried (the solid BL, the most robust finder under
/// blur (5.3.2 property 3), fits both), and the finders that `symbol::read` then finds at the
/// other corners must fit it too.
pub(crate) fn singles(
    image: &crate::LumaImage,
    finders: &[Finder],
    taken: &[bool],
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (i, f) in finders.iter().enumerate() {
        if out.len() >= 2 * MAX_SINGLES {
            break;
        }
        if taken.get(i).copied().unwrap_or(true) {
            continue;
        }
        let kind = f.best_kind().0;
        for j in (0..4).filter(|&j| f.quiet_corner(j)) {
            let c = f.corner(j);
            let Some((_, e1)) = crate::symbol::trace(image, f, c, f.corner(j + 1)) else {
                continue;
            };
            let Some((_, e3)) = crate::symbol::trace(image, f, c, f.corner(j + 3)) else {
                continue;
            };
            // Both edges at least 15 modules long: a side of 20 or more.
            if e1.dist(c) < 15.0 * f.pitch || e3.dist(c) < 15.0 * f.pitch {
                continue;
            }
            let e2 = e1.add(e3).sub(c);
            for x_is_e1 in [true, false] {
                let (ex, ey) = if x_is_e1 { (e1, e3) } else { (e3, e1) };
                let sigma = toward(f, j, ex) * step_x(kind);
                let jm = i32::try_from(j).unwrap_or(0)
                    - sigma * i32::try_from(outer_corner(kind)).unwrap_or(0);
                let r = usize::try_from(jm.rem_euclid(4)).unwrap_or(0);
                let t = transform(r, sigma);
                let score =
                    f.ssd.get(kind).and_then(|row| row.get(t)).copied().unwrap_or(f64::INFINITY);
                if score > SSD_ASSIGN {
                    continue;
                }
                let mut members: [Option<Member>; 4] = [None; 4];
                let mut outer = [Point::default(); 4];
                if let Some(slot) = members.get_mut(kind) {
                    *slot = Some(Member { finder: i, transform: t });
                }
                for (k, p) in
                    [(kind, c), (flip_x(kind), ex), (flip_y(kind), ey), (flip_x(flip_y(kind)), e2)]
                {
                    if let Some(slot) = outer.get_mut(k) {
                        *slot = p;
                    }
                }
                out.push(Candidate { members, outer, score, sigma });
            }
        }
    }
    out
}

/// The side of finder `fi` at corner `j` that points toward `toward_point` (the shared side).
fn shared_side(
    finders: &[Finder],
    fi: usize,
    j: usize,
    toward_point: Option<Point>,
) -> Option<usize> {
    let f = finders.get(fi)?;
    let dir = toward(f, j, toward_point?);
    Some(if dir > 0 { j } else { (j + 3) % 4 })
}

/// The finder with a corner nearest to `p`, within 2.5 pitches, not among `taken`; with that
/// corner.
fn find_d(finders: &[Finder], p: Point, taken: [usize; 3]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, f64)> = None;
    for (i, f) in finders.iter().enumerate() {
        if taken.contains(&i) {
            continue;
        }
        for j in 0..4 {
            let d = f.corner(j).dist(p);
            if d <= 2.5 * f.pitch && f.quiet_corner(j) && best.is_none_or(|(_, _, bd)| d < bd) {
                best = Some((i, j, d));
            }
        }
    }
    best.map(|(i, j, _)| (i, j))
}
