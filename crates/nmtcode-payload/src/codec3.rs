//! Codec 3 — token table and short-text model (6.8).
//!
//! The content is parsed into symbols (literal bytes and tokens of
//! [`TOKEN_TABLE_V0`]) and the symbols are coded with a range coder under a
//! static model: [`Model::model0`] for dictionary 0, or a registered
//! short-text model dictionary (6.8.4) loaded with [`Model::from_dictionary`].

use alloc::vec;
use alloc::vec::Vec;

use crate::bits::low_byte;
use crate::{CodecError, check_decoded_len, leb128, to_usize};

/// Codec ID.
pub const ID: u32 = 3;

/// `TOKEN_COUNT` of 6.8.1.
pub const TOKEN_COUNT: usize = 240;

/// Alphabet size `A` = 256 + [`TOKEN_COUNT`] (6.8.1).
pub const ALPHABET_SIZE: usize = 256 + TOKEN_COUNT;

/// Frequency total of every vector: 2^16 (6.8.5).
const TOTAL: u32 = 1 << 16;

/// Renormalisation threshold 2^24 (6.8.5).
const TOP: u32 = 1 << 24;

/// `TOKEN_TABLE_V0` of 6.8.2, in ID order. Every token is ASCII.
pub const TOKEN_TABLE_V0: [&[u8]; TOKEN_COUNT] = [
    b"https://www.",                           // 0
    b"http://www.",                            // 1
    b"https://",                               // 2
    b"http://",                                // 3
    b"HTTPS://",                               // 4
    b"HTTP://",                                // 5
    b"mailto:",                                // 6
    b"tel:",                                   // 7
    b"sms:",                                   // 8
    b"smsto:",                                 // 9
    b"SMSTO:",                                 // 10
    b"geo:",                                   // 11
    b"ftp://",                                 // 12
    b"wss://",                                 // 13
    b"ws://",                                  // 14
    b"magnet:?xt=urn:btih:",                   // 15
    b"market://details?id=",                   // 16
    b"bitcoin:",                               // 17
    b"ethereum:",                              // 18
    b"lightning:",                             // 19
    b"otpauth://totp/",                        // 20
    b"otpauth://hotp/",                        // 21
    b"WIFI:S:",                                // 22
    b"WIFI:T:WPA;S:",                          // 23
    b"MECARD:N:",                              // 24
    b"BEGIN:VCARD",                            // 25
    b"END:VCARD",                              // 26
    b"VERSION:3.0",                            // 27
    b"VERSION:4.0",                            // 28
    b"LOCATION:",                              // 29
    b"www.",                                   // 30
    b"m.",                                     // 31
    b"mail.",                                  // 32
    b"blog.",                                  // 33
    b"docs.",                                  // 34
    b"api.",                                   // 35
    b"app.",                                   // 36
    b"shop.",                                  // 37
    b"cdn.",                                   // 38
    b"play.google.com/store/apps/details?id=", // 39
    b"apps.apple.com/",                        // 40
    b"youtube.com",                            // 41
    b"youtu.be",                               // 42
    b"google.com",                             // 43
    b"github.com",                             // 44
    b"naver.com",                              // 45
    b"naver.me",                               // 46
    b"kakao.com",                              // 47
    b"instagram.com",                          // 48
    b"facebook.com",                           // 49
    b"x.com",                                  // 50
    b"twitter.com",                            // 51
    b"linkedin.com",                           // 52
    b"tiktok.com",                             // 53
    b"amazon.com",                             // 54
    b"wikipedia.org",                          // 55
    b"bit.ly",                                 // 56
    b"forms.gle",                              // 57
    b"maps.app.goo.gl",                        // 58
    b"open.kakao.com",                         // 59
    b"pf.kakao.com",                           // 60
    b"smartstore.naver.com",                   // 61
    b"blog.naver.com",                         // 62
    b"cafe.naver.com",                         // 63
    b"t.me",                                   // 64
    b"wa.me",                                  // 65
    b".com/",                                  // 66
    b".net/",                                  // 67
    b".org/",                                  // 68
    b".kr/",                                   // 69
    b".com",                                   // 70
    b".net",                                   // 71
    b".org",                                   // 72
    b".edu",                                   // 73
    b".gov",                                   // 74
    b".io",                                    // 75
    b".co",                                    // 76
    b".kr",                                    // 77
    b".jp",                                    // 78
    b".cn",                                    // 79
    b".de",                                    // 80
    b".uk",                                    // 81
    b".fr",                                    // 82
    b".ru",                                    // 83
    b".br",                                    // 84
    b".in",                                    // 85
    b".it",                                    // 86
    b".es",                                    // 87
    b".nl",                                    // 88
    b".au",                                    // 89
    b".ca",                                    // 90
    b".ch",                                    // 91
    b".se",                                    // 92
    b".no",                                    // 93
    b".pl",                                    // 94
    b".eu",                                    // 95
    b".us",                                    // 96
    b".me",                                    // 97
    b".tv",                                    // 98
    b".info",                                  // 99
    b".biz",                                   // 100
    b".app",                                   // 101
    b".dev",                                   // 102
    b".ai",                                    // 103
    b".xyz",                                   // 104
    b".be",                                    // 105
    b".at",                                    // 106
    b".dk",                                    // 107
    b".fi",                                    // 108
    b".tw",                                    // 109
    b".hk",                                    // 110
    b".sg",                                    // 111
    b".vn",                                    // 112
    b".id",                                    // 113
    b".th",                                    // 114
    b".mx",                                    // 115
    b".ar",                                    // 116
    b".tr",                                    // 117
    b".ua",                                    // 118
    b".site",                                  // 119
    b".online",                                // 120
    b".shop",                                  // 121
    b".store",                                 // 122
    b".to",                                    // 123
    b".gg",                                    // 124
    b".co.kr",                                 // 125
    b".or.kr",                                 // 126
    b".go.kr",                                 // 127
    b".ac.kr",                                 // 128
    b".ne.kr",                                 // 129
    b".co.jp",                                 // 130
    b".ne.jp",                                 // 131
    b".or.jp",                                 // 132
    b".co.uk",                                 // 133
    b".com.au",                                // 134
    b".com.br",                                // 135
    b".com.cn",                                // 136
    b".com.tw",                                // 137
    b".co.za",                                 // 138
    b"/index.html",                            // 139
    b"index.",                                 // 140
    b".html",                                  // 141
    b".htm",                                   // 142
    b".php",                                   // 143
    b".aspx",                                  // 144
    b".asp",                                   // 145
    b".jsp",                                   // 146
    b".do",                                    // 147
    b".pdf",                                   // 148
    b".jpg",                                   // 149
    b".png",                                   // 150
    b".json",                                  // 151
    b"/?",                                     // 152
    b"?id=",                                   // 153
    b"&id=",                                   // 154
    b"?v=",                                    // 155
    b"/watch?v=",                              // 156
    b"?q=",                                    // 157
    b"/search?q=",                             // 158
    b"?page=",                                 // 159
    b"&page=",                                 // 160
    b"?ref=",                                  // 161
    b"?si=",                                   // 162
    b"?utm_source=",                           // 163
    b"&utm_source=",                           // 164
    b"&utm_medium=",                           // 165
    b"&utm_campaign=",                         // 166
    b"&utm_content=",                          // 167
    b"?lang=",                                 // 168
    b"/ko/",                                   // 169
    b"/en/",                                   // 170
    b"/ko-kr/",                                // 171
    b"/en-us/",                                // 172
    b"/wiki/",                                 // 173
    b"/p/",                                    // 174
    b"/s/",                                    // 175
    b"/api/",                                  // 176
    b"/login",                                 // 177
    b"/download",                              // 178
    b"/products/",                             // 179
    b"/product/",                              // 180
    b"/item/",                                 // 181
    b"/event/",                                // 182
    b"/news/",                                 // 183
    b"/blog/",                                 // 184
    b"/post/",                                 // 185
    b"/view/",                                 // 186
    b"/detail",                                // 187
    b"/page/",                                 // 188
    b"/home",                                  // 189
    b"/main",                                  // 190
    b"/shop/",                                 // 191
    b"/user/",                                 // 192
    b"/share/",                                // 193
    b"/invite/",                               // 194
    b"/join",                                  // 195
    b"/docs/",                                 // 196
    b"/tree/",                                 // 197
    b"/blob/",                                 // 198
    b"/releases",                              // 199
    b"/issues/",                               // 200
    b"/pull/",                                 // 201
    b"/status/",                               // 202
    b"/channel/",                              // 203
    b"/playlist?list=",                        // 204
    b"/reel/",                                 // 205
    b"/shorts/",                               // 206
    b"/article/",                              // 207
    b"/search",                                // 208
    b";T:WPA;P:",                              // 209
    b";P:",                                    // 210
    b";H:true;",                               // 211
    b";;",                                     // 212
    b"?secret=",                               // 213
    b"&issuer=",                               // 214
    b"&algorithm=SHA1",                        // 215
    b"&digits=6",                              // 216
    b"&period=30",                             // 217
    b"?amount=",                               // 218
    b"&label=",                                // 219
    b"&message=",                              // 220
    b"bc1q",                                   // 221
    b"0x",                                     // 222
    b" the ",                                  // 223
    b"The ",                                   // 224
    b" and ",                                  // 225
    b" of ",                                   // 226
    b" to ",                                   // 227
    b" in ",                                   // 228
    b" is ",                                   // 229
    b" for ",                                  // 230
    b" you",                                   // 231
    b" on ",                                   // 232
    b" with ",                                 // 233
    b" that ",                                 // 234
    b"ing ",                                   // 235
    b"tion",                                   // 236
    b". ",                                     // 237
    b", ",                                     // 238
    b"\r\n",                                   // 239
];

