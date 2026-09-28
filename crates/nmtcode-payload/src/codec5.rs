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

/// The largest window size `WBITS` a coded field for `decoded_len` bytes may declare (6.10 rule
/// 1): one more than the smallest window that covers the content (2^`WBITS` − 16 ≥ `L`), at
/// least 18 and at most 24. [`window_bits`] of the same length is never above it, and 18 is the
/// smallest window brotli declares at qualities 0 and 1.
///
/// The bound keeps the decoder's ring buffer proportional to `L`: a decoder that cannot shrink
/// its ring buffer (the first meta-block is not the last) allocates the whole declared window.
pub fn max_window_bits(decoded_len: u32) -> u32 {
    let len = u64::from(decoded_len);
    let mut covering = 10;
    while covering < 24 && (1u64 << covering) - 16 < len {
        covering += 1;
    }
    (covering + 1).clamp(18, 24)
}

/// The window size `WBITS` a brotli stream declares in its first 1 to 7 bits (RFC 7932, 9.1).
///
/// Returns `None` for an empty field and for the large-window marker (the 7-bit form with value
/// 1), which 6.10 rule 1 makes malformed.
pub fn declared_window_bits(bytes: &[u8]) -> Option<u32> {
    // RFC 7932 reads bits from the least significant bit of each byte; 7 bits fit in byte 0.
    let first = u32::from(*bytes.first()?);
    if first & 1 == 0 {
        return Some(16);
    }
    let n = (first >> 1) & 7;
    if n != 0 {
        return Some(17 + n);
    }
    match (first >> 4) & 7 {
        0 => Some(17),
        1 => None,
        m => Some(8 + m),
    }
}

/// The coded field for `content` with quality [`QUALITY`] and the window of
/// [`window_bits`], or `None` when the `brotli` feature is off or the encoder
/// fails.
pub fn encode(content: &[u8]) -> Option<Vec<u8>> {
    encode_with(content, QUALITY, window_bits(content.len()))
}

/// The coded field for `content` with brotli quality `quality` (0–11) and
/// window size `WBITS` = `window_bits` (10–24). Returns `None` for values
/// outside those ranges, when the `brotli` feature is off, when the encoder
/// fails, or when the stream declares a window above [`max_window_bits`] of the
/// content length (6.10 rule 1), which happens for a `window_bits` above that
/// bound. Every stream it returns is a valid codec-5 coded field; only the size
/// differs.
pub fn encode_with(content: &[u8], quality: u32, window_bits: u32) -> Option<Vec<u8>> {
    #[cfg(feature = "brotli")]
    {
        let stream = imp::encode_raw(content, quality, window_bits)?;
        let len = u32::try_from(content.len()).ok()?;
        (declared_window_bits(&stream)? <= max_window_bits(len)).then_some(stream)
    }
    #[cfg(not(feature = "brotli"))]
    {
        let _ = (content, quality, window_bits);
        None
    }
}

/// The expansion bound of 6.4 rule 2 for codec 5: `L` ≤ 256 · (`Lc` + 4).
pub const EXPANSION_MAX: u32 = 256;

