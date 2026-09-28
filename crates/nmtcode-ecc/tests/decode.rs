//! Tests of the single-block Reed-Solomon decoder against the requirements of 4.9.
//!
//! Terms follow 4.9: e errors (wrong bytes at unknown positions) and s erasures (positions
//! marked before decoding, values ignored) in a block with P parity bytes.

use nmtcode_ecc::{EccError, rs};
use proptest::prelude::*;
use proptest::sample::subsequence;

/// A small deterministic generator for the loop tests (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).unwrap()).unwrap()
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[7]
    }

    fn nonzero_byte(&mut self) -> u8 {
        u8::try_from(1 + self.below(255)).unwrap()
    }

    /// `count` distinct positions below `n`, in random order.
    fn positions(&mut self, n: usize, count: usize) -> Vec<usize> {
        let mut all: Vec<usize> = (0..n).collect();
        for i in 0..count {
            let j = i + self.below(n - i);
            all.swap(i, j);
        }
        all.truncate(count);
        all
    }
}

fn is_codeword(block: &[u8], parity: usize) -> bool {
    rs::syndromes(block, parity).unwrap().iter().all(|&s| s == 0)
}

/// Block length n, parity count P (even, 2 ≤ P < n) and a message of n − P bytes.
fn block_params() -> impl Strategy<Value = (usize, usize, Vec<u8>)> {
    (3usize..=255).prop_flat_map(|n| (Just(n), 1..=(n - 1) / 2)).prop_flat_map(|(n, half)| {
        let parity = 2 * half;
        (Just(n), Just(parity), prop::collection::vec(any::<u8>(), n - parity))
    })
}

/// A block with e errors and s erasures, 2e + s ≤ P: (codeword, received, parity, erasures).
fn correctable_case() -> impl Strategy<Value = (Vec<u8>, Vec<u8>, usize, Vec<usize>)> {
    block_params()
        .prop_flat_map(|(n, parity, message)| {
            (Just(n), Just(parity), Just(message), 0..=parity / 2)
        })
        .prop_flat_map(|(n, parity, message, errors)| {
            let erasures = 0..=parity - 2 * errors;
            (Just(n), Just(parity), Just(message), Just(errors), erasures)
        })
        .prop_flat_map(|(n, parity, message, errors, erasures)| {
            let positions =
                subsequence((0..n).collect::<Vec<_>>(), errors + erasures).prop_shuffle();
            let error_values = prop::collection::vec(1..=255u8, errors);
            let erasure_values = prop::collection::vec(any::<u8>(), erasures);
            (Just(parity), Just(message), positions, error_values, erasure_values)
        })
        .prop_map(|(parity, message, positions, error_values, erasure_values)| {
            let codeword = rs::encode(&message, parity).unwrap();
            let mut received = codeword.clone();
            let (error_at, erased_at) = positions.split_at(error_values.len());
            for (&j, &v) in error_at.iter().zip(&error_values) {
                received[j] ^= v;
            }
            for (&j, &v) in erased_at.iter().zip(&erasure_values) {
                received[j] = v;
            }
            (codeword, received, parity, erased_at.to_vec())
        })
}