/// The longest token, in bytes.
pub const MAX_TOKEN_LEN: usize = {
    let mut max = 0;
    let mut i = 0;
    while i < TOKEN_COUNT {
        if TOKEN_TABLE_V0[i].len() > max {
            max = TOKEN_TABLE_V0[i].len();
        }
        i += 1;
    }
    max
};

/// `MODEL0_WEIGHTS` of 6.8.3: the frequency of symbol `s` under model 0.
pub const fn model0_frequency(symbol: usize) -> u32 {
    match symbol {
        0x61..=0x7A => 1300,
        0x30..=0x39 => 700,
        0x41..=0x5A => 250,
        0x2F => 864,
        0x2E | 0x2D | 0x5F | 0x3D | 0x3F | 0x26 | 0x3A | 0x25 | 0x23 | 0x2B | 0x7E | 0x20 => 560,
        0x21..=0x7E | 0x09 | 0x0A | 0x0D => 60,
        0x00..=0xFF => 4,
        _ => 36,
    }
}

/// `cum[s]` of model 0 for `s` = 0 … 496; `cum[496]` = 2^16.
const MODEL0_CUM: [u32; ALPHABET_SIZE + 1] = {
    let mut cum = [0u32; ALPHABET_SIZE + 1];
    let mut s = 0;
    while s < ALPHABET_SIZE {
        cum[s + 1] = cum[s] + model0_frequency(s);
        s += 1;
    }
    cum
};

