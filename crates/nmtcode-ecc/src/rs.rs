//! The Reed-Solomon code of one block: generator (4.3), systematic encoding (4.4) and an
//! errors-and-erasures decoder that meets 4.9.
//!
//! Byte j of an n-byte block (0 ≤ j < n) is the coefficient of x^(n−1−j): message bytes
//! first, then parity bytes. The roots of the generator are α^0, …, α^(P−1).
//!
//! The decoder is bounded-distance. It computes the syndromes, builds the erasure locator,
//! runs Berlekamp–Massey on the Forney-modified syndromes to find the error locator, finds
//! its roots by a Chien search over the whole field, and computes the byte values with
//! Forney's formula. It corrects every block with 2e + s ≤ P (e errors, s erasures) and
//! rejects the block in each case listed in 4.9. When 2e + s > P it can still return a
//! valid but wrong codeword; 4.9 makes no promise against that and the container's
//! CRC-32C (chapter 3) is the check that catches it.

use alloc::vec;
use alloc::vec::Vec;

use crate::{EccError, gf};

/// The shortest block, n = 3 bytes (4.2).
pub const MIN_BLOCK_LEN: usize = 3;

/// The longest block, n = 255 bytes (4.2).
pub const MAX_BLOCK_LEN: usize = 255;

/// The smallest parity count, P = 2 (4.2).
pub const MIN_PARITY_LEN: usize = 2;

/// The largest parity count, P = 254: at most n − 1 with n ≤ 255 (4.2).
pub const MAX_PARITY_LEN: usize = MAX_BLOCK_LEN - 1;

/// Room for every polynomial the decoder builds: degree at most P ≤ 254.
const CAP: usize = 256;

fn check_parity(parity_len: usize) -> Result<(), EccError> {
    if (MIN_PARITY_LEN..=MAX_PARITY_LEN).contains(&parity_len) && parity_len.is_multiple_of(2) {
        Ok(())
    } else {
        Err(EccError::InvalidParity)
    }
}

/// Checks the block constraints of 4.2: P even in 2..=254, n in 3..=255 and n > P.
pub(crate) fn check_block(block_len: usize, parity_len: usize) -> Result<(), EccError> {
    check_parity(parity_len)?;
    if (MIN_BLOCK_LEN..=MAX_BLOCK_LEN).contains(&block_len) && block_len > parity_len {
        Ok(())
    } else {
        Err(EccError::InvalidBlockLength)
    }
}

/// The generator polynomial `g_P(x) = (x − α^0)(x − α^1)···(x − α^(P−1))` of 4.3.
///
/// Returns the P + 1 coefficients g\[0\], …, g\[P\], highest degree first; g\[0\] = 1.
///
/// # Errors
///
/// [`EccError::InvalidParity`] if `parity_len` is odd, below 2 or above 254.
pub fn generator(parity_len: usize) -> Result<Vec<u8>, EccError> {
    check_parity(parity_len)?;
    let mut g = Vec::with_capacity(parity_len + 1);
    g.push(1u8);
    for i in 0..parity_len {
        let root = gf::exp(i);
        // Multiply by x, then add root · g shifted by one place (4.3). Walking downwards
        // lets each step read the coefficient before it is overwritten.
        g.push(0);
        for j in (1..g.len()).rev() {
            let prev = g[j - 1];
            g[j] ^= gf::mul(prev, root);
        }
    }
    Ok(g)
}

/// A systematic encoder for one parity count P (4.4). It keeps `g_P` so that many blocks
/// with the same P reuse it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoder {
    generator: Vec<u8>,
}

impl Encoder {
    /// An encoder for `parity_len` parity bytes per block.
    ///
    /// # Errors
    ///
    /// [`EccError::InvalidParity`] if `parity_len` is odd, below 2 or above 254.
    pub fn new(parity_len: usize) -> Result<Self, EccError> {
        Ok(Self { generator: generator(parity_len)? })
    }

    /// The parity count P.
    pub fn parity_len(&self) -> usize {
        self.generator.len().saturating_sub(1)
    }

    /// The generator coefficients g\[0\], …, g\[P\], highest degree first (4.3).
    pub fn generator(&self) -> &[u8] {
        &self.generator
    }