fn changed(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

proptest! {
    /// 4.9: "A reader MUST correct every block with 2e ≤ P and no erasures."
    #[test]
    fn corrects_errors_up_to_half_the_parity(
        (n, parity, message) in block_params(),
        seed in any::<u64>(),
    ) {
        let mut rng = Rng(seed | 1);
        let codeword = rs::encode(&message, parity).unwrap();
        let errors = rng.below(parity / 2 + 1);
        let mut received = codeword.clone();
        for j in rng.positions(n, errors) {
            received[j] ^= rng.nonzero_byte();
        }
        let mut block = received.clone();
        prop_assert_eq!(rs::decode(&mut block, parity, &[]), Ok(errors));
        prop_assert_eq!(block, codeword);
    }

    /// 4.9: erasures alone, s ≤ P, are always corrected. The count is of bytes whose value
    /// changed: an erased byte that held the right value is not counted.
    #[test]
    fn corrects_erasures_up_to_the_parity(
        (n, parity, message) in block_params(),
        seed in any::<u64>(),
    ) {
        let mut rng = Rng(seed | 1);
        let codeword = rs::encode(&message, parity).unwrap();
        let count = rng.below(parity + 1);
        let erasures = rng.positions(n, count);
        let mut received = codeword.clone();
        for &j in &erasures {
            received[j] = rng.byte();
        }
        let mut block = received.clone();
        prop_assert_eq!(rs::decode(&mut block, parity, &erasures), Ok(changed(&received, &codeword)));
        prop_assert_eq!(block, codeword);
    }

    /// 4.9: "A reader that passes erasures to the decoder MUST correct every block with
    /// 2e + s ≤ P."
    #[test]
    fn corrects_errors_and_erasures((codeword, received, parity, erasures) in correctable_case()) {
        let mut block = received.clone();
        let result = rs::decode(&mut block, parity, &erasures);
        prop_assert_eq!(result, Ok(changed(&received, &codeword)));
        prop_assert_eq!(block, codeword);
    }

    /// Beyond the limit, 2e + s > P (with s ≤ P). The test asserts what 4.9 allows and nothing
    /// more: the decoder either rejects the block with `Uncorrectable` and leaves it unchanged,
    /// or returns a miscorrection. A miscorrection is not hidden by the test; it must be a valid
    /// codeword, must differ from the sent codeword (a bounded-distance decoder cannot reach a
    /// codeword more than (P − s)/2 errors away), and must change at most s + (P − s)/2 bytes.
    #[test]
    fn beyond_the_limit_rejects_or_miscorrects_to_a_codeword(
        (n, parity, message) in block_params(),
        seed in any::<u64>(),
    ) {
        let mut rng = Rng(seed | 1);
        let codeword = rs::encode(&message, parity).unwrap();
        let erasure_count = rng.below(parity + 1);
        let min_errors = (parity - erasure_count) / 2 + 1;
        prop_assume!(min_errors + erasure_count <= n);
        let errors = min_errors + rng.below(n - erasure_count - min_errors + 1);
        let positions = rng.positions(n, errors + erasure_count);
        let (error_at, erased_at) = positions.split_at(errors);
        let mut received = codeword.clone();
        for &j in error_at {
            received[j] ^= rng.nonzero_byte();
        }
        for &j in erased_at {
            received[j] = rng.byte();
        }
        let mut block = received.clone();
        match rs::decode(&mut block, parity, erased_at) {
            Err(e) => {
                prop_assert_eq!(e, EccError::Uncorrectable);
                prop_assert_eq!(&block, &received);
            }
            Ok(count) => {
                prop_assert!(is_codeword(&block, parity));
                prop_assert_ne!(&block, &codeword);
                prop_assert_eq!(count, changed(&received, &block));
                prop_assert!(count <= erasure_count + (parity - erasure_count) / 2);
            }
        }
    }

    /// Arbitrary input never panics. Invalid parameters give their error; a success is always a
    /// codeword; a rejection leaves the block unchanged.
    #[test]
    fn arbitrary_input_never_panics(
        block in prop::collection::vec(any::<u8>(), 0..=300),
        parity in 0usize..=300,
        erasures in prop::collection::vec(0usize..=300, 0..=40),
    ) {
        let mut work = block.clone();
        match rs::decode(&mut work, parity, &erasures) {
            Ok(count) => {
                prop_assert!(is_codeword(&work, parity));
                prop_assert_eq!(count, changed(&block, &work));
            }
            Err(e) => {
                prop_assert_eq!(&work, &block);
                let valid_parity = (2..=254).contains(&parity) && parity.is_multiple_of(2);
                let valid_len = (3..=255).contains(&block.len()) && block.len() > parity;
                let expected_kinds: &[EccError] = if !valid_parity {
                    &[EccError::InvalidParity]
                } else if !valid_len {
                    &[EccError::InvalidBlockLength]
                } else if erasures.iter().any(|&j| j >= block.len()) {
                    &[EccError::ErasureOutOfRange]
                } else {
                    &[EccError::Uncorrectable]
                };
                prop_assert!(expected_kinds.contains(&e), "{:?}", e);
            }
        }
    }
}

#[test]
fn every_single_error_in_short_blocks() {
    // P = 2 corrects one error: every position and every non-zero error value.
    for n in 3..=40usize {
        let message: Vec<u8> = (0..n - 2).map(|i| u8::try_from(i * 37 % 256).unwrap()).collect();
        let codeword = rs::encode(&message, 2).unwrap();
        for j in 0..n {
            for v in 1..=255u8 {
                let mut block = codeword.clone();
                block[j] ^= v;
                assert_eq!(rs::decode(&mut block, 2, &[]), Ok(1), "n = {n}, j = {j}, v = {v}");
                assert_eq!(block, codeword);
            }
        }
    }
}

#[test]
fn every_pair_of_errors_and_every_pair_of_erasures() {
    // P = 4: every pair of error positions, and every pair of erasures plus one error.
    let mut rng = Rng(0x0123_4567_89AB_CDEF);
    for n in [5usize, 6, 12, 31, 64] {
        let message: Vec<u8> = (0..n - 4).map(|_| rng.byte()).collect();
        let codeword = rs::encode(&message, 4).unwrap();
        for a in 0..n {
            for b in a + 1..n {
                let mut block = codeword.clone();
                block[a] ^= rng.nonzero_byte();
                block[b] ^= rng.nonzero_byte();
                assert_eq!(rs::decode(&mut block, 4, &[]), Ok(2), "n = {n}, errors {a} {b}");
                assert_eq!(block, codeword);

                let mut block = codeword.clone();
                block[a] = !block[a];
                block[b] = !block[b];
                let c = (b + 1) % n;
                let with_error = c != a;
                if with_error {
                    block[c] ^= rng.nonzero_byte();
                }
                let expected = 2 + usize::from(with_error);
                assert_eq!(rs::decode(&mut block, 4, &[a, b]), Ok(expected), "n = {n}");
                assert_eq!(block, codeword);
            }
        }
    }
}

#[test]
fn longest_and_shortest_blocks() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    // n = 255, P = 254, k = 1: 127 errors, or 254 erasures, or any mix.
    for (errors, erasures) in [(127usize, 0usize), (0, 254), (60, 134), (1, 252)] {
        let codeword = rs::encode(&[rng.byte()], 254).unwrap();
        let positions = rng.positions(255, errors + erasures);
        let (error_at, erased_at) = positions.split_at(errors);
        let mut block = codeword.clone();
        for &j in error_at {
            block[j] ^= rng.nonzero_byte();
        }
        for &j in erased_at {
            block[j] = rng.byte();
        }
        let received = block.clone();
        let result = rs::decode(&mut block, 254, erased_at);
        assert_eq!(result, Ok(changed(&received, &codeword)), "e = {errors}, s = {erasures}");
        assert_eq!(block, codeword);
    }
    // n = 3, P = 2, k = 1: the shortest block of 4.2.
    let codeword = rs::encode(&[0xA5], 2).unwrap();
    for j in 0..3 {
        let mut block = codeword.clone();
        block[j] ^= 0x5A;
        assert_eq!(rs::decode(&mut block, 2, &[]), Ok(1));
        assert_eq!(block, codeword);
    }
}

