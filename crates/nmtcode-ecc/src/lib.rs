//! Reed-Solomon error correction over GF(2^8) and block splitting for NMT Code.
//!
//! This crate implements chapter 4 of the NMT Code specification 0.1:
//!
//! - [`gf`]: the field GF(2^8) with the field polynomial `0x11D` and α = `0x02` (4.2);
//! - [`rs`]: the generator polynomial (4.3), systematic encoding (4.4) and a bounded-distance
//!   errors-and-erasures decoder for one block (4.9);
//! - [`split`]: the block split of a layer at one of the four levels (4.5, 4.6);
//! - [`encode_stream`] and [`decode_stream`]: message assignment into blocks and the interleaved
//!   codeword stream (4.8).
//!
//! No result of this crate is a promise against miscorrection (4.9): a reader verifies the
//! container's CRC-32C (chapter 3) after [`decode_stream`].

#![no_std]

extern crate alloc;

pub mod gf;
pub mod rs;

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// Errors of this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EccError {
    /// The error-correction level is not 0, 1, 2 or 3 (4.5).
    InvalidLevel,
    /// The layer has fewer codewords than the minimum N of its level: 3 for levels 0 to 2 and 5
    /// for level 3 (4.6). A reader maps this to `E_LAYER_TOO_SMALL`.
    LayerTooSmall,
    /// A parity count is odd, below 2 or above 254 (4.2).
    InvalidParity,
    /// A block length is outside 3..=255, or not above the parity count (4.2).
    InvalidBlockLength,
    /// The message given to [`encode_stream`] is not exactly K bytes long (4.6, 4.8.1).
    MessageLength,
    /// The codeword stream given to [`decode_stream`] is not exactly N bytes long (4.8.2).
    StreamLength,
    /// An erasure position is not below the length of the stream or block it refers to.
    ErasureOutOfRange,
    /// The decoder rejected a block by a rule of 4.9. A reader maps this to `E_ECC_FAILED`: the
    /// base layer is undecodable and no content of the symbol may be presented.
    Uncorrectable,
}

impl fmt::Display for EccError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::InvalidLevel => "error-correction level is not 0 to 3",
            Self::LayerTooSmall => "layer has fewer codewords than its level's minimum",
            Self::InvalidParity => "parity count is odd or outside 2..=254",
            Self::InvalidBlockLength => "block length is outside 3..=255 or not above the parity",
            Self::MessageLength => "message length is not the capacity K",
            Self::StreamLength => "codeword stream length is not N",
            Self::ErasureOutOfRange => "erasure position is out of range",
            Self::Uncorrectable => "a Reed-Solomon block could not be corrected",
        };
        f.write_str(text)
    }
}

impl core::error::Error for EccError {}

/// The number of error-correction levels (4.5): the level is a 2-bit field.
pub const LEVEL_COUNT: usize = 4;

/// The one-letter names of levels 0 to 3 (4.5).
pub const LEVEL_NAMES: [char; LEVEL_COUNT] = ['L', 'M', 'Q', 'H'];

/// The parity numerators q\[lvl\] of levels 0 to 3: every block has P · 100 ≥ n · q\[lvl\]
/// (4.5, 4.6).
pub const PARITY_NUMERATOR: [usize; LEVEL_COUNT] = [15, 30, 50, 60];

/// The smallest number of codewords N a layer may have at levels 0 to 3 (4.6).
pub const MIN_CODEWORDS: [usize; LEVEL_COUNT] = [3, 3, 3, 5];

/// One block of a [`BlockSplit`] (4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// The block number b, 0 ≤ b < B. Long blocks come first.
    pub index: usize,
    /// The block length n\[b\] in bytes.
    pub len: usize,
    /// The parity count P, the same for every block of the split.
    pub parity_len: usize,
    /// The message bytes k\[b\] = n\[b\] − P.
    pub message_len: usize,
    /// The index in the message stream M of the block's first message byte (4.8.1).
    pub message_start: usize,
}

/// The block split of one layer: the output of the algorithm of 4.6.
///
/// Built only by [`split`], so every value satisfies the properties listed in 4.6: every block
/// length is in 3..=255, two lengths differ by at most one, the lengths add up to N, P is even
/// and at least 2, and every block has at least one message byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSplit {
    codewords: usize,
    level: u8,
    block_count: usize,
    short_len: usize,
    long_count: usize,
    parity_len: usize,
    capacity: usize,
}

