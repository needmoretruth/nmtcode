//! Symbol size selection: the RECOMMENDED default of chapter 1 (1.5).
//!
//! Size selection is a generator choice, not part of the format (1.5): a reader accepts every
//! valid size. The rules here are the ones 1.5 recommends.

use core::cmp::Ordering;

use nmtcode_core::{MAX_SIDE, MIN_SIDE, is_valid_side};
use nmtcode_symbol::SymbolCounts;

/// Step between valid sides in modules (chapter 2, 2.2).
const SIDE_STEP: u32 = 4;

/// Every valid side, 20 to 4108 in steps of [`SIDE_STEP`], in increasing order.
fn sides() -> impl Iterator<Item = u32> {
    (MIN_SIDE..=MAX_SIDE).filter(|side| side.is_multiple_of(SIDE_STEP))
}

/// The message capacity K in bytes (chapter 4, 4.6) of a `width` × `height` symbol at
/// error-correction level `level`: the largest container, CRC-32C included, that the symbol
/// holds. `None` when the size is not valid (each side a multiple of 4 from 20 to 4108) or the
/// level is not 0 to 3.
pub fn capacity(width: u32, height: u32, level: u8) -> Option<usize> {
    let counts = SymbolCounts::new(width, height).ok()?;
    nmtcode_ecc::split(counts.codewords, level).ok().map(|split| split.capacity())
}

/// A requested width-to-height ratio, for example 16:9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AspectRatio {
    width: u32,
    height: u32,
}

impl AspectRatio {
    /// The ratio `width` : `height`. `None` when either part is 0.
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 { None } else { Some(Self { width, height }) }
    }

    /// The width part.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// The height part.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// 1:1.
    const SQUARE: Self = Self { width: 1, height: 1 };
}

/// Limits on the symbol size (chapter 1, 1.5). Every field is optional; the default has none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SizeConstraints {
    /// The requested ratio of width to height.
    ///
    /// A size meets the ratio when its longer side is the multiple of 4 nearest to its shorter
    /// side times the ratio (for a:b with a ≥ b, W is nearest to H · a / b; otherwise H is
    /// nearest to W · b / a; an exact half rounds up). Sides are multiples of 4, so most ratios
    /// are met to within half a step of the longer side. Among sizes of equal area the one whose
    /// ratio is closest wins. A ratio that no valid size meets (beyond about 205:1 or 1:205)
    /// gives no size. A ratio replaces the 1:2 to 2:1 window of [`SizeRule::Recommended`].
    pub aspect_ratio: Option<AspectRatio>,
    /// The largest width in modules.
    pub max_width: Option<u32>,
    /// The largest height in modules.
    pub max_height: Option<u32>,
}

/// How the encoder chooses the symbol size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SizeRule {
    /// The RECOMMENDED default of 1.5: among the sizes whose message capacity K holds the
    /// container and whose sides are within a factor of 2 of each other (W ≤ 2H and H ≤ 2W),
    /// the one with the smallest area W × H; among equal areas the one closest to square, then
    /// the taller one. The same as [`SizeRule::Constrained`] with no constraint.
    #[default]
    Recommended,
    /// The smallest square whose message capacity K holds the container.
    SmallestSquare,
    /// Exactly this width and height in modules.
    Exact {
        /// Width W in modules.
        width: u32,
        /// Height H in modules.
        height: u32,
    },
    /// The size with the smallest area W × H that holds the container and meets every given
    /// constraint; among equal areas the one closest to the requested aspect ratio (1:1 when
    /// none is given), then the taller one (1.5). Without an aspect ratio, only sizes with
    /// W ≤ 2H and H ≤ 2W are considered while one of them meets the maxima; otherwise every size
    /// within the maxima is.
    Constrained(SizeConstraints),
}