/// Decodes a brotli coded field with dictionary 0 (the RFC 7932 built-in
/// dictionary only) into exactly `decoded_len` bytes.
///
/// # Errors
///
/// - [`CodecError::UnsupportedCodec`]: the `brotli` feature is off.
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   `decoded_len` > [`EXPANSION_MAX`] · (`bytes.len()` + 4), an empty field,
///   or a declared window above [`max_window_bits`] of `decoded_len`, all
///   checked before anything is allocated;
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
    /// The largest byte buffer is the ring buffer. When the first meta-block
    /// is the last, brotli-decompressor 6.0.1 sizes it to that meta-block's
    /// output; otherwise it takes the whole declared window. Measured, it is at
    /// most 2^`WBITS` + 566 bytes. The other byte buffers (context maps) are at
    /// most 16 KiB. The cap is 2^`WBITS` + 64 KiB, and the decoder checks
    /// `WBITS` ≤ [`super::max_window_bits`] of `L` first, so memory and work
    /// stay proportional to `L` (6.4 rules 3 and 6, 6.10 rules 1 and 2).
    struct CappedBytes {
        max_len: usize,
    }

    impl CappedBytes {
        fn for_window(window_bits: u32) -> Self {
            let ring = 1usize.checked_shl(window_bits).unwrap_or(usize::MAX);
            Self { max_len: ring.saturating_add(64 * 1024) }
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

    /// The brotli stream for `content`, whatever window it declares.
    pub(super) fn encode_raw(content: &[u8], quality: u32, window_bits: u32) -> Option<Vec<u8>> {
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
        if !crate::within_expansion(decoded_len, bytes.len(), super::EXPANSION_MAX) {
            return Err(CodecError::Malformed);
        }
        let window = super::declared_window_bits(bytes).ok_or(CodecError::Malformed)?;
        if window > super::max_window_bits(decoded_len) {
            return Err(CodecError::Malformed);
        }
        let total = to_usize(decoded_len)?;
        let mut out = vec![0u8; total];
        let mut state: BrotliState<CappedBytes, Heap, Heap> =
            BrotliState::new(CappedBytes::for_window(window), Heap, Heap);
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

        use super::{CappedBytes, Heap, encode_raw};
        use crate::CodecError;

        #[test]
        fn cap_follows_window() {
            assert_eq!(CappedBytes::for_window(10).max_len, 1024 + 64 * 1024);
            assert_eq!(CappedBytes::for_window(16).max_len, 128 * 1024);
            assert_eq!(CappedBytes::for_window(24).max_len, (16 << 20) + 64 * 1024);
        }

        /// Runs the decoder once with the byte cap of window `cap_window` and an
        /// output buffer of `out_len` bytes; returns the result and the bytes
        /// written.
        fn run(stream: &[u8], out_len: usize, cap_window: u32) -> (BrotliResult, usize) {
            let mut out = vec![0u8; out_len];
            let mut state: BrotliState<CappedBytes, Heap, Heap> =
                BrotliState::new(CappedBytes::for_window(cap_window), Heap, Heap);
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
            let stream = encode_raw(&zeros, 11, 23).unwrap();
            assert!(stream.len() < 32);
            assert_eq!(super::super::declared_window_bits(&stream), Some(23));
            // With the cap of its declared window the decoder fills the
            // window and then reports that the 10-byte output is full.
            let (result, written) = run(&stream, 10, 23);
            assert!(matches!(result, BrotliResult::NeedsMoreOutput));
            assert_eq!(written, 10);
            // With the cap of the largest window allowed for L = 10 it fails
            // before writing anything.
            let (result, written) = run(&stream, 10, super::super::max_window_bits(10));
            assert!(matches!(result, BrotliResult::ResultFailure));
            assert_eq!(written, 0);
            // decode() rejects it from the window field alone.
            assert_eq!(super::decode(10, &stream, u32::MAX), Err(CodecError::Malformed));
            // The true length allows the window, and the stream decodes.
            let (result, written) = run(&stream, 4 << 20, 23);
            assert!(matches!(result, BrotliResult::ResultSuccess));
            assert_eq!(written, 4 << 20);
        }

        /// brotli 9.0.0 at quality 0 and 1 declares a window of at least 18
        /// whatever is asked, and brotli-decompressor then allocates that whole
        /// window when the first meta-block is not the last. The byte cap used
        /// to follow `L` and refused this valid stream. The 103 bytes are the
        /// smallest content proptest found for it (2026-09-29).
        #[test]
        fn window_is_bounded_by_the_decoded_length() {
            let content: &[u8] = b"m.mail.play.google.com/store/apps/details?id=https://www.geo:www.mailto:mailto:https://www.https://www.";
            let len = u32::try_from(content.len()).unwrap();
            assert_eq!(super::super::max_window_bits(len), 18);
            let fast = encode_raw(content, 1, 10).unwrap();
            assert_eq!(super::super::declared_window_bits(&fast), Some(18));
            assert_eq!(super::decode(len, &fast, u32::MAX).as_deref(), Ok(content));
            // At quality 11 the stream declares the window it is asked for.
            for window in 10..=18 {
                let stream = encode_raw(content, 11, window).unwrap();
                assert_eq!(super::super::declared_window_bits(&stream), Some(window));
                assert_eq!(super::decode(len, &stream, u32::MAX).as_deref(), Ok(content));
            }
            let wide = encode_raw(content, 11, 19).unwrap();
            assert_eq!(super::decode(len, &wide, u32::MAX), Err(CodecError::Malformed));
            assert_eq!(super::super::encode_with(content, 11, 19), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{declared_window_bits, max_window_bits, window_bits};

    #[test]
    fn window_field() {
        assert_eq!(declared_window_bits(&[]), None);
        assert_eq!(declared_window_bits(&[0x00]), Some(16));
        assert_eq!(declared_window_bits(&[0x03]), Some(18));
        assert_eq!(declared_window_bits(&[0x0F]), Some(24));
        assert_eq!(declared_window_bits(&[0x01]), Some(17));
        assert_eq!(declared_window_bits(&[0x21]), Some(10));
        assert_eq!(declared_window_bits(&[0x71]), Some(15));
        // The 7-bit form with value 1 marks a large-window stream.
        assert_eq!(declared_window_bits(&[0x11]), None);
    }

    #[test]
    fn largest_window_follows_the_decoded_length() {
        assert_eq!(max_window_bits(0), 18);
        assert_eq!(max_window_bits(131_056), 18);
        assert_eq!(max_window_bits(131_057), 19);
        assert_eq!(max_window_bits(262_128), 19);
        assert_eq!(max_window_bits(262_129), 20);
        assert_eq!(max_window_bits(1 << 20), 22);
        assert_eq!(max_window_bits(8_388_592), 24);
        assert_eq!(max_window_bits(16 << 20), 24);
        assert_eq!(max_window_bits(u32::MAX), 24);
        for len in [0, 1, 1000, 65_520, 65_521, 131_056, 131_057, 262_129, 1 << 20, 16 << 20] {
            let asked = window_bits(len);
            assert!(asked <= max_window_bits(u32::try_from(len).unwrap()), "{len}");
        }
    }

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
