//! Codec 5 — brotli (6.10). `C` is exactly one RFC 7932 stream.
//!
//! Needs the `brotli` feature. Without it, [`decode`] fails with
//! [`CodecError::UnsupportedCodec`] and [`encode`] returns `None`, so codec 5
//! is never a candidate.
//!
//! Encoder settings: quality [`QUALITY`] (the densest brotli level; the
//! content of one symbol is small, so its cost in time is small) and the window
//! of [`window_bits`], which covers the whole content with the shortest
//! window-size field.

use alloc::vec::Vec;

use crate::CodecError;

/// Codec ID.
pub const ID: u32 = 5;

/// The brotli quality the encoder uses (0–11).
pub const QUALITY: u32 = 11;

/// The window size `WBITS` the encoder declares for content of `len` bytes.
///
/// The window covers the whole content (2^WBITS − 16 ≥ `len`) up to the RFC
/// 7932 maximum of 24, so a larger window would not change the stream. Among
/// windows that cover the content, the one with the shortest header field is
/// taken: 16 is written in 1 bit, 18–24 in 4 bits, and 10–15 and 17 in 7 bits.
pub const fn window_bits(len: usize) -> u32 {
    let mut bits = 16;
    while bits < 24 && (1usize << bits) - 16 < len {
        bits = if bits == 16 { 18 } else { bits + 1 };
    }
    bits
}

/// The coded field for `content` with quality [`QUALITY`] and the window of
/// [`window_bits`], or `None` when the `brotli` feature is off or the encoder
/// fails.
pub fn encode(content: &[u8]) -> Option<Vec<u8>> {
    encode_with(content, QUALITY, window_bits(content.len()))
}

/// The coded field for `content` with brotli quality `quality` (0–11) and
/// window size `WBITS` = `window_bits` (10–24). Returns `None` for values
/// outside those ranges, when the `brotli` feature is off, or when the encoder
/// fails. Every stream it returns is a valid codec-5 coded field; only the
/// size differs.
pub fn encode_with(content: &[u8], quality: u32, window_bits: u32) -> Option<Vec<u8>> {
    #[cfg(feature = "brotli")]
    {
        imp::encode_with(content, quality, window_bits)
    }
    #[cfg(not(feature = "brotli"))]
    {
        let _ = (content, quality, window_bits);
        None
    }
}

/// Decodes a brotli coded field with dictionary 0 (the RFC 7932 built-in
/// dictionary only) into exactly `decoded_len` bytes.
///
/// # Errors
///
/// - [`CodecError::UnsupportedCodec`]: the `brotli` feature is off.
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   an invalid stream, including a large-window stream; a stream that would
///   output more than `decoded_len` bytes (the decoder stops at the first byte
///   beyond it) or fewer; non-zero unused bits in the final byte; any byte after
///   the stream.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(decoded_len: u32, bytes: &[u8], limit: u32) -> Result<Vec<u8>, CodecError> {
    #[cfg(feature = "brotli")]
    {
        imp::decode(decoded_len, bytes, limit)
    }
    #[cfg(not(feature = "brotli"))]
    {
        let _ = (decoded_len, bytes, limit);
        Err(CodecError::UnsupportedCodec)
    }
}

#[cfg(feature = "brotli")]
mod imp {
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

    use brotli::enc::BrotliAlloc;
    use brotli::enc::StaticCommand;
    use brotli::enc::encode::{
        BrotliEncoderOperation, BrotliEncoderParameter, BrotliEncoderStateStruct,
    };
    use brotli::enc::interface::PredictionModeContextMap;
    use brotli::{InputPair, InputReferenceMut};
    use brotli_decompressor::{
        Allocator, BrotliDecompressStream, BrotliResult, BrotliState, SliceWrapper, SliceWrapperMut,
    };

    use crate::{CodecError, check_decoded_len, to_usize};

    /// Memory handed to the brotli crates: a boxed slice.
    pub(super) struct Block<T>(Box<[T]>);

    impl<T> Default for Block<T> {
        fn default() -> Self {
            Self(Box::default())
        }
    }

