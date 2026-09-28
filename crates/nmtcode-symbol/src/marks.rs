//! Reference-mark lines (5.6.1).

use alloc::vec::Vec;

use nmtcode_core::is_valid_side;

/// Sides shorter than this have no lines between the two finder centres (5.6.1).
const MARKS_FROM_SIDE: u32 = 48;
/// The line spacing that n is chosen to stay within (5.6.1).
const LINE_SPACING: u32 = 24;

/// The number of intervals n between the finder centre lines on a side of `side` modules
/// (5.6.1). `side` must be at least 5 and at most 4108.
pub(crate) fn interval_count(side: u32) -> u32 {
    if side < MARKS_FROM_SIDE { 1 } else { (side - 5).div_ceil(LINE_SPACING) }
}

/// The line coordinates line\[0\] … line\[n\] on a side of `side` modules (5.6.1).
/// `side` must be at least 5 and at most 4108.
pub(crate) fn lines(side: u32) -> Vec<u32> {
    let s = side - 5;
    let n = interval_count(side);
    (0..=n).map(|i| 2 + (2 * i * s + n) / (2 * n)).collect()
}

/// The reference-mark line coordinates line\[0\] … line\[n\] of 5.6.1 for a side of `side`
/// modules (W for the x lines, H for the y lines). line\[0\] = 2 and line\[n\] = `side` − 3 are
/// the finder centres.
///
/// Returns `None` when `side` is not a valid symbol side (a multiple of 4 from 20 to 4108).
pub fn reference_mark_lines(side: u32) -> Option<Vec<u32>> {
    is_valid_side(side).then(|| lines(side))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The consequences listed in 5.6.1, for every valid side from 48 to 4108.
    ///
    /// Only sides that are multiples of 4 are symbol sides. Read over every integer, the claims
    /// fail at 54 (line\[1\] = 18, a spacing of 16) and 55, which `sides_54_and_55` pins.
    #[test]
    fn spacing_and_edge_distance_for_every_side() {
        for z in (MARKS_FROM_SIDE..=4108).step_by(4) {
            let l = lines(z);
            let n = l.len() - 1;
            assert_eq!(l[0], 2, "side {z}");
            assert_eq!(l[n], z - 3, "side {z}");
            assert!(l[1] >= 19, "side {z}: line[1] = {}", l[1]);
            assert!(l[n - 1] <= z - 20, "side {z}: line[n-1] = {}", l[n - 1]);
            for pair in l.windows(2) {
                let gap = pair[1] - pair[0];
                assert!((17..=24).contains(&gap), "side {z}: gap {gap}");
            }
        }
    }

    #[test]
    fn short_sides_have_only_the_finder_lines() {
        for z in 20..MARKS_FROM_SIDE {
            assert_eq!(lines(z), [2, z - 3]);
        }
    }

    /// Two sides that are not multiples of 4 break the spacing claim of 5.6.1.
    #[test]
    fn sides_54_and_55() {
        assert_eq!(lines(54), [2, 18, 35, 51]);
        assert_eq!(lines(55), [2, 19, 35, 52]);
        let invalid: alloc::vec::Vec<u32> = (MARKS_FROM_SIDE..=4108)
            .filter(|&z| lines(z).windows(2).any(|p| !(17..=24).contains(&(p[1] - p[0]))))
            .collect();
        assert_eq!(invalid, [54, 55]);
        assert_eq!(reference_mark_lines(54), None);
    }
}