const _: () = assert!(MODEL0_CUM[ALPHABET_SIZE] == TOTAL);

/// The output bytes of symbol `symbol` (6.8.1), or `None` for a symbol ≥ `A`.
pub fn symbol_bytes(symbol: u16) -> Option<SymbolBytes> {
    match symbol {
        0..=255 => Some(SymbolBytes::Literal([low_byte(u64::from(symbol))])),
        _ => TOKEN_TABLE_V0.get(usize::from(symbol) - 256).map(|token| SymbolBytes::Token(token)),
    }
}

/// The bytes one symbol stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBytes {
    /// A literal byte.
    Literal([u8; 1]),
    /// A token of [`TOKEN_TABLE_V0`].
    Token(&'static [u8]),
}

impl SymbolBytes {
    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Literal(byte) => byte,
            Self::Token(token) => token,
        }
    }
}

/// A frequency vector with explicit entries; every other symbol has frequency 1
/// (6.8.4).
#[derive(Debug, Clone, PartialEq, Eq)]
struct SparseVector {
    /// Symbols with an explicit entry, strictly increasing.
    symbols: Vec<u16>,
    /// Their frequencies.
    frequencies: Vec<u32>,
    /// `cum` at each explicit symbol.
    cums: Vec<u32>,
}

impl SparseVector {
    fn range_of(&self, symbol: usize) -> Option<(u32, u32)> {
        if symbol >= ALPHABET_SIZE {
            return None;
        }
        let index = self.symbols.partition_point(|&explicit| usize::from(explicit) < symbol);
        if self.symbols.get(index).map(|&s| usize::from(s)) == Some(symbol) {
            return Some((*self.cums.get(index)?, *self.frequencies.get(index)?));
        }
        let symbol = u32::try_from(symbol).ok()?;
        let cum = match index.checked_sub(1) {
            None => symbol,
            Some(previous) => {
                let previous_symbol = u32::from(*self.symbols.get(previous)?);
                self.cums.get(previous)? + self.frequencies.get(previous)? + symbol
                    - previous_symbol
                    - 1
            }
        };
        Some((cum, 1))
    }

