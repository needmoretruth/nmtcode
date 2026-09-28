//! Arithmetic in GF(2^8) with the field of specification 4.2.
//!
//! An element is one byte; bit 7 is the coefficient of z^7. Addition is XOR.
//! Multiplication is polynomial multiplication modulo the field polynomial
//! z^8 + z^4 + z^3 + z^2 + 1 (`0x11D`). The primitive element is α = `0x02`,
//! whose order is 255.

/// The field polynomial z^8 + z^4 + z^3 + z^2 + 1 (4.2), with the z^8 term as bit 8.
pub const FIELD_POLY: u16 = 0x11D;

/// The primitive element α (4.2): the class of z.
pub const ALPHA: u8 = 0x02;

/// The multiplicative order of α. Every non-zero element is α^i for exactly one i in `0..ORDER`.
pub const ORDER: usize = 255;

/// Exponent and logarithm tables. `exp` repeats after 255 entries so that the sum of two
/// logarithms (at most 508) indexes it directly.
struct Tables {
    exp: [u8; 512],
    log: [u8; 256],
}

static TABLES: Tables = build_tables();

const fn build_tables() -> Tables {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    // The field polynomial without its z^8 term: what a carry out of bit 7 folds back in.
    let reduce = FIELD_POLY.to_le_bytes()[0];
    let mut x: u8 = 1;
    let mut power: u8 = 0;
    let mut i = 0;
    while i < ORDER {
        exp[i] = x;
        // `From` is not usable in a `const fn`; a u8 always fits in usize.
        let slot = x as usize;
        log[slot] = power;
        let carry = x & 0x80 != 0;
        x <<= 1;
        if carry {
            x ^= reduce;
        }
        power = power.wrapping_add(1);
        i += 1;
    }
    while i < exp.len() {
        exp[i] = exp[i - ORDER];
        i += 1;
    }
    Tables { exp, log }
}

/// Field addition, which is also subtraction: bitwise XOR.
#[inline]
pub const fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// α^i, for any `i` (the exponent is taken modulo 255).
#[inline]
pub fn exp(i: usize) -> u8 {
    TABLES.exp[i % ORDER]
}

/// The discrete logarithm of `a` to the base α, in `0..255`, or `None` for `a = 0`.
#[inline]
pub fn log(a: u8) -> Option<u8> {
    if a == 0 { None } else { Some(TABLES.log[usize::from(a)]) }
}

/// The product `a · b`.
#[inline]
pub fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        let la = usize::from(TABLES.log[usize::from(a)]);
        let lb = usize::from(TABLES.log[usize::from(b)]);
        // la + lb <= 508 < 512.
        TABLES.exp[la + lb]
    }
}

/// The quotient `a / b`, or `None` for `b = 0`.
#[inline]
pub fn div(a: u8, b: u8) -> Option<u8> {
    if b == 0 {
        None
    } else if a == 0 {
        Some(0)
    } else {
        let la = usize::from(TABLES.log[usize::from(a)]);
        let lb = usize::from(TABLES.log[usize::from(b)]);
        // la + 255 - lb <= 509 < 512, and lb <= 254 so the subtraction does not underflow.
        Some(TABLES.exp[la + ORDER - lb])
    }
}

/// The multiplicative inverse of `a`, or `None` for `a = 0`.
#[inline]
pub fn inv(a: u8) -> Option<u8> {
    div(1, a)
}

/// `a` raised to the power `e`, with 0^0 = 1.
#[inline]
pub fn pow(a: u8, e: usize) -> u8 {
    if e == 0 {
        1
    } else if a == 0 {
        0
    } else {
        let la = usize::from(TABLES.log[usize::from(a)]);
        // la <= 254 and e % 255 <= 254, so the product fits easily.
        exp(la * (e % ORDER))
    }
}