    impl<T> SliceWrapper<T> for Block<T> {
        fn slice(&self) -> &[T] {
            &self.0
        }
    }

    impl<T> SliceWrapperMut<T> for Block<T> {
        fn slice_mut(&mut self) -> &mut [T] {
            &mut self.0
        }
    }

    /// The global allocator, for every element type.
    #[derive(Debug, Default, Clone, Copy)]
    pub(super) struct Heap;

    impl<T: Clone + Default> Allocator<T> for Heap {
        type AllocatedMemory = Block<T>;

        fn alloc_cell(&mut self, len: usize) -> Block<T> {
            Block(vec![T::default(); len].into_boxed_slice())
        }

        fn free_cell(&mut self, _data: Block<T>) {}
    }

    impl BrotliAlloc for Heap {}

    /// The byte allocator of the decoder. It refuses any single allocation
    /// above `max_len`, which the decoder reports as an error.
    ///
    /// The largest byte buffer is the ring buffer. The decoder sizes it from
    /// the output of the meta-blocks read so far, which is at most `L` in a
    /// valid stream; measured with brotli-decompressor 6.0.1 it is at most
    /// 2 × the power of two at or above `L`, plus 566 bytes. The other byte
    /// buffers (context maps) are at most 16 KiB. The cap is twice that bound
    /// and at least 64 KiB, so a stream that declares far more output than `L`
    /// stops before the decoder allocates or fills a window for it. That keeps
    /// memory and work proportional to `L` (6.4 rules 3 and 6, 6.10 rule 2).
    struct CappedBytes {
        max_len: usize,
    }

    impl CappedBytes {
        fn for_output(len: usize) -> Self {
            let ring = len.max(1).checked_next_power_of_two().unwrap_or(usize::MAX);
            Self { max_len: ring.saturating_mul(4).max(64 * 1024) }
        }
    }

    impl Allocator<u8> for CappedBytes {
        type AllocatedMemory = Block<u8>;

        fn alloc_cell(&mut self, len: usize) -> Block<u8> {
            if len > self.max_len {
                Block::default()
            } else {
                Block(vec![0; len].into_boxed_slice())
            }
        }

        fn free_cell(&mut self, _data: Block<u8>) {}
    }

    fn no_callback(
        _: &mut PredictionModeContextMap<InputReferenceMut<'_>>,
        _: &mut [StaticCommand],
        _: InputPair<'_>,
        _: &mut Heap,
    ) {
    }

