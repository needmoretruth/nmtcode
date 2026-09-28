//! Codec 4 — Hangul syllable packing (6.9).
//!
//! H mode codes one syllable, a syllable and a space, or one of several
//! punctuation and jamo ranges in 14 bits; A mode codes ASCII in 7 bits.

use alloc::vec::Vec;

use crate::bits::{BitReader, BitWriter};
use crate::{CodecError, check_decoded_len, to_usize};

/// Codec ID.
pub const ID: u32 = 4;

/// `HANGUL_SPACE_FINALS` of 6.9.2: the `TIdx` values `F[0]` … `F[11]`.
pub const HANGUL_SPACE_FINALS: [u32; 12] = [0, 1, 4, 7, 8, 16, 17, 19, 20, 21, 23, 26];

const SYLLABLE_BASE: u32 = 0xAC00;
const SYLLABLE_COUNT: u32 = 11_172;
const LV_COUNT: u32 = 399;
const SYLLABLE_SPACE: u32 = 11_172;
const PUNCTUATION_SPACE: u32 = 16_358;
const PUNCTUATION: [u32; 4] = ['.' as u32, ',' as u32, '?' as u32, '!' as u32];
const SWITCH_A: u32 = 16_362;
const ESC16: u32 = 16_363;
const ESC21: u32 = 16_364;
const SWITCH_H: u32 = 0x7F;
const SPACE: u32 = 0x20;

/// The direct-code ranges of 6.9.2 other than syllables: (first code, first
/// code point, last code point).
const DIRECT_RANGES: [(u32, u32, u32); 5] = [
    (15_960, 0x0000, 0x007F),
    (16_088, 0x00A0, 0x00FF),
    (16_184, 0x2010, 0x203F),
    (16_232, 0x3000, 0x301F),
    (16_264, 0x3131, 0x318E),
];

/// True when `content` is well-formed UTF-8.
pub fn is_applicable(content: &[u8]) -> bool {
    core::str::from_utf8(content).is_ok()
}

fn is_syllable(cp: u32) -> bool {
    (SYLLABLE_BASE..SYLLABLE_BASE + SYLLABLE_COUNT).contains(&cp)
}

fn direct_code(cp: u32) -> Option<u32> {
    DIRECT_RANGES
        .iter()
        .find(|&&(_, first, last)| (first..=last).contains(&cp))
        .map(|&(code, first, _)| code + cp - first)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    H,
    A,
}

impl Mode {
    const fn index(self) -> usize {
        match self {
            Self::H => 0,
            Self::A => 1,
        }
    }

    const fn unit_bits(self) -> u32 {
        match self {
            Self::H => 14,
            Self::A => 7,
        }
    }
}

/// One unit the reference encoder may emit at a position.
#[derive(Debug, Clone, Copy)]
struct Step {
    /// The unit code, in the width of the current mode.
    code: u32,
    /// The escape's code point and its width.
    extra: Option<(u32, u32)>,
    /// Code points consumed.
    advance: usize,
    /// Mode after the unit.
    mode: Mode,
}

impl Step {
    fn bits(&self, mode: Mode) -> u32 {
        mode.unit_bits() + self.extra.map_or(0, |(_, width)| width)
    }
}

/// The options at `i` in `mode`, in the tie-break order of 6.9.4.
fn options(cps: &[u32], i: usize, mode: Mode) -> [Option<Step>; 6] {
    let mut out = [None; 6];
    let Some(&cp) = cps.get(i) else {
        return out;
    };
    let followed_by_space = cps.get(i + 1) == Some(&SPACE);
    let step = |code, advance, mode| Step { code, extra: None, advance, mode };
    match mode {
        Mode::H => {
            if is_syllable(cp) {
                let offset = cp - SYLLABLE_BASE;
                let (lv, t) = (offset / 28, offset % 28);
                if followed_by_space
                    && let Some(j) = HANGUL_SPACE_FINALS.iter().position(|&f| f == t)
                {
                    let j = u32::try_from(j).unwrap_or(0);
                    out[0] = Some(step(SYLLABLE_SPACE + j * LV_COUNT + lv, 2, Mode::H));
                }
                out[1] = Some(step(offset, 1, Mode::H));
            }
            if followed_by_space && let Some(p) = PUNCTUATION.iter().position(|&p| p == cp) {
                let p = u32::try_from(p).unwrap_or(0);
                out[2] = Some(step(PUNCTUATION_SPACE + p, 2, Mode::H));
            }
            if let Some(code) = direct_code(cp) {
                out[3] = Some(step(code, 1, Mode::H));
            } else if !is_syllable(cp) {
                // An escape only for a code point without a direct code.
                let (code, width) = if cp <= 0xFFFF { (ESC16, 16) } else { (ESC21, 21) };
                out[4] = Some(Step { code, extra: Some((cp, width)), advance: 1, mode: Mode::H });
            }
            out[5] = Some(step(SWITCH_A, 0, Mode::A));
        }
        Mode::A => {
            if cp <= 0x7E {
                out[0] = Some(step(cp, 1, Mode::A));
            }
            out[1] = Some(step(SWITCH_H, 0, Mode::H));
        }
    }
    out
}

