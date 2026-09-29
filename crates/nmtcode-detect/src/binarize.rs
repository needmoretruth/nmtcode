//! Local adaptive threshold: the midpoint, in linear light, of the darkest and lightest values
//! around each block of 8 × 8 pixels, where that range stands out from the noise.
//!
//! A pixel is dark when it is below the threshold of its block. Blocks whose neighbourhood of
//! 5 × 5 blocks (40 × 40 pixels) has too little range to hold an edge are flat: none of their
//! pixels is dark. The dark regions of a flat area therefore shrink to bands along their edges,
//! which keeps the outline of every dark square (a finder, 5.3) where it is.

use crate::LumaImage;

/// Side of a threshold block in pixels.
pub(crate) const BLOCK: usize = 8;
/// Neighbourhood radius in blocks: the threshold of a block comes from (2R + 1)² blocks.
const RADIUS: usize = 2;

/// The threshold of every block and the noise estimate of the image.
pub(crate) struct Thresholds {
    /// Blocks per row.
    pub bw: usize,
    /// Threshold per block, row-major; a pixel is dark when its value is below it. 0 in a flat
    /// block, where no pixel is dark.
    pub t: Vec<u8>,
    /// Estimated standard deviation of the pixel noise.
    pub noise: f64,
}

impl Thresholds {
    /// The threshold of the block holding pixel (`x`, `y`).
    pub(crate) fn at(&self, x: usize, y: usize) -> u8 {
        self.t.get((y / BLOCK) * self.bw + x / BLOCK).copied().unwrap_or(0)
    }
}

/// Noise estimate: the 25th percentile of the absolute difference of horizontal neighbours, on
/// every fourth row, is 0.45 σ for Gaussian noise on a flat area; edges only raise the upper
/// percentiles.
fn noise_sigma(image: &LumaImage) -> f64 {
    let w = image.w_usize();
    let mut histogram = [0u64; 256];
    for row in image.pixels.chunks(w.max(1)).step_by(8) {
        for pair in row.windows(2) {
            if let [a, b] = *pair
                && let Some(bin) = histogram.get_mut(usize::from(a.abs_diff(b)))
            {
                *bin += 1;
            }
        }
    }
    let total: u64 = histogram.iter().sum();
    if total == 0 {
        return 1.0;
    }
    let target = total / 4;
    let mut seen = 0u64;
    let mut p25 = 0.0;
    for (value, &n) in (0u32..).zip(histogram.iter()) {
        if seen + n > target {
            // Interpolate inside the bin: values are integers, so a bin of width 1 around each.
            let inside = crate::num::u64_f64(target - seen) / crate::num::u64_f64(n.max(1));
            p25 = f64::from(value) - 0.5 + inside;
            break;
        }
        seen += n;
    }
    (p25.max(0.0) / 0.45).max(0.5)
}