#[test]
fn clean_blocks_decode_with_no_change() {
    let message: Vec<u8> = (0..34u8).collect();
    let codeword = rs::encode(&message, 16).unwrap();
    let mut block = codeword.clone();
    assert_eq!(rs::decode(&mut block, 16, &[]), Ok(0));
    // Erasures on correct bytes change nothing and count nothing.
    assert_eq!(rs::decode(&mut block, 16, &[0, 5, 49]), Ok(0));
    assert_eq!(block, codeword);
}

#[test]
fn more_erasures_than_parity_are_rejected() {
    // 4.9: a bounded-distance decoder MUST reject the block when s > P.
    let message: Vec<u8> = (0..20u8).collect();
    let codeword = rs::encode(&message, 6).unwrap();
    let mut block = codeword.clone();
    let seven: Vec<usize> = (0..7).collect();
    assert_eq!(rs::decode(&mut block, 6, &seven), Err(EccError::Uncorrectable));
    // Even with every erased byte correct: the rule is on s, not on the values.
    assert_eq!(block, codeword);
    // Duplicate marks count once, so six distinct positions are within the limit.
    block[0] ^= 1;
    block[3] ^= 2;
    let marks = [0usize, 0, 1, 2, 3, 3, 4, 5, 5];
    assert_eq!(rs::decode(&mut block, 6, &marks), Ok(2));
    assert_eq!(block, codeword);
}

#[test]
fn a_root_outside_the_shortened_block_is_rejected() {
    // 4.9: reject when a root of the error locator points outside the n bytes of the shortened
    // block. Adding Y · (x^q mod g) to a codeword gives the syndromes of one error Y at x^q,
    // q ≥ n, which is a position the shortened block does not have. The weight of x^q mod g is
    // at most P, so no pattern of 2e ≤ P explains the syndromes and the block must be rejected.
    let (n, parity) = (20usize, 4usize);
    let message: Vec<u8> = (0..16u8).map(|i| i.wrapping_mul(29)).collect();
    let codeword = rs::encode(&message, parity).unwrap();
    let encoder = rs::Encoder::new(parity).unwrap();
    for q in n..=254 {
        // x^q mod g is the parity of the message x^(q − P), that is [1, 0, …, 0].
        let mut unit = vec![0u8; q - parity + 1];
        unit[0] = 1;
        let remainder = encoder.parity(&unit).unwrap();
        for y in [1u8, 0x53, 0xFF] {
            let mut block = codeword.clone();
            for (byte, &r) in block[n - parity..].iter_mut().zip(&remainder) {
                *byte ^= nmtcode_ecc::gf::mul(y, r);
            }
            let received = block.clone();
            assert_eq!(
                rs::decode(&mut block, parity, &[]),
                Err(EccError::Uncorrectable),
                "q = {q}"
            );
            assert_eq!(block, received);
        }
    }
}

