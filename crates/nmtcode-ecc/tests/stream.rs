//! Tests of message assignment and the interleaved codeword stream (4.8), and of
//! `decode_stream` against 4.9.

use nmtcode_ecc::{BlockSplit, Decoded, EccError, decode_stream, encode_stream, rs, split};
use proptest::prelude::*;

/// A small deterministic generator (xorshift64*).
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

    fn positions(&mut self, n: usize, count: usize) -> Vec<usize> {
        let mut all: Vec<usize> = (0..n).collect();
        for i in 0..count {
            let j = i + self.below(n - i);
            all.swap(i, j);
        }
        all.truncate(count);
        all
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

/// The literal pseudocode of 4.8.1 and 4.8.2, written from the text and independent of the
/// closed form used by the library: blocks by contiguous runs of the message, then the
/// round-robin loop over byte positions.
fn reference_stream(split: &BlockSplit, message: &[u8]) -> Vec<u8> {
    let lengths: Vec<usize> = split.blocks().map(|b| b.len).collect();
    let parity = split.parity_per_block();
    let mut blocks = Vec::new();
    let mut offset = 0;
    for &n in &lengths {
        let k = n - parity;
        blocks.push(rs::encode(&message[offset..offset + k], parity).unwrap());
        offset += k;
    }
    let n_max = *lengths.iter().max().unwrap();
    let mut stream = Vec::new();
    for j in 0..n_max {
        for (b, block) in blocks.iter().enumerate() {
            if j < lengths[b] {
                stream.push(block[j]);
            }
        }
    }
    stream
}

fn valid_split() -> impl Strategy<Value = BlockSplit> {
    (0u8..=3, 0usize..=3000).prop_map(|(level, extra)| {
        let minimum = if level == 3 { 5 } else { 3 };
        split(minimum + extra, level).unwrap()
    })
}

proptest! {
    /// 4.8: encode_stream equals the literal loops of 4.8.1 and 4.8.2, and a clean stream
    /// decodes to the message with nothing corrected.
    #[test]
    fn stream_matches_the_pseudocode(s in valid_split(), seed in any::<u64>()) {
        let mut rng = Rng(seed | 1);
        let message = rng.bytes(s.capacity());
        let stream = encode_stream(&s, &message).unwrap();
        prop_assert_eq!(stream.len(), s.codewords());
        prop_assert_eq!(&stream, &reference_stream(&s, &message));
        let decoded = decode_stream(&s, &stream, &[]).unwrap();
        prop_assert_eq!(decoded, Decoded { message, corrected: 0 });
        for i in 0..s.codewords() {
            let (block, byte) = s.locate(i).unwrap();
            prop_assert_eq!(s.stream_index(block, byte), Some(i));
        }
    }

    /// 4.9 over a whole layer: every block gets its own e errors and s erasures with
    /// 2e + s ≤ P, placed through the stream order of 4.8.2. The whole message comes back and
    /// the count is the number of bytes whose value changed.
    #[test]
    fn corrects_every_block_within_the_limit(s in valid_split(), seed in any::<u64>()) {
        let mut rng = Rng(seed | 1);
        let message = rng.bytes(s.capacity());
        let sent = encode_stream(&s, &message).unwrap();
        let mut received = sent.clone();
        let mut erasures = Vec::new();
        let parity = s.parity_per_block();
        for block in s.blocks() {
            let errors = rng.below(parity / 2 + 1);
            let erased = rng.below(parity - 2 * errors + 1);
            let positions = rng.positions(block.len, errors + erased);
            let (error_at, erased_at) = positions.split_at(errors);
            for &j in error_at {
                received[s.stream_index(block.index, j).unwrap()] ^= rng.nonzero_byte();
            }
            for &j in erased_at {
                let index = s.stream_index(block.index, j).unwrap();
                received[index] = rng.byte();
                erasures.push(index);
            }
        }
        let expected = sent.iter().zip(&received).filter(|(a, b)| a != b).count();
        let decoded = decode_stream(&s, &received, &erasures).unwrap();
        prop_assert_eq!(decoded, Decoded { message, corrected: expected });
    }

    /// Arbitrary streams and erasure lists never panic. A wrong length or an out-of-range
    /// erasure gives its error; otherwise the result is a message of K bytes or `Uncorrectable`.
    #[test]
    fn arbitrary_streams_never_panic(
        s in valid_split(),
        stream_len_delta in -2isize..=2,
        seed in any::<u64>(),
        erasure_count in 0usize..=64,
    ) {
        let mut rng = Rng(seed | 1);
        let len = s.codewords().saturating_add_signed(stream_len_delta);
        let stream = rng.bytes(len);
        let erasures: Vec<usize> =
            (0..erasure_count).map(|_| rng.below(s.codewords() + 3)).collect();
        match decode_stream(&s, &stream, &erasures) {
            Ok(decoded) => {
                prop_assert_eq!(len, s.codewords());
                prop_assert_eq!(decoded.message.len(), s.capacity());
            }
            Err(EccError::StreamLength) => prop_assert_ne!(len, s.codewords()),
            Err(EccError::ErasureOutOfRange) => {
                prop_assert!(erasures.iter().any(|&i| i >= s.codewords()));
            }
            Err(e) => prop_assert_eq!(e, EccError::Uncorrectable),
        }
    }
}

#[test]
fn a_burst_of_b_times_half_p_codewords_is_corrected() {
    // 4.8.2: any run of up to B consecutive codewords touches each block at most once, so a
    // run of B · P/2 wrong codewords puts at most P/2 errors in each block, and a run of B · P
    // erased codewords at most P erasures in each block.
    let mut rng = Rng(7);
    for (n, level) in [(512usize, 1u8), (1311, 0), (13000, 2), (1000, 3), (256, 0)] {
        let s = split(n, level).unwrap();
        let message = rng.bytes(s.capacity());
        let sent = encode_stream(&s, &message).unwrap();
        let (b, p) = (s.block_count(), s.parity_per_block());
        for start in [0, n / 3, n - b * p / 2] {
            let mut received = sent.clone();
            for byte in &mut received[start..start + b * p / 2] {
                *byte ^= rng.nonzero_byte();
            }
            let decoded = decode_stream(&s, &received, &[]).unwrap();
            assert_eq!(decoded.message, message, "N = {n}, level {level}, errors at {start}");
            assert_eq!(decoded.corrected, b * p / 2);
        }
        for start in [0, n - b * p] {
            let mut received = sent.clone();
            let erasures: Vec<usize> = (start..start + b * p).collect();
            for &i in &erasures {
                received[i] = 0;
            }
            let decoded = decode_stream(&s, &received, &erasures).unwrap();
            assert_eq!(decoded.message, message, "N = {n}, level {level}, erasures at {start}");
        }
    }
}

#[test]
fn one_block_beyond_the_limit_fails_the_layer() {
    // 4.9: if any block is rejected the layer is undecodable, and no message bytes come back.
    // Block 1 of N = 512 at level 1 (P = 52) gets 27 errors, one more than P/2. The test asserts
    // rejection; this relies on the miscorrection chance being about 1/26! (4.9), and the fixed
    // seed makes the outcome the same on every run.
    let s = split(512, 1).unwrap();
    let mut rng = Rng(11);
    let message = rng.bytes(s.capacity());
    let mut received = encode_stream(&s, &message).unwrap();
    for j in rng.positions(171, 27) {
        received[s.stream_index(1, j).unwrap()] ^= rng.nonzero_byte();
    }
    assert_eq!(decode_stream(&s, &received, &[]), Err(EccError::Uncorrectable));
    // 53 erasures in one block: s > P.
    let mut received = encode_stream(&s, &message).unwrap();
    let erasures: Vec<usize> = (0..53).map(|j| s.stream_index(2, j).unwrap()).collect();
    received[erasures[0]] ^= 1;
    assert_eq!(decode_stream(&s, &received, &erasures), Err(EccError::Uncorrectable));
}

#[test]
fn a_large_layer_round_trips() {
    // N = 262 144 at level 3: 1029 blocks of 255 or 254 bytes, P = 154, with a few errors in
    // every block.
    let s = split(262_144, 3).unwrap();
    assert_eq!(s.block_count(), 1029);
    let mut rng = Rng(3);
    let message = rng.bytes(s.capacity());
    let mut received = encode_stream(&s, &message).unwrap();
    for i in (0..received.len()).step_by(97) {
        received[i] ^= rng.nonzero_byte();
    }
    let decoded = decode_stream(&s, &received, &[]).unwrap();
    assert_eq!(decoded.message, message);
    assert_eq!(decoded.corrected, received.len().div_ceil(97));
}

#[test]
fn length_and_range_errors() {
    let s = split(50, 1).unwrap();
    assert_eq!(encode_stream(&s, &[0u8; 33]), Err(EccError::MessageLength));
    assert_eq!(encode_stream(&s, &[0u8; 35]), Err(EccError::MessageLength));
    let stream = encode_stream(&s, &[0u8; 34]).unwrap();
    assert_eq!(decode_stream(&s, &stream[..49], &[]), Err(EccError::StreamLength));
    assert_eq!(decode_stream(&s, &[0u8; 51], &[]), Err(EccError::StreamLength));
    assert_eq!(decode_stream(&s, &stream, &[50]), Err(EccError::ErasureOutOfRange));
    assert_eq!(decode_stream(&s, &stream, &[usize::MAX]), Err(EccError::ErasureOutOfRange));
    let decoded = decode_stream(&s, &stream, &[0, 0, 49]).unwrap();
    assert_eq!(decoded, Decoded { message: vec![0u8; 34], corrected: 0 });
    assert_eq!(s.block(1), None);
    assert_eq!(s.blocks().len(), 1);
}
