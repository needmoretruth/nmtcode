//! A small deterministic random number generator (`SplitMix64`), so that every frame is fixed
//! by its seed on every platform.

/// The `SplitMix64` generator.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator from `seed`.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        // 53 random bits over 2^53.
        let bits = self.next_u64() >> 11;
        f64::from(u32::try_from(bits >> 21).unwrap_or(0)) * 2f64.powi(-32)
            + f64::from(u32::try_from(bits & 0x1F_FFFF).unwrap_or(0)) * 2f64.powi(-53)
    }

    /// Uniform in [`lo`, `hi`).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Uniform integer in [0, `n`); 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// A standard normal value (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-300);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (core::f64::consts::TAU * u2).cos()
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
}

/// A seed from a condition name and a trial number, stable across runs.
pub fn seed(name: &str, trial: u64) -> u64 {
    // FNV-1a over the name, then mixed with the trial.
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    let mut r = Rng::new(h ^ trial.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    r.next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(1);
        for _ in 0..1000 {
            let x = a.unit();
            assert_eq!(x.to_bits(), b.unit().to_bits());
            assert!((0.0..1.0).contains(&x));
        }
        assert_ne!(seed("A", 0), seed("A", 1));
        assert_eq!(seed("A", 3), seed("A", 3));
    }
}
