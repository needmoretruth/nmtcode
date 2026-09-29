//! Linear light. Optics blur an image in linear light, while a camera stores its values after a
//! transfer curve (sRGB, about a power of 1 / 2.4). The midpoint of the stored dark and light
//! levels therefore lies on the dark side of a blurred edge: under a blur of half a module, a
//! dark square measured at that midpoint comes out about a tenth smaller. Every threshold that
//! places an edge or splits dark from light is taken at the midpoint in linear light instead,
//! mapped back to a stored value. The mapping is monotonic, so each decision is still one of
//! luma above or below a threshold (specification 1.4).

use std::sync::OnceLock;

/// The linear value of each stored value 0 to 255, by the sRGB transfer function (IEC
/// 61966-2-1), scaled to 0 to 255.
fn table() -> &'static [f64; 256] {
    static TABLE: OnceLock<[f64; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0.0; 256];
        for (v, slot) in (0u32..).zip(t.iter_mut()) {
            *slot = 255.0 * decode(f64::from(v) / 255.0);
        }
        t
    })
}

fn decode(e: f64) -> f64 {
    if e <= 0.040_45 { e / 12.92 } else { ((e + 0.055) / 1.055).powf(2.4) }
}

fn encode(l: f64) -> f64 {
    if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

/// The linear value (0 to 255) of a stored value (0 to 255), interpolated between integers.
pub(crate) fn linear(v: f64) -> f64 {
    let v = v.clamp(0.0, 255.0);
    let i = crate::num::floor_i32(v).clamp(0, 254);
    let f = v - f64::from(i);
    let t = table();
    let (Some(&a), Some(&b)) = (
        usize::try_from(i).ok().and_then(|i| t.get(i)),
        usize::try_from(i + 1).ok().and_then(|i| t.get(i)),
    ) else {
        return v;
    };
    a + (b - a) * f
}

/// The stored value (0 to 255) of a linear value (0 to 255).
pub(crate) fn stored(l: f64) -> f64 {
    255.0 * encode((l / 255.0).clamp(0.0, 1.0))
}

/// [`stored`] rounded to a whole value, by table: for the thresholds of whole blocks.
pub(crate) fn stored_u8(l: f64) -> u8 {
    static TABLE: OnceLock<[u8; 4097]> = OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u8; 4097];
        for (i, slot) in (0u32..).zip(t.iter_mut()) {
            let v = stored(f64::from(i) * 255.0 / 4096.0).round();
            *slot = u8::try_from(crate::num::floor_u32(v).min(255)).unwrap_or(u8::MAX);
        }
        t
    });
    let i = crate::num::floor_u32((l.clamp(0.0, 255.0) * 4096.0 / 255.0).round());
    usize::try_from(i).ok().and_then(|i| t.get(i)).copied().unwrap_or(u8::MAX)
}

/// The stored value whose linear value lies midway between those of `dark` and `light`.
pub(crate) fn midpoint(dark: f64, light: f64) -> f64 {
    stored(f64::midpoint(linear(dark), linear(light)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_midpoint() {
        for v in [0.0, 10.0, 60.0, 128.0, 200.0, 255.0] {
            assert!((stored(linear(v)) - v).abs() < 0.2, "{v}");
        }
        // Stored 40 and 230: linear 5.4 and 201.8; the linear midpoint is stored 170.8.
        let m = midpoint(40.0, 230.0);
        assert!((m - 170.8).abs() < 0.5, "{m}");
        assert!(midpoint(100.0, 100.0) - 100.0 < 0.2);
    }
}