/// Why no size was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SizeFailure {
    /// The exact size is not a valid size.
    InvalidSize { width: u32, height: u32 },
    /// The exact size is valid but its capacity is below the container length.
    DoesNotFit { width: u32, height: u32, capacity: usize },
    /// No size that meets the rule holds the container.
    NoSize,
}

/// The multiple of 4 nearest to `numerator / denominator` modules, when it is a valid side
/// (20 to 4108); `None` otherwise, since then no valid side meets the ratio from this side.
/// `denominator` is not 0. Exact halves between two multiples round up.
fn nearest_side(numerator: u64, denominator: u64) -> Option<u32> {
    let step = u64::from(SIDE_STEP);
    // round(numerator / (step · denominator)) with integers: floor((2n + s·d) / (2·s·d)).
    let steps = (2 * numerator + step * denominator) / (2 * step * denominator);
    u32::try_from(steps.saturating_mul(step)).ok().filter(|&side| is_valid_side(side))
}

/// How far `width` × `height` is from `ratio`, as the fraction `(high, low)` ≥ 1: the larger of
/// W·b and H·a over the smaller, for the ratio a:b.
fn ratio_distance(width: u32, height: u32, ratio: AspectRatio) -> (u64, u64) {
    let across = u64::from(width) * u64::from(ratio.height);
    let down = u64::from(height) * u64::from(ratio.width);
    (across.max(down), across.min(down))
}

/// The order of 1.5 between two sizes that both qualify: smaller area first, then the ratio
/// closest to `ratio`, then the taller.
fn compare(a: (u32, u32), b: (u32, u32), ratio: AspectRatio) -> Ordering {
    let area = |(w, h): (u32, u32)| u64::from(w) * u64::from(h);
    let (a_high, a_low) = ratio_distance(a.0, a.1, ratio);
    let (b_high, b_low) = ratio_distance(b.0, b.1, ratio);
    // a_high / a_low against b_high / b_low, cross-multiplied; every factor is below 2^45.
    let a_distance = u128::from(a_high) * u128::from(b_low);
    let b_distance = u128::from(b_high) * u128::from(a_low);
    area(a).cmp(&area(b)).then(a_distance.cmp(&b_distance)).then(b.1.cmp(&a.1))
}

/// Whether the sides of a size are within a factor of 2 of each other: the window of the
/// RECOMMENDED rule of 1.5.
const fn within_window(width: u32, height: u32) -> bool {
    width <= 2 * height && height <= 2 * width
}

/// Chooses the size for a container of `len` bytes at level `level` by `rule`.
pub(crate) fn choose(rule: &SizeRule, len: usize, level: u8) -> Result<(u32, u32), SizeFailure> {
    let holds = |w: u32, h: u32| capacity(w, h, level).is_some_and(|k| k >= len);
    match *rule {
        SizeRule::Recommended => constrained(&SizeConstraints::default(), len, level),
        SizeRule::SmallestSquare => {
            sides().find(|&s| holds(s, s)).map(|s| (s, s)).ok_or(SizeFailure::NoSize)
        }
        SizeRule::Exact { width, height } => {
            if !is_valid_side(width) || !is_valid_side(height) {
                return Err(SizeFailure::InvalidSize { width, height });
            }
            let capacity = capacity(width, height, level).unwrap_or(0);
            if capacity >= len {
                Ok((width, height))
            } else {
                Err(SizeFailure::DoesNotFit { width, height, capacity })
            }
        }
        SizeRule::Constrained(constraints) => constrained(&constraints, len, level),
    }
}