/// Computes the thresholds of `image` (a usable image) at `fraction` of the way from the dark
/// to the light level in linear light: 0.5 splits an edge at its midpoint; a smaller value keeps
/// thin light gaps (a finder's separator under blur) light at the cost of thin dark lines.
pub(crate) fn thresholds(image: &LumaImage, fraction: f64) -> Thresholds {
    let w = image.w_usize();
    let h = image.h_usize();
    let bw = w.div_ceil(BLOCK).max(1);
    let bh = h.div_ceil(BLOCK).max(1);
    // Per block: the least and the greatest sum of two horizontal neighbours. The pair sum
    // halves the noise variance and ignores single-pixel outliers.
    let mut lo = vec![u16::MAX; bw * bh];
    let mut hi = vec![0u16; bw * bh];
    for (y, row) in image.pixels.chunks(w.max(1)).enumerate() {
        let brow = (y / BLOCK) * bw;
        let (Some(lo_row), Some(hi_row)) =
            (lo.get_mut(brow..brow + bw), hi.get_mut(brow..brow + bw))
        else {
            continue;
        };
        if row.len() < 2 {
            let s = 2 * u16::from(row.first().copied().unwrap_or(255));
            if let (Some(a), Some(b)) = (lo_row.first_mut(), hi_row.first_mut()) {
                *a = (*a).min(s);
                *b = (*b).max(s);
            }
            continue;
        }
        // Pair x covers pixels x and x + 1; block bx holds pairs 8·bx to 8·bx + 7.
        for (bx, (a, b)) in lo_row.iter_mut().zip(hi_row.iter_mut()).enumerate() {
            let x0 = bx * BLOCK;
            let Some(seg) = row.get(x0..(x0 + BLOCK + 1).min(row.len())) else { continue };
            let (mut l, mut g) = (*a, *b);
            if seg.len() == 1 {
                let s = 2 * u16::from(seg.first().copied().unwrap_or(255));
                l = l.min(s);
                g = g.max(s);
            }
            for pair in seg.windows(2) {
                if let [p, q] = *pair {
                    let s = u16::from(p) + u16::from(q);
                    l = l.min(s);
                    g = g.max(s);
                }
            }
            *a = l;
            *b = g;
        }
    }
    // Separable minimum and maximum over (2R + 1)² blocks.
    let filter = |src: &[u16], take_max: bool| -> Vec<u16> {
        let pick = |a: u16, b: u16| if take_max { a.max(b) } else { a.min(b) };
        let init = if take_max { 0 } else { u16::MAX };
        let mut rows = vec![init; bw * bh];
        for (out_row, in_row) in rows.chunks_mut(bw).zip(src.chunks(bw)) {
            for (bx, slot) in out_row.iter_mut().enumerate() {
                let from = bx.saturating_sub(RADIUS);
                let to = (bx + RADIUS + 1).min(in_row.len());
                *slot = in_row.get(from..to).unwrap_or(&[]).iter().fold(init, |m, &v| pick(m, v));
            }
        }
        let mut out = vec![init; bw * bh];
        for by in 0..bh {
            let from = by.saturating_sub(RADIUS);
            let to = (by + RADIUS + 1).min(bh);
            let Some(slot_row) = out.get_mut(by * bw..(by + 1) * bw) else { continue };
            for y in from..to {
                let Some(in_row) = rows.get(y * bw..(y + 1) * bw) else { continue };
                for (slot, &v) in slot_row.iter_mut().zip(in_row.iter()) {
                    *slot = pick(*slot, v);
                }
            }
        }
        out
    };
    let lo = filter(&lo, false);
    let hi = filter(&hi, true);
    let noise = noise_sigma(image);
    // Flat when the range of the neighbourhood is within what noise alone gives over 1600
    // pixels (about 7 σ), with a margin.
    let min_range = 8.0 * noise + 6.0;
    let t = lo
        .iter()
        .zip(hi.iter())
        .map(|(&l, &g)| {
            let range = f64::from(g.saturating_sub(l)) / 2.0;
            if range < min_range {
                0
            } else {
                // The split point in linear light (see `light`).
                let (dl, ll) = (
                    crate::light::linear(f64::from(l) / 2.0),
                    crate::light::linear(f64::from(g) / 2.0),
                );
                crate::light::stored_u8(dl + fraction * (ll - dl))
            }
        })
        .collect();
    Thresholds { bw, t, noise }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_images_have_no_dark_pixels() {
        let image = LumaImage::new(64, 64, vec![40; 64 * 64]).unwrap();
        let t = thresholds(&image, 0.5);
        assert!(t.t.iter().all(|&v| v == 0));
    }

    #[test]
    fn an_edge_is_split_at_its_midpoint() {
        let pixels: Vec<u8> = (0..64 * 64).map(|i| if i % 64 < 32 { 20 } else { 220 }).collect();
        let image = LumaImage::new(64, 64, pixels).unwrap();
        let t = thresholds(&image, 0.5);
        // Stored 20 and 220 meet at stored 162 in linear light.
        assert_eq!(t.at(30, 30), 162);
        assert!(t.noise <= 1.0);
    }
}