    /// Writes the P parity bytes p\[0\], …, p\[P−1\] of `message` into `parity`, by the
    /// shift-register division of 4.4.
    ///
    /// # Errors
    ///
    /// - [`EccError::InvalidBlockLength`] if `message` is empty or `message.len() + P > 255`.
    /// - [`EccError::InvalidParity`] if `parity.len()` is not P.
    pub fn parity_into(&self, message: &[u8], parity: &mut [u8]) -> Result<(), EccError> {
        let p = self.parity_len();
        check_block(message.len().saturating_add(p), p)?;
        if parity.len() != p {
            return Err(EccError::InvalidParity);
        }
        let taps = self.generator.get(1..).unwrap_or(&[]);
        parity.fill(0);
        for &m in message {
            let f = m ^ parity.first().copied().unwrap_or(0);
            // Shift left by one byte; P ≥ 2 so the range `1..` is inside the slice.
            parity.copy_within(1.., 0);
            if let Some(last) = parity.last_mut() {
                *last = 0;
            }
            if f != 0 {
                for (r, &g) in parity.iter_mut().zip(taps) {
                    *r ^= gf::mul(f, g);
                }
            }
        }
        Ok(())
    }

    /// The P parity bytes of `message` (4.4).
    ///
    /// # Errors
    ///
    /// [`EccError::InvalidBlockLength`] if `message` is empty or `message.len() + P > 255`.
    pub fn parity(&self, message: &[u8]) -> Result<Vec<u8>, EccError> {
        let mut parity = vec![0u8; self.parity_len()];
        self.parity_into(message, &mut parity)?;
        Ok(parity)
    }

    /// The whole codeword of 4.4: the k message bytes followed by the P parity bytes.
    ///
    /// # Errors
    ///
    /// [`EccError::InvalidBlockLength`] if `message` is empty or `message.len() + P > 255`.
    pub fn encode(&self, message: &[u8]) -> Result<Vec<u8>, EccError> {
        let parity = self.parity(message)?;
        let mut codeword = Vec::with_capacity(message.len() + parity.len());
        codeword.extend_from_slice(message);
        codeword.extend_from_slice(&parity);
        Ok(codeword)
    }
}

/// Encodes one block: returns `message` followed by its `parity_len` parity bytes (4.4).
///
/// # Errors
///
/// - [`EccError::InvalidParity`] if `parity_len` is odd, below 2 or above 254.
/// - [`EccError::InvalidBlockLength`] if `message` is empty or `message.len() + parity_len > 255`.
pub fn encode(message: &[u8], parity_len: usize) -> Result<Vec<u8>, EccError> {
    Encoder::new(parity_len)?.encode(message)
}

/// c(α^i), with byte 0 of `block` as the highest-degree coefficient.
fn eval_at_power(block: &[u8], i: usize) -> u8 {
    let x = gf::exp(i);
    block.iter().fold(0u8, |acc, &byte| gf::mul(acc, x) ^ byte)
}

fn syndromes_into(block: &[u8], out: &mut [u8]) {
    for (i, s) in out.iter_mut().enumerate() {
        *s = eval_at_power(block, i);
    }
}

/// The syndromes c(α^0), …, c(α^(P−1)) of a block. All are zero exactly when the block is
/// a codeword.
///
/// # Errors
///
/// - [`EccError::InvalidParity`] if `parity_len` is odd, below 2 or above 254.
/// - [`EccError::InvalidBlockLength`] if `block.len()` is outside 3..=255 or not above `parity_len`.
pub fn syndromes(block: &[u8], parity_len: usize) -> Result<Vec<u8>, EccError> {
    check_block(block.len(), parity_len)?;
    let mut out = vec![0u8; parity_len];
    syndromes_into(block, &mut out);
    Ok(out)
}

/// A polynomial over GF(2^8), lowest degree first, of degree below [`CAP`].
#[derive(Clone, Copy)]
struct Poly {
    coef: [u8; CAP],
    /// Number of coefficients in use; every coefficient at or above it is zero.
    len: usize,
}

impl Poly {
    fn one() -> Self {
        let mut coef = [0u8; CAP];
        coef[0] = 1;
        Self { coef, len: 1 }
    }

    fn get(&self, i: usize) -> u8 {
        self.coef.get(i).copied().unwrap_or(0)
    }

    fn degree(&self) -> usize {
        self.coef.iter().take(self.len).rposition(|&c| c != 0).unwrap_or(0)
    }

    fn eval(&self, x: u8) -> u8 {
        self.coef.iter().take(self.len).rev().fold(0u8, |acc, &c| gf::mul(acc, x) ^ c)
    }

