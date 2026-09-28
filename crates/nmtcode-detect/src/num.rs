//! Numeric conversions used by the geometry. Float-to-integer `as` casts in Rust saturate and map
//! NaN to 0, so these helpers cannot panic; they exist to keep the casts in one audited place.

/// `floor(v)` as an `i32`, saturating at the `i32` range; NaN gives 0.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn floor_i32(v: f64) -> i32 {
    v.floor() as i32
}

/// `v` rounded to the nearest `u8`, saturating at 0 and 255; NaN gives 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn round_u8(v: f64) -> u8 {
    v.round() as u8
}

/// `v` rounded down to a `u32`, saturating; negative and NaN give 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn floor_u32(v: f64) -> u32 {
    v.floor() as u32
}

/// `v` rounded up to a `u32`, saturating; negative and NaN give 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn ceil_u32(v: f64) -> u32 {
    v.ceil() as u32
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
