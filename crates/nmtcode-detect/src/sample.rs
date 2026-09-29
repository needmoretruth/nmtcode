//! Sampling every module and deciding dark or light. Samples are taken in linear light, where
//! blur is a linear sum of the modules around (see `light`).
//!
//! Blur spreads each module into its neighbours: under a Gaussian blur of half a module a
//! light module surrounded by dark ones reads darker than the midpoint. The reader therefore
//! decides in two steps:
//!
//! 1. A first decision against a local threshold: two-means per block of 8 × 8 modules, averaged
//!    over the 3 × 3 blocks around.
//! 2. Three rounds of decision-directed equalisation: in each region of 16 × 16 modules, the
//!    sample of a module is fitted by least squares as a constant plus a weight for its own
//!    value and one weight for each of its eight neighbours' current decisions; each module is
//!    then decided on its own term, with the neighbours' terms taken away. Function modules and
//!    the first quiet-zone ring enter with their known values.
//!
//! The distance of the equalised value from the decision point, in units of the module's own
//! contrast, is the module's confidence.

use nmtcode_core::ModuleGrid;
use nmtcode_symbol::{Layout, ModuleClass};

use crate::LumaImage;
use crate::marks::Mapping;
use crate::num::count_f64;

/// Modules whose equalised value lies within this distance of the decision point (in units of
/// the module's contrast; 0.5 is fully certain) are reported as uncertain.
pub(crate) const UNCERTAIN_MARGIN: f64 = 0.12;