    /// The formal derivative at `x`: in characteristic 2 only odd-degree terms survive,
    /// and the term `c_i·x^i` becomes `c_i·x^(i−1)`.
    fn eval_derivative(&self, x: u8) -> u8 {
        let x2 = gf::mul(x, x);
        // Sum of c_(2m+1) · (x^2)^m, by Horner over the odd coefficients.
        self.coef
            .iter()
            .take(self.len)
            .skip(1)
            .step_by(2)
            .rev()
            .fold(0u8, |acc, &c| gf::mul(acc, x2) ^ c)
    }

    /// Multiplies by `(1 + x_k·x)`.
    fn mul_linear(&mut self, xk: u8) -> Result<(), EccError> {
        if self.len >= CAP {
            return Err(EccError::Uncorrectable);
        }
        self.len += 1;
        for i in (1..self.len).rev() {
            let prev = self.coef[i - 1];
            self.coef[i] ^= gf::mul(prev, xk);
        }
        Ok(())
    }

    /// Adds `scale · x^shift · other`.
    fn add_scaled_shift(&mut self, other: &Self, scale: u8, shift: usize) -> Result<(), EccError> {
        for (i, &c) in other.coef.iter().take(other.len).enumerate() {
            if c == 0 {
                continue;
            }
            let slot = self.coef.get_mut(i + shift).ok_or(EccError::Uncorrectable)?;
            *slot ^= gf::mul(c, scale);
        }
        self.len = self.len.max(other.len + shift).min(CAP);
        Ok(())
    }

    /// The product `self · other`, keeping only the terms of degree below `limit`.
    fn mul_trunc(&self, other: &Self, limit: usize) -> Self {
        let mut out = Self { coef: [0u8; CAP], len: 0 };
        let limit = limit.min(CAP);
        for (i, &a) in self.coef.iter().take(self.len).enumerate() {
            if a == 0 {
                continue;
            }
            for (j, &b) in other.coef.iter().take(other.len).enumerate() {
                if let Some(slot) = out.coef.get_mut(i + j).filter(|_| i + j < limit) {
                    *slot ^= gf::mul(a, b);
                }
            }
        }
        out.len = (self.len + other.len).saturating_sub(1).min(limit);
        out
    }
}

/// Berlekamp–Massey: the shortest linear-feedback shift register that generates `seq`.
/// Returns its connection polynomial C(x), with C(0) = 1, and its length L.
fn berlekamp_massey(seq: &[u8]) -> Result<(Poly, usize), EccError> {
    let mut c = Poly::one();
    let mut prev = Poly::one();
    let mut len = 0usize;
    let mut shift = 1usize;
    let mut prev_discrepancy = 1u8;
    for (r, &sr) in seq.iter().enumerate() {
        let mut d = sr;
        for i in 1..=len {
            let past = r.checked_sub(i).and_then(|k| seq.get(k)).copied().unwrap_or(0);
            d ^= gf::mul(c.get(i), past);
        }
        if d == 0 {
            shift += 1;
            continue;
        }
        let scale = gf::div(d, prev_discrepancy).ok_or(EccError::Uncorrectable)?;
        if 2 * len <= r {
            let before = c;
            c.add_scaled_shift(&prev, scale, shift)?;
            len = r + 1 - len;
            prev = before;
            prev_discrepancy = d;
            shift = 1;
        } else {
            c.add_scaled_shift(&prev, scale, shift)?;
            shift += 1;
        }
    }
    Ok((c, len))
}