#[test]
fn miscorrection_rate_beyond_the_limit() {
    // 4.9 promises no detection beyond 2e + s ≤ P and cites a miscorrection chance of roughly
    // 1/t! with t = P/2. This test counts miscorrections instead of hiding them. For every
    // outcome it asserts the invariants of `beyond_the_limit_rejects_or_miscorrects_to_a_codeword`.
    // With P = 16 (1/8! ≈ 2.5e-5) it also asserts that at most 3 of 3000 far patterns
    // miscorrect. With P = 2 and n = 255 almost every received word lies within one byte of some
    // codeword, so it asserts that miscorrections do occur: the decoder does not reject them and
    // the container's CRC-32C (chapter 3) is the check that must catch them.
    let mut rng = Rng(0x5EED_0000_0000_0001);
    for (parity, errors, trials) in [(16usize, 9usize, 3000usize), (2, 2, 300)] {
        let mut miscorrections = 0usize;
        for _ in 0..trials {
            let message: Vec<u8> = (0..255 - parity).map(|_| rng.byte()).collect();
            let codeword = rs::encode(&message, parity).unwrap();
            let mut block = codeword.clone();
            for j in rng.positions(255, errors) {
                block[j] ^= rng.nonzero_byte();
            }
            let received = block.clone();
            match rs::decode(&mut block, parity, &[]) {
                Err(e) => {
                    assert_eq!(e, EccError::Uncorrectable);
                    assert_eq!(block, received);
                }
                Ok(count) => {
                    miscorrections += 1;
                    assert!(is_codeword(&block, parity));
                    assert_ne!(block, codeword);
                    assert!(count <= parity / 2);
                }
            }
        }
        if parity == 16 {
            assert!(miscorrections <= 3, "P = 16: {miscorrections} of {trials} miscorrected");
        } else {
            assert!(miscorrections > 0, "P = 2: {miscorrections} of {trials} miscorrected");
        }
    }
}

#[test]
fn invalid_parameters() {
    let mut block = vec![0u8; 10];
    assert_eq!(rs::decode(&mut block, 3, &[]), Err(EccError::InvalidParity));
    assert_eq!(rs::decode(&mut block, 0, &[]), Err(EccError::InvalidParity));
    assert_eq!(rs::decode(&mut block, 256, &[]), Err(EccError::InvalidParity));
    assert_eq!(rs::decode(&mut block, 10, &[]), Err(EccError::InvalidBlockLength));
    assert_eq!(rs::decode(&mut block, 2, &[10]), Err(EccError::ErasureOutOfRange));
    assert_eq!(rs::decode(&mut [0u8; 2], 2, &[]), Err(EccError::InvalidBlockLength));
    assert_eq!(rs::decode(&mut [0u8; 256], 2, &[]), Err(EccError::InvalidBlockLength));
    assert_eq!(rs::encode(&[], 2), Err(EccError::InvalidBlockLength));
    assert_eq!(rs::encode(&[0u8; 254], 2), Err(EccError::InvalidBlockLength));
    assert_eq!(rs::encode(&[0u8; 5], 5), Err(EccError::InvalidParity));
    assert_eq!(rs::generator(1), Err(EccError::InvalidParity));
    assert_eq!(rs::generator(256), Err(EccError::InvalidParity));
    assert_eq!(rs::generator(254).map(|g| g.len()), Ok(255));
    assert!(rs::syndromes(&[0u8; 3], 4).is_err());
    let encoder = rs::Encoder::new(4).unwrap();
    assert_eq!(encoder.parity_len(), 4);
    assert_eq!(encoder.parity_into(&[1, 2], &mut [0u8; 3]), Err(EccError::InvalidParity));
}

#[test]
fn generator_roots_and_parity_division() {
    // 4.3: g_P has the roots α^0, …, α^(P−1). 4.4: c(α^i) = 0 for every encoded block.
    for parity in (2..=254).step_by(2) {
        let g = rs::generator(parity).unwrap();
        assert_eq!(g.len(), parity + 1);
        assert_eq!(g[0], 1);
        for i in 0..parity {
            let x = nmtcode_ecc::gf::exp(i);
            let value = g.iter().fold(0u8, |acc, &c| nmtcode_ecc::gf::mul(acc, x) ^ c);
            assert_eq!(value, 0, "g_{parity}(α^{i})");
        }
    }
    let mut rng = Rng(42);
    for parity in [2usize, 8, 16, 52, 154, 254] {
        let message: Vec<u8> = (0..255 - parity).map(|_| rng.byte()).collect();
        let codeword = rs::encode(&message, parity).unwrap();
        assert_eq!(&codeword[..message.len()], message.as_slice());
        assert!(is_codeword(&codeword, parity));
    }
}