    pub(super) fn encode_with(content: &[u8], quality: u32, window_bits: u32) -> Option<Vec<u8>> {
        if quality > 11 || !(10..=24).contains(&window_bits) {
            return None;
        }
        let mut state = BrotliEncoderStateStruct::new(Heap);
        let size_hint = u32::try_from(content.len()).unwrap_or(u32::MAX);
        if !(state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_QUALITY, quality)
            && state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_LGWIN, window_bits)
            && state.set_parameter(BrotliEncoderParameter::BROTLI_PARAM_SIZE_HINT, size_hint))
        {
            return None;
        }
        let mut out = Vec::new();
        let mut chunk = vec![0u8; 4096];
        let mut available_in = content.len();
        let mut input_offset = 0;
        loop {
            let mut available_out = chunk.len();
            let mut output_offset = 0;
            let mut total_out = None;
            let before = (available_in, out.len());
            if !state.compress_stream(
                BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
                &mut available_in,
                content,
                &mut input_offset,
                &mut available_out,
                &mut chunk,
                &mut output_offset,
                &mut total_out,
                &mut no_callback,
            ) {
                return None;
            }
            out.extend_from_slice(chunk.get(..output_offset)?);
            if state.is_finished() {
                return Some(out);
            }
            if (available_in, out.len()) == before {
                // No progress: never loop forever on an encoder fault.
                return None;
            }
        }
    }

    pub(super) fn decode(
        decoded_len: u32,
        bytes: &[u8],
        limit: u32,
    ) -> Result<Vec<u8>, CodecError> {
        check_decoded_len(decoded_len, limit)?;
        let total = to_usize(decoded_len)?;
        let mut out = vec![0u8; total];
        let mut state: BrotliState<CappedBytes, Heap, Heap> =
            BrotliState::new(CappedBytes::for_output(total), Heap, Heap);
        // brotli-decompressor accepts large-window streams unless this is
        // off; 6.10 rule 1 makes them malformed.
        state.large_window = false;
        let mut available_in = bytes.len();
        let mut input_offset = 0;
        // The output buffer is exactly L bytes: a stream with more output
        // stops with "needs more output" (6.10 rule 3).
        let mut available_out = total;
        let mut output_offset = 0;
        let mut total_out = 0;
        let result = BrotliDecompressStream(
            &mut available_in,
            &mut input_offset,
            bytes,
            &mut available_out,
            &mut output_offset,
            &mut out,
            &mut total_out,
            &mut state,
        );
        // Success means the ISLAST meta-block ended with zero padding bits
        // (6.10 rule 4); any input left over is trailing data.
        match result {
            BrotliResult::ResultSuccess if available_in == 0 && output_offset == total => Ok(out),
            _ => Err(CodecError::Malformed),
        }
    }

    #[cfg(test)]
    mod tests {
        use alloc::vec;

        use brotli_decompressor::{BrotliDecompressStream, BrotliResult, BrotliState};

        use super::{CappedBytes, Heap, encode_with};

        #[test]
        fn cap_follows_output_length() {
            assert_eq!(CappedBytes::for_output(0).max_len, 64 * 1024);
            assert_eq!(CappedBytes::for_output(40_000).max_len, 256 * 1024);
            assert_eq!(CappedBytes::for_output(16 << 20).max_len, 64 << 20);
        }

        /// Runs the decoder once with a byte cap sized for `cap_for` and an
        /// output buffer of `out_len` bytes; returns the result and the bytes
        /// written.
        fn run(stream: &[u8], out_len: usize, cap_for: usize) -> (BrotliResult, usize) {
            let mut out = vec![0u8; out_len];
            let mut state: BrotliState<CappedBytes, Heap, Heap> =
                BrotliState::new(CappedBytes::for_output(cap_for), Heap, Heap);
            let (mut available_in, mut input_offset) = (stream.len(), 0);
            let (mut available_out, mut output_offset, mut total_out) = (out_len, 0, 0);
            let result = BrotliDecompressStream(
                &mut available_in,
                &mut input_offset,
                stream,
                &mut available_out,
                &mut output_offset,
                &mut out,
                &mut total_out,
                &mut state,
            );
            (result, output_offset)
        }

        #[test]
        fn cap_stops_a_stream_that_declares_more_than_l_before_it_decodes() {
            // 4 MiB of zeros is a stream of a few bytes that declares 4 MiB.
            let zeros = vec![0u8; 4 << 20];
            let stream = encode_with(&zeros, 11, 23).unwrap();
            assert!(stream.len() < 32);
            // Without a tight cap the decoder fills its window and then
            // reports that the 10-byte output is full.
            let (result, written) = run(&stream, 10, 4 << 20);
            assert!(matches!(result, BrotliResult::NeedsMoreOutput));
            assert_eq!(written, 10);
            // With the cap for L = 10 it fails before writing anything.
            let (result, written) = run(&stream, 10, 10);
            assert!(matches!(result, BrotliResult::ResultFailure));
            assert_eq!(written, 0);
            // The cap for the true length lets the stream through.
            let (result, written) = run(&stream, 4 << 20, 4 << 20);
            assert!(matches!(result, BrotliResult::ResultSuccess));
            assert_eq!(written, 4 << 20);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::window_bits;

    #[test]
    fn window_covers_content_with_shortest_field() {
        assert_eq!(window_bits(0), 16);
        assert_eq!(window_bits(65_520), 16);
        assert_eq!(window_bits(65_521), 18);
        assert_eq!(window_bits(262_128), 18);
        assert_eq!(window_bits(262_129), 19);
        assert_eq!(window_bits(16 << 20), 24);
    }
}