/// Decodes one block in place and returns the number of bytes whose value it changed.
///
/// `erasures` lists byte positions (0 ≤ j < n) that the reader marked as unreliable (4.9);
/// their values are ignored. Duplicate positions count once.
///
/// The block is corrected whenever 2e + s ≤ P, where e is the number of wrong bytes outside
/// the erasures and s the number of distinct erasures. On any error the block is left
/// unchanged.
///
/// # Errors
///
/// - [`EccError::InvalidParity`] if `parity_len` is odd, below 2 or above 254.
/// - [`EccError::InvalidBlockLength`] if `block.len()` is outside 3..=255 or not above `parity_len`.
/// - [`EccError::ErasureOutOfRange`] if an erasure position is not below `block.len()`.
/// - [`EccError::Uncorrectable`] if the decoder rejects the block by any rule of 4.9: more
///   than P erasures, an error locator of degree above (P − s)/2, a locator whose number of
///   distinct roots in the field differs from its degree, a root outside the n bytes of the
///   shortened block, or a corrected block whose recomputed syndromes are not all zero.
pub fn decode(block: &mut [u8], parity_len: usize, erasures: &[usize]) -> Result<usize, EccError> {
    let n = block.len();
    check_block(n, parity_len)?;
    let p = parity_len;

    let mut erased = [false; CAP];
    let mut s = 0usize;
    for &j in erasures {
        if j >= n {
            return Err(EccError::ErasureOutOfRange);
        }
        let flag = erased.get_mut(j).ok_or(EccError::ErasureOutOfRange)?;
        if !*flag {
            *flag = true;
            s += 1;
        }
    }
    if s > p {
        return Err(EccError::Uncorrectable);
    }

    let mut synd = Poly { coef: [0u8; CAP], len: p };
    syndromes_into(block, synd.coef.get_mut(..p).unwrap_or(&mut []));
    if synd.coef.iter().all(|&v| v == 0) {
        return Ok(0);
    }

    // Byte j is the coefficient of x^(n−1−j), so its locator is α^(n−1−j).
    let exponent_of = |j: usize| n - 1 - j;

    // Erasure locator Γ(x) = Π (1 − X_k·x) over the erased positions.
    let mut gamma = Poly::one();
    for (j, _) in erased.iter().take(n).enumerate().filter(|(_, e)| **e) {
        gamma.mul_linear(gf::exp(exponent_of(j)))?;
    }

    // Forney-modified syndromes: T(x) = Γ(x)·S(x) mod x^P. Its coefficients T_s … T_(P−1)
    // obey the recurrence of the error locator σ(x), of degree e.
    let modified = gamma.mul_trunc(&synd, p);
    let tail = modified.coef.get(s..p).ok_or(EccError::Uncorrectable)?;
    let (sigma, errors) = berlekamp_massey(tail)?;

    // 4.9: reject a locator of degree above (P − s)/2. BM's length L bounds the degree
    // from above; a degree below L also means no consistent error pattern.
    if 2 * errors > p - s || sigma.degree() != errors {
        return Err(EccError::Uncorrectable);
    }

    // Chien search over every non-zero element: σ(α^(−q)) = 0 puts an error at exponent q.
    let mut error_exps = [0usize; CAP];
    let mut found = 0usize;
    for q in 0..gf::ORDER {
        if sigma.eval(gf::exp(gf::ORDER - q)) != 0 {
            continue;
        }
        // 4.9: a root outside the n bytes of the shortened block.
        if q >= n {
            return Err(EccError::Uncorrectable);
        }
        // An error on an erased byte cannot occur within 2e + s ≤ P.
        if erased.get(n - 1 - q).copied().unwrap_or(true) {
            return Err(EccError::Uncorrectable);
        }
        let slot = error_exps.get_mut(found).ok_or(EccError::Uncorrectable)?;
        *slot = q;
        found += 1;
    }
    // 4.9: the number of distinct roots differs from the degree.
    if found != errors {
        return Err(EccError::Uncorrectable);
    }

    // Full locator Ψ = Γ·σ and evaluator Ω = Ψ·S mod x^P.
    let psi = gamma.mul_trunc(&sigma, CAP);
    let omega = psi.mul_trunc(&synd, p);

    let mut fixed = [0u8; CAP];
    let work = fixed.get_mut(..n).ok_or(EccError::InvalidBlockLength)?;
    work.copy_from_slice(block);

    let erasure_exps = erased
        .iter()
        .take(n)
        .enumerate()
        .filter(|(_, e)| **e)
        .map(|(j, _)| (exponent_of(j), false));
    let error_iter = error_exps.iter().take(found).map(|&q| (q, true));
    for (q, is_error) in error_iter.chain(erasure_exps) {
        let x = gf::exp(q);
        let x_inv = gf::exp(gf::ORDER - q);
        // Forney with first root α^0: Y = X · Ω(X^−1) / Ψ'(X^−1).
        let den = psi.eval_derivative(x_inv);
        let value = gf::div(gf::mul(x, omega.eval(x_inv)), den).ok_or(EccError::Uncorrectable)?;
        // A located error must change its byte.
        if is_error && value == 0 {
            return Err(EccError::Uncorrectable);
        }
        let byte = work.get_mut(n - 1 - q).ok_or(EccError::Uncorrectable)?;
        *byte ^= value;
    }

    // 4.9: the recomputed syndromes of the corrected block must all be zero.
    if (0..p).any(|i| eval_at_power(work, i) != 0) {
        return Err(EccError::Uncorrectable);
    }

    let changed = block.iter().zip(work.iter()).filter(|(a, b)| a != b).count();
    block.copy_from_slice(work);
    Ok(changed)
}