/// Splits a layer of `n_codewords` codewords at level `level` into blocks, by the algorithm of
/// 4.6.
///
/// # Errors
///
/// - [`EccError::InvalidLevel`] if `level` is above 3.
/// - [`EccError::LayerTooSmall`] if `n_codewords` is below the level's minimum
///   ([`MIN_CODEWORDS`]).
pub fn split(n_codewords: usize, level: u8) -> Result<BlockSplit, EccError> {
    let lvl = usize::from(level);
    let numerator = *PARITY_NUMERATOR.get(lvl).ok_or(EccError::InvalidLevel)?;
    let minimum = *MIN_CODEWORDS.get(lvl).ok_or(EccError::InvalidLevel)?;
    if n_codewords < minimum {
        return Err(EccError::LayerTooSmall);
    }
    // B = ceil_div(N, 255): the fewest blocks with n ≤ 255. N ≥ 3, so B ≥ 1.
    let block_count = n_codewords.div_ceil(rs::MAX_BLOCK_LEN);
    let short_len = n_codewords / block_count;
    let long_count = n_codewords % block_count;
    let max_len = n_codewords.div_ceil(block_count);
    // P = 2 · ceil_div(n_max · q, 200). n_max ≤ 255 and q ≤ 60, so the product is small.
    let parity_len = 2 * (max_len * numerator).div_ceil(200);
    // 4.6 shows that every block keeps a message byte from N_min upward; checking it here turns
    // a broken invariant into an error instead of a wrong layout.
    rs::check_block(short_len, parity_len).map_err(|_| EccError::LayerTooSmall)?;
    rs::check_block(max_len, parity_len).map_err(|_| EccError::LayerTooSmall)?;
    let capacity = block_count
        .checked_mul(parity_len)
        .and_then(|parity_total| n_codewords.checked_sub(parity_total))
        .ok_or(EccError::LayerTooSmall)?;
    Ok(BlockSplit {
        codewords: n_codewords,
        level,
        block_count,
        short_len,
        long_count,
        parity_len,
        capacity,
    })
}

impl BlockSplit {
    /// N, the number of codewords in the layer.
    pub fn codewords(&self) -> usize {
        self.codewords
    }

    /// The error-correction level, 0 to 3.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// K, the message capacity in bytes: N − B · P.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The parity bytes of the whole layer: B · P = N − K.
    pub fn parity_total(&self) -> usize {
        self.codewords - self.capacity
    }

    /// P, the parity count of every block.
    pub fn parity_per_block(&self) -> usize {
        self.parity_len
    }

    /// B, the number of blocks.
    pub fn block_count(&self) -> usize {
        self.block_count
    }

    /// s = floor(N / B), the length of a short block (4.6).
    pub fn short_block_len(&self) -> usize {
        self.short_len
    }

    /// r = N mod B, the number of long blocks, each s + 1 bytes (4.6).
    pub fn long_block_count(&self) -> usize {
        self.long_count
    }

    /// `n_max = ceil_div(N, B)`, the length of the longest block (4.6).
    pub fn max_block_len(&self) -> usize {
        self.short_len + usize::from(self.long_count > 0)
    }

    /// Block `index`, or `None` if `index` is not below B.
    pub fn block(&self, index: usize) -> Option<Block> {
        (index < self.block_count).then(|| self.make_block(index))
    }

    /// The blocks in block order, long blocks first (4.6).
    pub fn blocks(&self) -> impl ExactSizeIterator<Item = Block> + '_ {
        (0..self.block_count).map(|index| self.make_block(index))
    }

    /// Block `index`, for `index` below B. [`split`] guarantees s > P, so the subtractions
    /// cannot saturate for a valid split.
    fn make_block(&self, index: usize) -> Block {
        let is_long = index < self.long_count;
        let len = self.short_len + usize::from(is_long);
        let short_message = self.short_len.saturating_sub(self.parity_len);
        Block {
            index,
            len,
            parity_len: self.parity_len,
            message_len: len.saturating_sub(self.parity_len),
            // Every earlier block holds short_message bytes, and each earlier long block one more.
            message_start: index * short_message + index.min(self.long_count),
        }
    }

    /// The stream index of byte `byte` of block `block` (4.8.2), or `None` if the block has no
    /// such byte. Byte j of block b is c\[j · B + b\].
    pub fn stream_index(&self, block: usize, byte: usize) -> Option<usize> {
        let len = self.block(block)?.len;
        if byte >= len {
            return None;
        }
        byte.checked_mul(self.block_count)?.checked_add(block)
    }

    /// The (block, byte of block) pair that stream codeword c\[`index`\] comes from (4.8.2), or
    /// `None` if `index` is not below N.
    pub fn locate(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.codewords {
            return None;
        }
        // Every block has a byte at every position below s, and the long blocks one at s, so
        // c[i] is byte i / B of block i mod B throughout the stream.
        Some((index % self.block_count, index / self.block_count))
    }
}