/// Side of a threshold block, in modules.
const BLOCK: usize = 8;
/// Rounds of equalisation.
const ROUNDS: usize = 5;
/// Side of an equalisation region, in modules.
const REGION: usize = 16;
/// Features of the equaliser: constant, own value, 8 neighbours.
const FEATURES: usize = 10;
/// Neighbour offsets in the order of the features.
const NEIGHBOURS: [(i64, i64); 8] =
    [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// The sampled symbol.
pub(crate) struct Sampled {
    pub grid: ModuleGrid,
    /// Format and data modules with a confidence below [`UNCERTAIN_MARGIN`], least confident
    /// first.
    pub uncertain: Vec<(u32, u32)>,
}

/// A `w` × `h` array of modules from (`o`, `o`): the symbol and its first quiet-zone ring
/// (`o` = −1), or a patch around a finder.
struct Plane<T> {
    o: i64,
    w: usize,
    h: usize,
    v: Vec<T>,
}

impl<T: Copy> Plane<T> {
    fn new(o: i64, w: usize, h: usize, value: T) -> Self {
        Self { o, w, h, v: vec![value; w * h] }
    }

    fn like<U: Copy>(&self, value: U) -> Plane<U> {
        Plane::new(self.o, self.w, self.h, value)
    }

    fn get(&self, x: i64, y: i64) -> Option<T> {
        let (Ok(xi), Ok(yi)) = (usize::try_from(x - self.o), usize::try_from(y - self.o)) else {
            return None;
        };
        if xi >= self.w || yi >= self.h {
            return None;
        }
        self.v.get(yi * self.w + xi).copied()
    }

    fn set(&mut self, x: i64, y: i64, value: T) {
        let (Ok(xi), Ok(yi)) = (usize::try_from(x - self.o), usize::try_from(y - self.o)) else {
            return;
        };
        if xi < self.w
            && yi < self.h
            && let Some(slot) = self.v.get_mut(yi * self.w + xi)
        {
            *slot = value;
        }
    }
}

/// First decisions and three rounds of equalisation over the modules [0, `tw`) × [0, `th`) of
/// `samples`; function modules enter with their `known` value. Returns what was read (light =
/// `true`) and the margins.
fn decide(
    samples: &Plane<f64>,
    known: &Plane<Option<bool>>,
    tw: usize,
    th: usize,
) -> (Plane<bool>, Plane<f64>) {
    // `features` holds the known value of every function module and the current decision of
    // every other module; `light` is what was read, function modules included, so that a
    // covered finder reads as covered.
    let mut features = first_decision(samples, known);
    let mut light = samples.like(true);
    light.v.clone_from(&features.v);
    let mut margin = samples.like(0.5f64);
    for _ in 0..ROUNDS {
        let (next, next_margin, _) = equalise(samples, &features, tw, th);
        for ((f, &read), k) in features.v.iter_mut().zip(next.v.iter()).zip(known.v.iter()) {
            *f = k.unwrap_or(read);
        }
        light = next;
        margin = next_margin;
    }
    (light, margin)
}

/// Reads the `size` × `size` modules at a symbol corner, [0, `size`)² in the corner's own
/// coordinates, with the quiet zone at negative coordinates and the corner's finder and
/// separator given by `finder(x, y)` (dark = `true`) at [0, 5)². `at(u, v)` maps continuous
/// corner coordinates to the image. Returns (dark, margin) per module, row-major.
pub(crate) fn corner_patch(
    image: &LumaImage,
    at: &dyn Fn(f64, f64) -> crate::geom::Point,
    finder: &dyn Fn(u32, u32) -> bool,
    size: usize,
) -> Vec<(bool, f64)> {
    let span = size + 2;
    let mut samples = Plane::new(-2, span, span, 0.0f64);
    let mut known = Plane::new(-2, span, span, None::<bool>);
    let end = i64::try_from(size).unwrap_or(0);
    for y in -2..end {
        for x in -2..end {
            let p = at(
                count_f64(usize::try_from(x + 2).unwrap_or(0)) - 1.5,
                count_f64(usize::try_from(y + 2).unwrap_or(0)) - 1.5,
            );
            samples.set(x, y, crate::light::linear(image.sample(p)));
            let k = if x < 0 || y < 0 {
                Some(true)
            } else if x < 5 && y < 5 {
                Some(!finder(u32::try_from(x).unwrap_or(0), u32::try_from(y).unwrap_or(0)))
            } else if (x == 5 && y <= 5) || (y == 5 && x <= 5) {
                Some(true)
            } else {
                None
            };
            known.set(x, y, k);
        }
    }
    let (light, margin) = decide(&samples, &known, size, size);
    let mut out = Vec::with_capacity(size * size);
    for y in 0..end {
        for x in 0..end {
            out.push((!light.get(x, y).unwrap_or(true), margin.get(x, y).unwrap_or(0.5)));
        }
    }
    out
}

/// Samples the modules of a `layout`-sized symbol through `mapping` and decides each.
pub(crate) fn sample(image: &LumaImage, mapping: &Mapping, layout: &Layout) -> Option<Sampled> {
    let (w, h) = (layout.width(), layout.height());
    let (wu, hu) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    let (pw, ph) = (wu + 2, hu + 2);
    let centre = mapping.jacobian(f64::from(w) / 2.0, f64::from(h) / 2.0);
    let pitch = f64::midpoint(centre.0.norm(), centre.1.norm());
    let offsets: &[(f64, f64)] = if pitch >= 3.5 {
        &[(-0.18, -0.18), (0.18, -0.18), (-0.18, 0.18), (0.18, 0.18)]
    } else {
        &[(0.0, 0.0)]
    };
    let mut samples = Plane::new(-1, pw, ph, 0.0f64);
    // Known values: Some(light).
    let mut known = Plane::new(-1, pw, ph, None::<bool>);
    for y in -1..=i64::from(h) {
        for x in -1..=i64::from(w) {
            let (u, v) =
                (f64::from(i32::try_from(x).ok()?) + 0.5, f64::from(i32::try_from(y).ok()?) + 0.5);
            let s = offsets
                .iter()
                .map(|&(du, dv)| crate::light::linear(image.sample(mapping.map(u + du, v + dv))))
                .sum::<f64>()
                / count_f64(offsets.len());
            samples.set(x, y, s);
            let inside = x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h);
            let value = if inside {
                let (xu, yu) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
                layout.function_value(xu, yu).map(|dark| !dark)
            } else {
                Some(true)
            };
            known.set(x, y, value);
        }
    }
    let (light, margin) = decide(&samples, &known, wu, hu);
    let mut grid = ModuleGrid::new(w, h)?;
    let mut uncertain: Vec<(f64, u32, u32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let (xi, yi) = (i64::from(x), i64::from(y));
            let is_light = light.get(xi, yi).unwrap_or(true);
            grid.set(x, y, !is_light);
            let class = layout.module_class(x, y);
            if matches!(class, Some(ModuleClass::Data | ModuleClass::Format)) {
                let m = margin.get(xi, yi).unwrap_or(0.5);
                if m < UNCERTAIN_MARGIN {
                    uncertain.push((m, x, y));
                }
            }
        }
    }
    uncertain.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(Sampled { grid, uncertain: uncertain.into_iter().map(|(_, x, y)| (x, y)).collect() })
}