    fn symbol_at(&self, value: u32) -> Option<usize> {
        let index = self.cums.partition_point(|&cum| cum <= value);
        let (base_symbol, base_cum) = match index.checked_sub(1) {
            None => (0, 0),
            Some(previous) => {
                let cum = *self.cums.get(previous)?;
                let end = cum + self.frequencies.get(previous)?;
                let symbol = usize::from(*self.symbols.get(previous)?);
                if value < end {
                    return Some(symbol);
                }
                (symbol + 1, end)
            }
        };
        let symbol = base_symbol + to_usize(value - base_cum).ok()?;
        let next_explicit = self.symbols.get(index).map_or(ALPHABET_SIZE, |&s| usize::from(s));
        (symbol < next_explicit).then_some(symbol)
    }
}

/// The frequency vector the model selects for the next symbol.
#[derive(Clone, Copy)]
enum Vector<'a> {
    Model0,
    Sparse(&'a SparseVector),
}

impl Vector<'_> {
    /// (`cum[s]`, `f[s]`).
    fn range_of(self, symbol: usize) -> Option<(u32, u32)> {
        match self {
            Self::Model0 => {
                let cum = *MODEL0_CUM.get(symbol)?;
                let end = *MODEL0_CUM.get(symbol + 1)?;
                Some((cum, end - cum))
            }
            Self::Sparse(vector) => vector.range_of(symbol),
        }
    }

    /// The unique `s` with `cum[s]` ≤ `value` < `cum[s]` + `f[s]`, for `value` < 2^16.
    fn symbol_at(self, value: u32) -> Option<usize> {
        match self {
            Self::Model0 => {
                let symbol = MODEL0_CUM.get(1..)?.partition_point(|&end| end <= value);
                (symbol < ALPHABET_SIZE).then_some(symbol)
            }
            Self::Sparse(vector) => vector.symbol_at(value),
        }
    }
}

/// An order-`k` hashed context model (6.8.4).
#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextModel {
    order: usize,
    hash_bits: u32,
    slotmap: Vec<u16>,
    vectors: Vec<SparseVector>,
}

