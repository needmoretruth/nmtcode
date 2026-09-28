//! Print growth compensation of specification 1.5 (`print_growth_dots`): the union of the dark
//! areas of the canvas is shrunk by k pixels on every edge that faces a light area.
//!
//! The result is the erosion of the dark pixels by a square of (2k + 1) × (2k + 1) pixels: a
//! pixel stays dark when every pixel within k of it, in x and in y, is dark. Outside the canvas
//! is light. With 2k below the module size s, the window around a pixel covers at most two
//! modules in each direction, so each module splits into three bands per axis: the first k
//! pixels (which also see the module before), the middle s − 2k (this module only) and the last
//! k (which also see the module after).

use nmtcode_core::ModuleGrid;

/// The three bands of a module along one axis: (first module offset, last module offset) of
/// the window, and the band's width in pixels for module size `s` and growth `k`.
pub(crate) fn bands(s: u32, k: u32) -> [(i64, i64, u32); 3] {
    [(-1, 0, k), (0, 0, s - 2 * k), (0, 1, k)]
}

/// Whether module (`x`, `y`) is dark; `false` outside the grid.
fn dark(modules: &ModuleGrid, x: i64, y: i64) -> bool {
    match (u32::try_from(x), u32::try_from(y)) {
        (Ok(x), Ok(y)) => modules.get(x, y) == Some(true),
        _ => false,
    }
}

/// Whether every module with x in `mx + xs.0 ..= mx + xs.1` and y in `my + ys.0 ..= my + ys.1`
/// is dark: the value of the pixels of that band pair after erosion.
pub(crate) fn eroded(
    modules: &ModuleGrid,
    mx: u32,
    my: u32,
    xs: (i64, i64),
    ys: (i64, i64),
) -> bool {
    let (mx, my) = (i64::from(mx), i64::from(my));
    (ys.0..=ys.1).all(|dy| (xs.0..=xs.1).all(|dx| dark(modules, mx + dx, my + dy)))
}

/// The eroded canvas on the grid of bands: cell (3i + a, 3j + b) is band a of module column i
/// and band b of module row j. `None` when the band grid would not fit in memory addressing.
pub(crate) fn band_grid(modules: &ModuleGrid, s: u32, k: u32) -> Option<ModuleGrid> {
    let bands = bands(s, k);
    let mut out =
        ModuleGrid::new(modules.width().checked_mul(3)?, modules.height().checked_mul(3)?)?;
    for my in 0..modules.height() {
        for (b, &(y0, y1, _)) in (0u32..).zip(&bands) {
            for mx in 0..modules.width() {
                for (a, &(x0, x1, _)) in (0u32..).zip(&bands) {
                    if eroded(modules, mx, my, (x0, x1), (y0, y1)) {
                        out.set(3 * mx + a, 3 * my + b, true);
                    }
                }
            }
        }
    }
    Some(out)
}

/// The pixel coordinate of the start of band cell `cell` (3i + a) for module size `s` and
/// growth `k`: i · s plus 0, k or s − k.
pub(crate) fn band_start(cell: u32, s: u32, k: u32) -> u64 {
    let offset = match cell % 3 {
        0 => 0,
        1 => k,
        _ => s - k,
    };
    u64::from(cell / 3) * u64::from(s) + u64::from(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The erosion of the pixel image by a (2k + 1)-square, computed pixel by pixel.
    fn brute_force(modules: &ModuleGrid, s: u32, k: u32) -> Vec<Vec<bool>> {
        let (w, h) = (modules.width() * s, modules.height() * s);
        let pixel = |x: i64, y: i64| {
            x >= 0
                && y >= 0
                && x < i64::from(w)
                && y < i64::from(h)
                && modules.get(u32::try_from(x).unwrap() / s, u32::try_from(y).unwrap() / s)
                    == Some(true)
        };
        let k = i64::from(k);
        (0..i64::from(h))
            .map(|y| {
                (0..i64::from(w))
                    .map(|x| (-k..=k).all(|dy| (-k..=k).all(|dx| pixel(x + dx, y + dy))))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn band_grid_equals_the_pixel_erosion() {
        let rows = ["##..#", "##.##", "#####", "..#..", "#.#.#"];
        let modules = ModuleGrid::from_rows(&rows).unwrap();
        for (s, k) in [(3, 1), (5, 1), (5, 2), (10, 1), (10, 4)] {
            let grid = band_grid(&modules, s, k).unwrap();
            let expected = brute_force(&modules, s, k);
            for (y, row) in expected.iter().enumerate() {
                for (x, &dark) in row.iter().enumerate() {
                    let (x, y) = (u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
                    let cell = |p: u32| {
                        let (m, o) = (p / s, p % s);
                        3 * m
                            + if o < k {
                                0
                            } else if o >= s - k {
                                2
                            } else {
                                1
                            }
                    };
                    assert_eq!(grid.get(cell(x), cell(y)), Some(dark), "s {s} k {k} ({x}, {y})");
                }
            }
            assert_eq!(band_start(3 * 5, s, k), u64::from(5 * s));
        }
    }
}
