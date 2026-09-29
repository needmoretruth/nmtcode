//! Numeric conversions and small numeric helpers used by the geometry. Float-to-integer `as`
//! casts in Rust saturate and map NaN to 0, so these helpers cannot panic; they exist to keep the
//! casts in one audited place.

/// `floor(v)` as an `i32`, saturating at the `i32` range; NaN gives 0.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn floor_i32(v: f64) -> i32 {
    v.floor() as i32
}

/// `v` rounded down to a `u32`, saturating; negative and NaN give 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn floor_u32(v: f64) -> u32 {
    v.floor() as u32
}

/// A count as `f64`. Counts in this crate stay far below 2^53, where the conversion is exact.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn count_f64(v: usize) -> f64 {
    v as f64
}

/// A 64-bit count as `f64` (exact below 2^53).
#[allow(clippy::cast_precision_loss)]
pub(crate) fn u64_f64(v: u64) -> f64 {
    v as f64
}

/// The error function, by Abramowitz and Stegun 7.1.26 (absolute error below 1.5 · 10^−7).
pub(crate) fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    sign * (1.0 - poly * (-x * x).exp())
}

/// The standard normal distribution function Φ.
pub(crate) fn phi(x: f64) -> f64 {
    f64::midpoint(1.0, erf(x / core::f64::consts::SQRT_2))
}

/// The median of `values` (sorted in place); `None` when empty or when a value is NaN.
pub(crate) fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() || values.iter().any(|v| v.is_nan()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let mid = n / 2;
    if n % 2 == 1 {
        values.get(mid).copied()
    } else {
        Some(f64::midpoint(*values.get(mid - 1)?, *values.get(mid)?))
    }
}

/// Solves the `n × n` system `a · x = b` in place by Gaussian elimination with partial
/// pivoting (`a` row-major). `None` when the matrix is singular to working precision.
pub(crate) fn solve(a: &mut [f64], b: &mut [f64], n: usize) -> Option<()> {
    if a.len() != n * n || b.len() != n {
        return None;
    }
    let scale = a.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(f64::MIN_POSITIVE);
    for col in 0..n {
        let mut pivot = col;
        let mut best = 0.0;
        for row in col..n {
            let v = a.get(row * n + col).copied().unwrap_or(0.0).abs();
            if v > best {
                best = v;
                pivot = row;
            }
        }
        if best <= 1e-13 * scale || !best.is_finite() {
            return None;
        }
        if pivot != col {
            for k in 0..n {
                a.swap(pivot * n + k, col * n + k);
            }
            b.swap(pivot, col);
        }
        let diag = a.get(col * n + col).copied()?;
        for row in col + 1..n {
            let factor = a.get(row * n + col).copied()? / diag;
            if factor == 0.0 {
                continue;
            }
            for k in col..n {
                let upper = a.get(col * n + k).copied()?;
                if let Some(v) = a.get_mut(row * n + k) {
                    *v -= factor * upper;
                }
            }
            let upper_b = b.get(col).copied()?;
            if let Some(v) = b.get_mut(row) {
                *v -= factor * upper_b;
            }
        }
    }
    for col in (0..n).rev() {
        let mut sum = b.get(col).copied()?;
        for k in col + 1..n {
            sum -= a.get(col * n + k).copied()? * b.get(k).copied()?;
        }
        let diag = a.get(col * n + col).copied()?;
        *b.get_mut(col)? = sum / diag;
    }
    b.iter().all(|v| v.is_finite()).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_and_phi() {
        assert!(erf(0.0).abs() < 1e-7);
        assert!((erf(1.0) - 0.842_700_79).abs() < 1e-6);
        assert!((erf(-2.0) + 0.995_322_27).abs() < 1e-6);
        assert!((phi(1.0) - 0.841_344_75).abs() < 1e-6);
    }

    #[test]
    fn solves_a_small_system() {
        let mut a = vec![2.0, 1.0, 1.0, 3.0];
        let mut b = vec![3.0, 5.0];
        solve(&mut a, &mut b, 2).unwrap();
        assert!((b[0] - 0.8).abs() < 1e-12 && (b[1] - 1.4).abs() < 1e-12);
        let mut singular = vec![1.0, 2.0, 2.0, 4.0];
        let mut rhs = vec![1.0, 2.0];
        assert!(solve(&mut singular, &mut rhs, 2).is_none());
    }

    #[test]
    fn medians() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), Some(2.5));
    }
}