/// Two-means of `values`: (dark mean, light mean), `None` when one class is empty.
fn two_means(values: &[f64]) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let mut t = values.iter().sum::<f64>() / count_f64(values.len());
    let (mut d, mut l) = (t, t);
    for _ in 0..4 {
        let (mut sd, mut nd, mut sl, mut nl) = (0.0, 0usize, 0.0, 0usize);
        for &v in values {
            if v < t {
                sd += v;
                nd += 1;
            } else {
                sl += v;
                nl += 1;
            }
        }
        if nd == 0 || nl == 0 {
            return None;
        }
        d = sd / count_f64(nd);
        l = sl / count_f64(nl);
        t = f64::midpoint(d, l);
    }
    Some((d, l))
}

/// Decisions against block thresholds; known modules keep their values.
fn first_decision(samples: &Plane<f64>, known: &Plane<Option<bool>>) -> Plane<bool> {
    let (pw, ph) = (samples.w, samples.h);
    let (bw, bh) = (pw.div_ceil(BLOCK), ph.div_ceil(BLOCK));
    let mut levels: Vec<Option<(f64, f64)>> = Vec::with_capacity(bw * bh);
    let mut buffer = Vec::with_capacity(BLOCK * BLOCK);
    for by in 0..bh {
        for bx in 0..bw {
            buffer.clear();
            for y in by * BLOCK..((by + 1) * BLOCK).min(ph) {
                for x in bx * BLOCK..((bx + 1) * BLOCK).min(pw) {
                    if let Some(&v) = samples.v.get(y * pw + x) {
                        buffer.push(v);
                    }
                }
            }
            levels.push(two_means(&buffer));
        }
    }
    let all: Vec<f64> = samples.v.clone();
    let global = two_means(&all).unwrap_or((0.0, 255.0));
    let mut out = samples.like(true);
    for by in 0..bh {
        for bx in 0..bw {
            let (mut sd, mut sl, mut n) = (0.0, 0.0, 0usize);
            for ny in by.saturating_sub(1)..=(by + 1).min(bh - 1) {
                for nx in bx.saturating_sub(1)..=(bx + 1).min(bw - 1) {
                    if let Some(Some((d, l))) = levels.get(ny * bw + nx) {
                        // Only blocks with a real split: both classes clearly apart.
                        if l - d > 0.25 * (global.1 - global.0) {
                            sd += d;
                            sl += l;
                            n += 1;
                        }
                    }
                }
            }
            let t = if n > 0 {
                f64::midpoint(sd, sl) / count_f64(n)
            } else {
                f64::midpoint(global.0, global.1)
            };
            for y in by * BLOCK..((by + 1) * BLOCK).min(ph) {
                for x in bx * BLOCK..((bx + 1) * BLOCK).min(pw) {
                    let i = y * pw + x;
                    let decided = match known.v.get(i).copied().flatten() {
                        Some(k) => k,
                        None => samples.v.get(i).copied().unwrap_or(255.0) >= t,
                    };
                    if let Some(slot) = out.v.get_mut(i) {
                        *slot = decided;
                    }
                }
            }
        }
    }
    out
}

/// The feature vector of module (`x`, `y`) under decisions `light`.
fn features(light: &Plane<bool>, own: bool, x: i64, y: i64) -> [f64; FEATURES] {
    let mut f = [0.0f64; FEATURES];
    f[0] = 1.0;
    f[1] = f64::from(u8::from(own));
    for (k, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
        if let Some(slot) = f.get_mut(k + 2) {
            *slot = f64::from(u8::from(light.get(x + dx, y + dy).unwrap_or(true)));
        }
    }
    f
}