/// [`SizeRule::Constrained`].
fn constrained(
    constraints: &SizeConstraints,
    len: usize,
    level: u8,
) -> Result<(u32, u32), SizeFailure> {
    let max_width = constraints.max_width.unwrap_or(MAX_SIDE);
    let max_height = constraints.max_height.unwrap_or(MAX_SIDE);
    let allowed = |w: u32, h: u32| {
        w <= max_width && h <= max_height && capacity(w, h, level).is_some_and(|k| k >= len)
    };
    let ratio = constraints.aspect_ratio.unwrap_or(AspectRatio::SQUARE);
    let mut best: Option<(u32, u32)> = None;
    let mut offer = |size: (u32, u32)| {
        if best.is_none_or(|current| compare(size, current, ratio) == Ordering::Less) {
            best = Some(size);
        }
    };
    if let Some(ratio) = constraints.aspect_ratio {
        // The longer side follows from the shorter one, so the ratio is met to within half a
        // step of the longer side.
        let (a, b) = (u64::from(ratio.width), u64::from(ratio.height));
        for short in sides() {
            let size = if a >= b {
                nearest_side(a * u64::from(short), b).map(|w| (w, short))
            } else {
                nearest_side(b * u64::from(short), a).map(|h| (short, h))
            };
            if let Some((w, h)) = size
                && allowed(w, h)
            {
                offer((w, h));
            }
        }
    } else {
        // For one width a taller symbol only has a larger area, so the first height that holds
        // the container is the only candidate of that width, and inside the window the first
        // such height that is at least half the width.
        let mut outside: Option<(u32, u32)> = None;
        for w in sides().take_while(|&w| w <= max_width) {
            let Some(first) = sides().take_while(|&h| h <= max_height).find(|&h| allowed(w, h))
            else {
                continue;
            };
            if outside.is_none_or(|current| compare((w, first), current, ratio) == Ordering::Less) {
                outside = Some((w, first));
            }
            let inside = sides()
                .skip_while(|&h| h < first)
                .take_while(|&h| h <= max_height && h <= 2 * w)
                .find(|&h| within_window(w, h) && allowed(w, h));
            if let Some(h) = inside {
                offer((w, h));
            }
        }
        return best.or(outside).ok_or(SizeFailure::NoSize);
    }
    best.ok_or(SizeFailure::NoSize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_side_rounds_to_multiples_of_four_within_range() {
        // 16:9: the width for H = 36 is 16 · 36 / 9 = 64, the height for W = 64 is 36.
        assert_eq!(nearest_side(16 * 36, 9), Some(64));
        assert_eq!(nearest_side(9 * 64, 16), Some(36));
        assert_eq!(nearest_side(100, 1), Some(100));
        assert_eq!(nearest_side(101, 1), Some(100));
        assert_eq!(nearest_side(102, 1), Some(104));
        assert_eq!(nearest_side(20, 1), Some(20));
        assert_eq!(nearest_side(17, 1), None);
        assert_eq!(nearest_side(4108, 1), Some(4108));
        assert_eq!(nearest_side(4111, 1), None);
        assert_eq!(nearest_side(u64::from(u32::MAX) * 4108, 1), None);
    }

    #[test]
    fn compare_prefers_area_then_ratio_then_height() {
        let square = AspectRatio::SQUARE;
        assert_eq!(compare((20, 20), (20, 24), square), Ordering::Less);
        assert_eq!(compare((20, 28), (28, 20), square), Ordering::Less);
        assert_eq!(compare((24, 24), (20, 28), square), Ordering::Greater);
        let wide = AspectRatio { width: 4, height: 1 };
        assert_eq!(compare((28, 20), (20, 28), wide), Ordering::Less);
        let tall = AspectRatio { width: 1, height: 4 };
        assert_eq!(compare((20, 28), (28, 20), tall), Ordering::Less);
    }

    #[test]
    fn recommended_rule_of_1_5() {
        // 20 × 20 gives K = 16, 20 × 24 gives 24, 20 × 28 and 24 × 24 give 34 at level 0.
        assert_eq!(capacity(20, 20, 0), Some(16));
        assert_eq!(capacity(20, 24, 0), Some(24));
        assert_eq!(capacity(20, 28, 0), Some(34));
        assert_eq!(capacity(24, 24, 0), Some(34));
        // The 29-byte container of 3.10 f: 20 × 28 and 28 × 20 hold it with a smaller area than
        // 24 × 24; they are equally close to square, and the taller one wins.
        assert_eq!(choose(&SizeRule::Recommended, 29, 0), Ok((20, 28)));
        assert_eq!(choose(&SizeRule::SmallestSquare, 29, 0), Ok((24, 24)));
        let any = SizeRule::Constrained(SizeConstraints::default());
        assert_eq!(choose(&any, 29, 0), Ok((20, 28)));
        assert_eq!(choose(&any, 28, 0), Ok((20, 28)));
        let tall = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(5, 7),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&tall, 28, 0), Ok((20, 28)));
        // 1:2 is met by 20 × 40, not by 20 × 28.
        let half = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(1, 2),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&half, 28, 0), Ok((20, 40)));
    }

    #[test]
    fn limits_and_failures() {
        let narrow = SizeRule::Constrained(SizeConstraints {
            max_width: Some(20),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&narrow, 28, 0), Ok((20, 28)));
        let wide = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(4, 1),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&wide, 28, 0), Ok((80, 20)));
        let screen = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(16, 9),
            ..SizeConstraints::default()
        });
        // 20 · 16 / 9 = 35.6 rounds to 36.
        assert_eq!(choose(&screen, 28, 0), Ok((36, 20)));
        let strip = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(200, 1),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&strip, 28, 0), Ok((4000, 20)));
        let impossible = SizeRule::Constrained(SizeConstraints {
            aspect_ratio: AspectRatio::new(1000, 1),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&impossible, 28, 0), Err(SizeFailure::NoSize));
        let tiny = SizeRule::Constrained(SizeConstraints {
            max_width: Some(16),
            ..SizeConstraints::default()
        });
        assert_eq!(choose(&tiny, 1, 0), Err(SizeFailure::NoSize));
        assert_eq!(choose(&SizeRule::SmallestSquare, usize::MAX, 0), Err(SizeFailure::NoSize));
        assert_eq!(
            choose(&SizeRule::Exact { width: 22, height: 20 }, 1, 0),
            Err(SizeFailure::InvalidSize { width: 22, height: 20 })
        );
        assert_eq!(
            choose(&SizeRule::Exact { width: 20, height: 20 }, 17, 0),
            Err(SizeFailure::DoesNotFit { width: 20, height: 20, capacity: 16 })
        );
    }

    #[test]
    fn exhaustive_constrained_search_agrees_with_brute_force() {
        // The pruned search without an aspect ratio against every pair, for a few lengths:
        // the best size inside the 1:2 to 2:1 window, else the best size within the maxima.
        for (len, level, max_w, max_h) in [
            (28, 0, 4108, 4108),
            (500, 1, 60, 4108),
            (3000, 3, 4108, 100),
            (90, 2, 40, 40),
            (2000, 0, 4108, 20),
            (700, 1, 24, 4108),
        ] {
            let rule = SizeRule::Constrained(SizeConstraints {
                aspect_ratio: None,
                max_width: Some(max_w),
                max_height: Some(max_h),
            });
            let best_of = |window: bool| {
                let mut best: Option<(u32, u32)> = None;
                for w in sides().filter(|&w| w <= max_w) {
                    for h in sides().filter(|&h| h <= max_h) {
                        if (!window || within_window(w, h))
                            && capacity(w, h, level).is_some_and(|k| k >= len)
                            && best.is_none_or(|b| {
                                compare((w, h), b, AspectRatio::SQUARE) == Ordering::Less
                            })
                        {
                            best = Some((w, h));
                        }
                    }
                }
                best
            };
            let expected = best_of(true).or_else(|| best_of(false));
            assert_eq!(choose(&rule, len, level).ok(), expected, "len {len} level {level}");
            if max_w == 4108 && max_h == 4108 {
                assert_eq!(choose(&SizeRule::Recommended, len, level).ok(), expected);
            }
        }
    }
}