/// The reference encoder of 6.9.4: minimal bit length by dynamic programming
/// over (position, mode), ties broken in the order of 6.9.4. Returns `None`
/// when `content` is not well-formed UTF-8.
pub fn encode(content: &[u8]) -> Option<Vec<u8>> {
    let text = core::str::from_utf8(content).ok()?;
    let cps: Vec<u32> = text.chars().map(u32::from).collect();
    if cps.is_empty() {
        return Some(Vec::new());
    }
    // cost[i][mode]: fewest bits to code cps[i..] starting in `mode`. At most
    // 35 bits per code point, so u32 holds it for any content of format
    // version 0; the additions saturate regardless.
    let mut cost = alloc::vec![[0u32; 2]; cps.len() + 1];
    for i in (0..cps.len()).rev() {
        let mut stay = [u32::MAX; 2];
        for mode in [Mode::H, Mode::A] {
            for step in options(&cps, i, mode).into_iter().flatten() {
                if step.advance > 0 {
                    let after = cost.get(i + step.advance)?[step.mode.index()];
                    stay[mode.index()] =
                        stay[mode.index()].min(step.bits(mode).saturating_add(after));
                }
            }
        }
        let [stay_h, stay_a] = stay;
        *cost.get_mut(i)? =
            [stay_h.min(stay_a.saturating_add(14)), stay_a.min(stay_h.saturating_add(7))];
    }
    let [start_h, start_a] = *cost.first()?;
    let mut mode = if start_h <= start_a { Mode::H } else { Mode::A };
    let mut writer = BitWriter::new();
    writer.put(u32::from(mode == Mode::A), 1);
    let mut i = 0;
    while i < cps.len() {
        let target = cost.get(i)?[mode.index()];
        let step = options(&cps, i, mode).into_iter().flatten().find(|step| {
            cost.get(i + step.advance).is_some_and(|after| {
                step.bits(mode).saturating_add(after[step.mode.index()]) == target
            })
        })?;
        writer.put(step.code, mode.unit_bits());
        if let Some((value, width)) = step.extra {
            writer.put(value, width);
        }
        i += step.advance;
        mode = step.mode;
    }
    Some(writer.finish())
}

/// Output that refuses to grow beyond `L` bytes (6.9.1).
struct Output {
    bytes: Vec<u8>,
    total: usize,
}

impl Output {
    fn push_char(&mut self, cp: u32) -> Result<(), CodecError> {
        let c = char::from_u32(cp).ok_or(CodecError::Malformed)?;
        let mut buffer = [0u8; 4];
        self.push(c.encode_utf8(&mut buffer).as_bytes())
    }

    fn push(&mut self, piece: &[u8]) -> Result<(), CodecError> {
        if piece.len() > self.total - self.bytes.len() {
            return Err(CodecError::Malformed);
        }
        self.bytes.extend_from_slice(piece);
        Ok(())
    }
}

/// Decodes a Hangul-packing coded field into `decoded_len` bytes of UTF-8.
///
/// # Errors
///
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   `L` = 0 with `Lc` ≠ 0; 7·`L` > 16·`Lc` (checked before allocating); a
///   reserved unit; an escaped surrogate or a value above U+10FFFF; a unit that
///   would go beyond `L` bytes; a read past the end; 8 or more bits left after
///   the last unit; a non-zero padding bit.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(decoded_len: u32, bytes: &[u8], limit: u32) -> Result<Vec<u8>, CodecError> {
    check_decoded_len(decoded_len, limit)?;
    if decoded_len == 0 {
        return if bytes.is_empty() { Ok(Vec::new()) } else { Err(CodecError::Malformed) };
    }
    // A 14-bit unit outputs at most 4 bytes: 7·L ≤ 16·Lc.
    let coded_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if 7 * u64::from(decoded_len) > coded_len.saturating_mul(16) {
        return Err(CodecError::Malformed);
    }
    let total = to_usize(decoded_len)?;
    let mut out = Output { bytes: Vec::with_capacity(total), total };
    let mut reader = BitReader::new(bytes);
    let mut mode = if reader.read(1)? == 0 { Mode::H } else { Mode::A };
    while out.bytes.len() < total {
        match mode {
            Mode::A => match reader.read(7)? {
                SWITCH_H => mode = Mode::H,
                ascii => out.push_char(ascii)?,
            },
            Mode::H => match reader.read(14)? {
                u @ 0..11_172 => out.push_char(SYLLABLE_BASE + u)?,
                u @ 11_172..15_960 => {
                    let m = u - SYLLABLE_SPACE;
                    let j = to_usize(m / LV_COUNT)?;
                    let final_index = *HANGUL_SPACE_FINALS.get(j).ok_or(CodecError::Malformed)?;
                    out.push_char(SYLLABLE_BASE + 28 * (m % LV_COUNT) + final_index)?;
                    out.push(b" ")?;
                }
                u @ 15_960..16_358 => {
                    let (code, first, _) = DIRECT_RANGES
                        .iter()
                        .rev()
                        .find(|&&(code, _, _)| code <= u)
                        .copied()
                        .ok_or(CodecError::Malformed)?;
                    out.push_char(first + u - code)?;
                }
                u @ 16_358..16_362 => {
                    let p = to_usize(u - PUNCTUATION_SPACE)?;
                    out.push_char(*PUNCTUATION.get(p).ok_or(CodecError::Malformed)?)?;
                    out.push(b" ")?;
                }
                SWITCH_A => mode = Mode::A,
                ESC16 => {
                    // `char::from_u32` rejects 0xD800–0xDFFF.
                    let cp = reader.read(16)?;
                    out.push_char(cp)?;
                }
                ESC21 => {
                    // `char::from_u32` rejects surrogates and values above U+10FFFF.
                    let cp = reader.read(21)?;
                    out.push_char(cp)?;
                }
                _ => return Err(CodecError::Malformed),
            },
        }
    }
    if !reader.only_zero_padding_left() {
        return Err(CodecError::Malformed);
    }
    Ok(out.bytes)
}