/// One round of equalisation over the W × H modules; returns the new decisions and margins.
fn equalise(
    samples: &Plane<f64>,
    light: &Plane<bool>,
    w: usize,
    h: usize,
) -> (Plane<bool>, Plane<f64>, Vec<Option<[f64; FEATURES]>>) {
    let (rw, rh) = (w.div_ceil(REGION), h.div_ceil(REGION));
    // Normal equations per region.
    let mut ata = vec![[0.0f64; FEATURES * FEATURES]; rw * rh];
    let mut atb = vec![[0.0f64; FEATURES]; rw * rh];
    for y in 0..h {
        for x in 0..w {
            let (xi, yi) = (i64::try_from(x).unwrap_or(0), i64::try_from(y).unwrap_or(0));
            let own = light.get(xi, yi).unwrap_or(true);
            let f = features(light, own, xi, yi);
            let s = samples.get(xi, yi).unwrap_or(0.0);
            let r = (y / REGION) * rw + x / REGION;
            if let (Some(a), Some(b)) = (ata.get_mut(r), atb.get_mut(r)) {
                for i in 0..FEATURES {
                    let fi = f.get(i).copied().unwrap_or(0.0);
                    if fi == 0.0 {
                        continue;
                    }
                    if let Some(slot) = b.get_mut(i) {
                        *slot += fi * s;
                    }
                    for j in 0..FEATURES {
                        if let Some(slot) = a.get_mut(i * FEATURES + j) {
                            *slot += fi * f.get(j).copied().unwrap_or(0.0);
                        }
                    }
                }
            }
        }
    }
    // Coefficients per region from its 3 × 3 neighbourhood.
    let mut coefficients: Vec<Option<[f64; FEATURES]>> = Vec::with_capacity(rw * rh);
    for ry in 0..rh {
        for rx in 0..rw {
            let mut a = [0.0f64; FEATURES * FEATURES];
            let mut b = [0.0f64; FEATURES];
            for ny in ry.saturating_sub(1)..=(ry + 1).min(rh - 1) {
                for nx in rx.saturating_sub(1)..=(rx + 1).min(rw - 1) {
                    if let (Some(sa), Some(sb)) = (ata.get(ny * rw + nx), atb.get(ny * rw + nx)) {
                        for (d, s) in a.iter_mut().zip(sa.iter()) {
                            *d += s;
                        }
                        for (d, s) in b.iter_mut().zip(sb.iter()) {
                            *d += s;
                        }
                    }
                }
            }
            let trace: f64 =
                (0..FEATURES).map(|i| a.get(i * FEATURES + i).copied().unwrap_or(0.0)).sum();
            let ridge = 1e-6 * trace / count_f64(FEATURES) + 1e-9;
            for i in 0..FEATURES {
                if let Some(v) = a.get_mut(i * FEATURES + i) {
                    *v += ridge;
                }
            }
            let mut av = a.to_vec();
            let mut bv = b.to_vec();
            let solved = crate::num::solve(&mut av, &mut bv, FEATURES).map(|()| {
                let mut c = [0.0f64; FEATURES];
                for (d, s) in c.iter_mut().zip(bv.iter()) {
                    *d = *s;
                }
                c
            });
            coefficients.push(solved);
        }
    }
    let mut out = samples.like(true);
    out.v.clone_from(&light.v);
    let mut margins = samples.like(0.5f64);
    for step in 0..w * h {
        let (x, y) = (step % w, step / w);
        {
            let (xi, yi) = (i64::try_from(x).unwrap_or(0), i64::try_from(y).unwrap_or(0));
            let r = (y / REGION) * rw + x / REGION;
            let Some(Some(c)) = coefficients.get(r) else { continue };
            let beta = c.get(1).copied().unwrap_or(0.0);
            let s = samples.get(xi, yi).unwrap_or(0.0);
            // Gauss–Seidel: neighbours decided in this round already count with their new value;
            // updating all at once from the old values oscillates on patterns such as dark,
            // light, dark.
            let f = features(&out, false, xi, yi);
            // The sample less the constant and the neighbours' terms, over the own weight.
            let rest: f64 = f
                .iter()
                .zip(c.iter())
                .enumerate()
                .filter(|&(k, _)| k != 1)
                .map(|(_, (a, b))| a * b)
                .sum();
            if beta <= 1e-6 {
                continue;
            }
            let z = (s - rest) / beta;
            margins.set(xi, yi, (z - 0.5).abs());
            out.set(xi, yi, z >= 0.5);
        }
    }
    (out, margins, coefficients)
}