impl ContextModel {
    fn vector(&self, history: &[u8]) -> Option<&SparseVector> {
        // Positions before the start of the content count as 0x00.
        let start = history.len().saturating_sub(self.order);
        let padding = self.order - (history.len() - start);
        let context = core::iter::repeat_n(&0u8, padding).chain(history.get(start..)?);
        let hash = context
            .fold(0x811C_9DC5_u32, |h, &byte| (h ^ u32::from(byte)).wrapping_mul(0x0100_0193));
        let slot = if self.hash_bits == 0 { 0 } else { hash >> (32 - self.hash_bits) };
        let index = *self.slotmap.get(to_usize(slot).ok()?)?;
        self.vectors.get(usize::from(index))
    }
}

/// The static model of codec 3: model 0 (6.8.3) or a short-text model
/// dictionary (6.8.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model(ModelKind);

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelKind {
    Model0,
    Context(ContextModel),
}

impl Default for Model {
    fn default() -> Self {
        Self::model0()
    }
}

/// Reads fixed-width fields and LEB128 values from a dictionary.
struct Cursor<'a> {
    bytes: &'a [u8],
}

impl Cursor<'_> {
    fn u8(&mut self) -> Result<u8, CodecError> {
        let (&first, rest) = self.bytes.split_first().ok_or(CodecError::Malformed)?;
        self.bytes = rest;
        Ok(first)
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_be_bytes([self.u8()?, self.u8()?]))
    }

    fn leb128(&mut self) -> Result<u32, CodecError> {
        let (value, used) = leb128::read(self.bytes)?;
        self.bytes = self.bytes.get(used..).ok_or(CodecError::Malformed)?;
        Ok(value)
    }
}

impl Model {
    /// Model 0 (6.8.3), used when `d` = 0.
    pub const fn model0() -> Self {
        Self(ModelKind::Model0)
    }

    /// True for model 0.
    pub const fn is_model0(&self) -> bool {
        matches!(self.0, ModelKind::Model0)
    }

    /// Loads a short-text model dictionary in the byte format of 6.8.4.
    ///
    /// The whole dictionary is validated here, before any symbol is decoded.
    /// Work and memory are proportional to the dictionary size.
    ///
    /// # Errors
    ///
    /// [`CodecError::Malformed`] for any violation of the byte format: a field
    /// out of range, a malformed LEB128 value, a symbol ≥ `A`, a vector whose
    /// frequencies do not sum to 2^16, a missing byte or a trailing byte.
    pub fn from_dictionary(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = Cursor { bytes };
        if cursor.u8()? != 0 {
            return Err(CodecError::Malformed);
        }
        let order = cursor.u8()?;
        let hash_bits = cursor.u8()?;
        if order > 4 || hash_bits > 12 || (hash_bits == 0) != (order == 0) {
            return Err(CodecError::Malformed);
        }
        let vector_count = cursor.u16()?;
        if !(1..=4096).contains(&vector_count) {
            return Err(CodecError::Malformed);
        }
        if usize::from(cursor.u16()?) != ALPHABET_SIZE {
            return Err(CodecError::Malformed);
        }
        let mut slotmap = Vec::with_capacity(1 << hash_bits);
        for _ in 0..(1u32 << hash_bits) {
            let index = cursor.u16()?;
            if index >= vector_count {
                return Err(CodecError::Malformed);
            }
            slotmap.push(index);
        }
        let mut vectors = Vec::with_capacity(usize::from(vector_count));
        for _ in 0..vector_count {
            vectors.push(Self::read_vector(&mut cursor)?);
        }
        if !cursor.bytes.is_empty() {
            return Err(CodecError::Malformed);
        }
        Ok(Self(ModelKind::Context(ContextModel {
            order: usize::from(order),
            hash_bits: u32::from(hash_bits),
            slotmap,
            vectors,
        })))
    }