/// The result of [`decode_stream`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The K message bytes M\[0\], …, M\[K−1\] (4.8.1).
    pub message: Vec<u8>,
    /// The number of codeword bytes whose value the decoder changed, over all blocks. An erased
    /// byte that already held the right value is not counted.
    pub corrected: usize,
}

/// Encodes the K-byte message stream into the N-byte interleaved codeword stream of 4.8.
///
/// The message fills the blocks in block order (4.8.1), each block is encoded by 4.4, and the
/// codewords leave the blocks round-robin by byte position (4.8.2).
///
/// # Errors
///
/// [`EccError::MessageLength`] if `message.len()` is not [`BlockSplit::capacity`].
pub fn encode_stream(split: &BlockSplit, message: &[u8]) -> Result<Vec<u8>, EccError> {
    if message.len() != split.capacity {
        return Err(EccError::MessageLength);
    }
    let encoder = rs::Encoder::new(split.parity_len)?;
    let mut stream = vec![0u8; split.codewords];
    let mut buffer = [0u8; rs::MAX_BLOCK_LEN];
    for block in split.blocks() {
        let data = message
            .get(block.message_start..block.message_start + block.message_len)
            .ok_or(EccError::MessageLength)?;
        let codeword = buffer.get_mut(..block.len).ok_or(EccError::InvalidBlockLength)?;
        let (message_part, parity_part) =
            codeword.split_at_mut_checked(block.message_len).ok_or(EccError::InvalidBlockLength)?;
        message_part.copy_from_slice(data);
        encoder.parity_into(data, parity_part)?;
        // Byte j of block b is c[j · B + b]; the positions b, b + B, … below N are exactly the
        // n[b] bytes of the block.
        let positions = stream.iter_mut().skip(block.index).step_by(split.block_count);
        for (slot, &byte) in positions.zip(codeword.iter()) {
            *slot = byte;
        }
    }
    Ok(stream)
}

/// Decodes the N-byte codeword stream of 4.8 and returns the K message bytes.
///
/// `erasures` lists stream indices that the reader marked as unreliable (4.9); duplicates count
/// once. Each block is corrected when 2e + s ≤ P for its e errors and s erasures. The result is
/// still subject to the container's CRC-32C check (chapter 3): when 2e + s > P a block can be
/// miscorrected without any sign of failure (4.9).
///
/// # Errors
///
/// - [`EccError::StreamLength`] if `stream.len()` is not N.
/// - [`EccError::ErasureOutOfRange`] if an erasure index is not below N.
/// - [`EccError::Uncorrectable`] if any block is rejected by a rule of 4.9 (`E_ECC_FAILED`).
///   No message bytes are returned in that case.
pub fn decode_stream(
    split: &BlockSplit,
    stream: &[u8],
    erasures: &[usize],
) -> Result<Decoded, EccError> {
    if stream.len() != split.codewords {
        return Err(EccError::StreamLength);
    }
    let mut erased = vec![false; if erasures.is_empty() { 0 } else { split.codewords }];
    for &index in erasures {
        *erased.get_mut(index).ok_or(EccError::ErasureOutOfRange)? = true;
    }

    let mut message = Vec::with_capacity(split.capacity);
    let mut corrected = 0usize;
    let mut buffer = [0u8; rs::MAX_BLOCK_LEN];
    let mut marks = [0usize; rs::MAX_BLOCK_LEN];
    for block in split.blocks() {
        let codeword = buffer.get_mut(..block.len).ok_or(EccError::InvalidBlockLength)?;
        let received = stream.iter().skip(block.index).step_by(split.block_count);
        for (slot, &byte) in codeword.iter_mut().zip(received) {
            *slot = byte;
        }
        let mut mark_count = 0usize;
        let flags = erased.iter().skip(block.index).step_by(split.block_count);
        for (byte_index, _) in flags.enumerate().filter(|(_, flag)| **flag) {
            *marks.get_mut(mark_count).ok_or(EccError::ErasureOutOfRange)? = byte_index;
            mark_count += 1;
        }
        let block_marks = marks.get(..mark_count).ok_or(EccError::ErasureOutOfRange)?;
        corrected += rs::decode(codeword, block.parity_len, block_marks)?;
        let data = codeword.get(..block.message_len).ok_or(EccError::InvalidBlockLength)?;
        message.extend_from_slice(data);
    }
    Ok(Decoded { message, corrected })
}
