//! Tests of GF(2^8) with the field polynomial `0x11D` and α = `0x02` (4.2).

use nmtcode_ecc::gf;

/// Carry-less multiplication reduced modulo 0x11D, one bit at a time: an implementation that
/// shares nothing with the table-driven one under test.
fn slow_mul(a: u8, b: u8) -> u8 {
    let mut product: u16 = 0;
    for bit in 0..8 {
        if b & (1 << bit) != 0 {
            product ^= u16::from(a) << bit;
        }
    }
    for bit in (8..16).rev() {
        if product & (1 << bit) != 0 {
            product ^= 0x11D << (bit - 8);
        }
    }
    u8::try_from(product).unwrap()
}

#[test]
fn multiplication_matches_the_field_polynomial() {
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            assert_eq!(gf::mul(a, b), slow_mul(a, b), "{a:#04x} · {b:#04x}");
        }
    }
}

#[test]
fn alpha_has_order_255() {
    assert_eq!(gf::FIELD_POLY, 0x11D);
    assert_eq!(gf::ALPHA, 2);
    let mut x = 1u8;
    let mut seen = [false; 256];
    for i in 0..255usize {
        assert_eq!(gf::exp(i), x, "α^{i}");
        assert!(!seen[usize::from(x)], "α^{i} repeats an earlier power");
        seen[usize::from(x)] = true;
        assert_eq!(gf::log(x), Some(u8::try_from(i).unwrap()));
        x = slow_mul(x, gf::ALPHA);
    }
    assert_eq!(x, 1, "α^255 = 1");
    assert!(!seen[0]);
    assert_eq!(gf::exp(8), 0x1D, "α^8 = z^4 + z^3 + z^2 + 1");
    assert_eq!(gf::exp(255), 1);
    assert_eq!(gf::exp(256 + 7), gf::exp(8));
    assert_eq!(gf::log(0), None);
}

#[test]
fn division_inverse_and_power() {
    for a in 0..=255u8 {
        assert_eq!(gf::div(a, 0), None);
        assert_eq!(gf::add(a, a), 0);
        for b in 1..=255u8 {
            let q = gf::div(a, b).unwrap();
            assert_eq!(gf::mul(q, b), a, "{a:#04x} / {b:#04x}");
        }
        if a != 0 {
            assert_eq!(gf::mul(a, gf::inv(a).unwrap()), 1);
        }
    }
    assert_eq!(gf::inv(0), None);
    for a in 0..=255u8 {
        let mut power = 1u8;
        for e in 0..600usize {
            assert_eq!(gf::pow(a, e), power, "{a:#04x}^{e}");
            power = gf::mul(power, a);
        }
    }
}