    fn read_vector(cursor: &mut Cursor<'_>) -> Result<SparseVector, CodecError> {
        let count = to_usize(cursor.leb128()?)?;
        if count > ALPHABET_SIZE {
            return Err(CodecError::Malformed);
        }
        let mut entries: Vec<(u16, u32)> = Vec::with_capacity(count);
        let mut previous: Option<u64> = None;
        for _ in 0..count {
            let gap = u64::from(cursor.leb128()?);
            let excess = u64::from(cursor.leb128()?);
            let symbol = match previous {
                None => gap,
                Some(previous) => previous + 1 + gap,
            };
            let frequency = excess + 2;
            if symbol >= ALPHABET_SIZE as u64 || frequency > u64::from(TOTAL) {
                return Err(CodecError::Malformed);
            }
            let symbol16 = u16::try_from(symbol).map_err(|_| CodecError::Malformed)?;
            let frequency32 = u32::try_from(frequency).map_err(|_| CodecError::Malformed)?;
            entries.push((symbol16, frequency32));
            previous = Some(symbol);
        }
        // Symbols without an explicit entry have frequency 1.
        let mut sum = (ALPHABET_SIZE - count) as u64;
        for &(_, frequency) in &entries {
            sum += u64::from(frequency);
        }
        if sum != u64::from(TOTAL) {
            return Err(CodecError::Malformed);
        }
        let mut vector = SparseVector {
            symbols: Vec::with_capacity(count),
            frequencies: Vec::with_capacity(count),
            cums: Vec::with_capacity(count),
        };
        let mut cum = 0u32;
        let mut next_symbol = 0u32;
        for (symbol, frequency) in entries {
            cum += u32::from(symbol) - next_symbol;
            vector.symbols.push(symbol);
            vector.frequencies.push(frequency);
            vector.cums.push(cum);
            cum += frequency;
            next_symbol = u32::from(symbol) + 1;
        }
        Ok(vector)
    }

    /// The vector for the next symbol after `history`, the content bytes so far.
    fn vector(&self, history: &[u8]) -> Option<Vector<'_>> {
        match &self.0 {
            ModelKind::Model0 => Some(Vector::Model0),
            ModelKind::Context(model) => model.vector(history).map(Vector::Sparse),
        }
    }
}

/// The reference parse of 6.8.6: at each position the longest matching token,
/// else a literal byte.
pub fn reference_parse(content: &[u8]) -> Vec<u16> {
    // Token IDs grouped by their first byte.
    let mut by_first: Vec<Vec<u16>> = vec![Vec::new(); 256];
    for (id, token) in (256u16..).zip(TOKEN_TABLE_V0.iter()) {
        if let Some(group) = token.first().and_then(|&b| by_first.get_mut(usize::from(b))) {
            group.push(id);
        }
    }
    let mut symbols = Vec::with_capacity(content.len());
    let mut rest = content;
    while let Some(&first) = rest.first() {
        let mut best: Option<(u16, usize)> = None;
        for &id in by_first.get(usize::from(first)).map_or(&[][..], Vec::as_slice) {
            let token = TOKEN_TABLE_V0.get(usize::from(id) - 256).copied().unwrap_or_default();
            if rest.starts_with(token) && best.is_none_or(|(_, len)| token.len() > len) {
                best = Some((id, token.len()));
            }
        }
        let (symbol, len) = best.unwrap_or((u16::from(first), 1));
        symbols.push(symbol);
        rest = rest.get(len..).unwrap_or_default();
    }
    symbols
}

/// The encoder's `low` of 6.8.5 as an unbounded big-endian integer. Its length
/// is `n` + 4 bytes after `n` shifts.
struct Low {
    digits: Vec<u8>,
}

impl Low {
    fn add(digits: &mut [u8], value: u32) {
        let mut carry = u64::from(value);
        for digit in digits.iter_mut().rev() {
            if carry == 0 {
                break;
            }
            let sum = u64::from(*digit) + (carry & 0xFF);
            *digit = low_byte(sum);
            carry = (carry >> 8) + (sum >> 8);
        }
    }

    /// The coded field: the first `m` bytes of the smallest multiple of
    /// 2^(8(`M`−`m`)) at or above `low` that is below `low` + `range`, for the
    /// smallest such `m`.
    fn finish(self, range: u32) -> Vec<u8> {
        let low = self.digits;
        let mut last = low.clone();
        Self::add(&mut last, range - 1);
        // Leading bytes in which low and low + range − 1 agree.
        let agree = low.iter().zip(&last).take_while(|(a, b)| a == b).count();
        // Bytes up to the last non-zero byte of low.
        let significant = low.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let length = significant.min(agree + 1);
        let mut coded = low;
        coded.truncate(length);
        if length < significant {
            // Round up: the dropped tail of low is not zero.
            for digit in coded.iter_mut().rev() {
                let (sum, overflow) = digit.overflowing_add(1);
                *digit = sum;
                if !overflow {
                    break;
                }
            }
        }
        coded
    }
}

/// Encodes a symbol sequence (any parse) under `model`. Returns `None` if a
/// symbol is ≥ `A`.
pub fn encode_symbols(symbols: &[u16], model: &Model) -> Option<Vec<u8>> {
    let mut low = Low { digits: vec![0; 4] };
    let mut range = u32::MAX;
    let mut history = Vec::new();
    for &symbol in symbols {
        let (cum, frequency) = model.vector(&history)?.range_of(usize::from(symbol))?;
        let r = range >> 16;
        Low::add(&mut low.digits, r * cum);
        range = r * frequency;
        while range < TOP {
            low.digits.push(0);
            range <<= 8;
        }
        if !model.is_model0() {
            history.extend_from_slice(symbol_bytes(symbol)?.as_slice());
        }
    }
    Some(low.finish(range))
}

/// The coded field of `content` under `model`, with the reference parse.
/// Every content is applicable.
pub fn encode(content: &[u8], model: &Model) -> Vec<u8> {
    encode_symbols(&reference_parse(content), model).unwrap_or_default()
}

/// Decodes a codec-3 coded field into `decoded_len` bytes (the normative
/// decoder of 6.8.5).
///
/// # Errors
///
/// - [`CodecError::Malformed`]: `decoded_len` > [`crate::MAX_CONTENT_LEN_V0`];
///   `code / r` ≥ 2^16; a token that would go beyond `decoded_len` bytes; bytes
///   of `bytes` the decoder never read; a last byte of `00`.
/// - [`CodecError::TooLarge`]: `decoded_len` > `limit`.
pub fn decode(
    model: &Model,
    decoded_len: u32,
    bytes: &[u8],
    limit: u32,
) -> Result<Vec<u8>, CodecError> {
    check_decoded_len(decoded_len, limit)?;
    let total = to_usize(decoded_len)?;
    // Zero extension: C[i] = 0 for i ≥ Lc.
    let byte_at = |index: usize| u32::from(bytes.get(index).copied().unwrap_or(0));
    let mut code = byte_at(0) << 24 | byte_at(1) << 16 | byte_at(2) << 8 | byte_at(3);
    let mut range = u32::MAX;
    let mut next = 4usize;
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let vector = model.vector(&out).ok_or(CodecError::Malformed)?;
        let r = range >> 16;
        let value = code.checked_div(r).ok_or(CodecError::Malformed)?;
        if value >= TOTAL {
            return Err(CodecError::Malformed);
        }
        let symbol = vector.symbol_at(value).ok_or(CodecError::Malformed)?;
        let (cum, frequency) = vector.range_of(symbol).ok_or(CodecError::Malformed)?;
        code = code.checked_sub(r * cum).ok_or(CodecError::Malformed)?;
        range = r * frequency;
        while range < TOP {
            code = (code << 8) | byte_at(next);
            next += 1;
            range <<= 8;
        }
        let symbol = u16::try_from(symbol).map_err(|_| CodecError::Malformed)?;
        let piece = symbol_bytes(symbol).ok_or(CodecError::Malformed)?;
        if piece.as_slice().len() > total - out.len() {
            return Err(CodecError::Malformed);
        }
        out.extend_from_slice(piece.as_slice());
    }
    if bytes.len() > next || bytes.last() == Some(&0) {
        return Err(CodecError::Malformed);
    }
    Ok(out)
}
