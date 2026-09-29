//! A baseline JPEG encoder (ITU-T T.81, sequential DCT, Huffman coding with the example tables
//! of Annex K), for the compression step of the channel model and for test files.
//!
//! Greyscale images are coded as one component; RGB images as YCbCr with 4:2:0 chroma
//! subsampling, as phone cameras store them. Quality scales the Annex K quantisation tables by
//! the widely used rule: 5000 / q below 50, 200 − 2q from 50.

/// Luminance quantisation table of Annex K (K.1), row-major.
const LUMA_Q: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Chrominance quantisation table of Annex K (K.2), row-major.
const CHROMA_Q: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// Zig-zag order: `ZIGZAG[k]` is the row-major index of the k-th coefficient.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// A Huffman code table: code and length per symbol.
struct Huffman {
    code: [u16; 256],
    len: [u8; 256],
}

impl Huffman {
    fn new(bits: &[u8; 16], values: &[u8]) -> Self {
        let mut table = Self { code: [0; 256], len: [0; 256] };
        let mut code: u16 = 0;
        let mut k = 0usize;
        for (length, &count) in (1u8..).zip(bits.iter()) {
            for _ in 0..count {
                if let Some(&v) = values.get(k) {
                    let v = usize::from(v);
                    if let (Some(c), Some(l)) = (table.code.get_mut(v), table.len.get_mut(v)) {
                        *c = code;
                        *l = length;
                    }
                }
                code = code.wrapping_add(1);
                k += 1;
            }
            code = code.wrapping_shl(1);
        }
        table
    }
}

/// Entropy-coded output with byte stuffing.
struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, bits: u32) {
        if bits == 0 {
            return;
        }
        self.acc = (self.acc << bits) | (value & ((1u32 << bits) - 1));
        self.n += bits;
        while self.n >= 8 {
            let byte = u8::try_from((self.acc >> (self.n - 8)) & 0xFF).unwrap_or(0);
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1u32 << self.n).wrapping_sub(1);
    }

    fn huff(&mut self, table: &Huffman, symbol: u8) {
        let s = usize::from(symbol);
        let (code, len) =
            (table.code.get(s).copied().unwrap_or(0), table.len.get(s).copied().unwrap_or(0));
        self.put(u32::from(code), u32::from(len));
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
        self.out
    }
}

/// The quantisation table `base` at `quality` (1 to 100), row-major.
fn scaled(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = u32::from(quality.clamp(1, 100));
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    base.map(|b| u16::try_from(((u32::from(b) * scale + 50) / 100).clamp(1, 255)).unwrap_or(255))
}

/// The DCT basis: `C[u][x]` = C(u) / 2 · cos((2x + 1) u π / 16).
fn basis() -> [[f64; 8]; 8] {
    let mut c = [[0.0; 8]; 8];
    for (u, row) in c.iter_mut().enumerate() {
        let cu = if u == 0 { core::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
        for (x, v) in row.iter_mut().enumerate() {
            let (uf, xf) =
                (f64::from(u8::try_from(u).unwrap_or(0)), f64::from(u8::try_from(x).unwrap_or(0)));
            *v = cu / 2.0 * ((2.0 * xf + 1.0) * uf * core::f64::consts::PI / 16.0).cos();
        }
    }
    c
}

/// Bits needed for the magnitude of `v`, and the value bits as T.81 sends them.
fn category(v: i32) -> (u32, u32) {
    let magnitude = v.unsigned_abs();
    let size = 32 - magnitude.leading_zeros();
    let bits =
        if v < 0 { (v - 1).cast_unsigned() & ((1u32 << size).wrapping_sub(1)) } else { magnitude };
    (size, bits)
}

/// Encodes one 8 × 8 block (level-shifted samples, row-major).
fn block(
    w: &mut BitWriter,
    samples: &[f64; 64],
    q: &[u16; 64],
    c: &[[f64; 8]; 8],
    dc_prev: &mut i32,
    dc: &Huffman,
    ac: &Huffman,
) {
    // Rows, then columns.
    let mut tmp = [0.0f64; 64];
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += c[u][x] * samples[y * 8 + x];
            }
            tmp[y * 8 + u] = s;
        }
    }
    let mut coef = [0i32; 64];
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += c[v][y] * tmp[y * 8 + u];
            }
            let quant = f64::from(q[v * 8 + u]);
            // Rounded quotient; the values stay far inside i32.
            coef[v * 8 + u] = round_i32(s / quant);
        }
    }
    let diff = coef[0] - *dc_prev;
    *dc_prev = coef[0];
    let (size, bits) = category(diff);
    w.huff(dc, u8::try_from(size).unwrap_or(0));
    w.put(bits, size);
    let mut run = 0u32;
    for k in 1..64 {
        let v = coef[ZIGZAG[k]];
        if v == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            w.huff(ac, 0xF0);
            run -= 16;
        }
        let (size, bits) = category(v);
        w.huff(ac, u8::try_from((run << 4) | size).unwrap_or(0));
        w.put(bits, size);
        run = 0;
    }
    if run > 0 {
        w.huff(ac, 0x00);
    }
}

#[allow(clippy::cast_possible_truncation)]
fn round_i32(v: f64) -> i32 {
    // Saturating cast of a finite coefficient.
    v.round() as i32
}

fn marker(out: &mut Vec<u8>, code: u8, payload: &[u8]) {
    out.extend_from_slice(&[0xFF, code]);
    let len = u16::try_from(payload.len() + 2).unwrap_or(u16::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
}

fn dqt(out: &mut Vec<u8>, id: u8, table: &[u16; 64]) {
    let mut p = vec![id];
    p.extend(ZIGZAG.iter().map(|&i| u8::try_from(table[i]).unwrap_or(255)));
    marker(out, 0xDB, &p);
}

fn dht(out: &mut Vec<u8>, class_id: u8, bits: &[u8; 16], values: &[u8]) {
    let mut p = vec![class_id];
    p.extend_from_slice(bits);
    p.extend_from_slice(values);
    marker(out, 0xC4, &p);
}

/// An EXIF APP1 payload holding only the orientation tag (big-endian TIFF).
fn exif(orientation: u16) -> Vec<u8> {
    let mut p = b"Exif\0\0MM".to_vec();
    p.extend_from_slice(&42u16.to_be_bytes());
    p.extend_from_slice(&8u32.to_be_bytes());
    p.extend_from_slice(&1u16.to_be_bytes());
    p.extend_from_slice(&0x0112u16.to_be_bytes());
    p.extend_from_slice(&3u16.to_be_bytes());
    p.extend_from_slice(&1u32.to_be_bytes());
    p.extend_from_slice(&orientation.to_be_bytes());
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&0u32.to_be_bytes());
    p
}

fn header(out: &mut Vec<u8>, orientation: Option<u16>) {
    out.extend_from_slice(&[0xFF, 0xD8]);
    marker(out, 0xE0, b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
    if let Some(o) = orientation {
        marker(out, 0xE1, &exif(o));
    }
}

fn sof(out: &mut Vec<u8>, width: u16, height: u16, components: &[(u8, u8, u8)]) {
    let mut p = vec![8];
    p.extend_from_slice(&height.to_be_bytes());
    p.extend_from_slice(&width.to_be_bytes());
    p.push(u8::try_from(components.len()).unwrap_or(1));
    for &(id, sampling, tq) in components {
        p.extend_from_slice(&[id, sampling, tq]);
    }
    marker(out, 0xC0, &p);
}

fn sos(out: &mut Vec<u8>, components: &[(u8, u8)]) {
    let mut p = vec![u8::try_from(components.len()).unwrap_or(1)];
    for &(id, tables) in components {
        p.extend_from_slice(&[id, tables]);
    }
    p.extend_from_slice(&[0, 63, 0]);
    marker(out, 0xDA, &p);
}

/// Encodes a greyscale image (`width × height` bytes, row-major) at `quality` (1 to 100), with an
/// EXIF orientation tag when given. `None` when the buffer does not match or a side is 0 or
/// above 65 535.
pub fn encode_luma(
    width: u32,
    height: u32,
    pixels: &[u8],
    quality: u8,
    orientation: Option<u16>,
) -> Option<Vec<u8>> {
    let (w16, h16) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    let (w, h) = (usize::from(w16), usize::from(h16));
    if w == 0 || h == 0 || pixels.len() != w * h {
        return None;
    }
    let q = scaled(&LUMA_Q, quality);
    let c = basis();
    let mut out = Vec::new();
    header(&mut out, orientation);
    dqt(&mut out, 0, &q);
    sof(&mut out, w16, h16, &[(1, 0x11, 0)]);
    dht(&mut out, 0x00, &DC_LUMA_BITS, &DC_VALUES);
    dht(&mut out, 0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES);
    sos(&mut out, &[(1, 0x00)]);
    let (dc, ac) =
        (Huffman::new(&DC_LUMA_BITS, &DC_VALUES), Huffman::new(&AC_LUMA_BITS, &AC_LUMA_VALUES));
    let mut writer = BitWriter { out: Vec::with_capacity(w * h / 4), acc: 0, n: 0 };
    let mut prev = 0i32;
    let at = |x: usize, y: usize| {
        f64::from(pixels.get(y.min(h - 1) * w + x.min(w - 1)).copied().unwrap_or(0)) - 128.0
    };
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            let mut s = [0.0f64; 64];
            for (i, v) in s.iter_mut().enumerate() {
                *v = at(bx + i % 8, by + i / 8);
            }
            block(&mut writer, &s, &q, &c, &mut prev, &dc, &ac);
        }
    }
    out.extend(writer.finish());
    out.extend_from_slice(&[0xFF, 0xD9]);
    Some(out)
}

/// Encodes an RGB image (`width × height × 3` bytes) as YCbCr 4:2:0 at `quality`, with an EXIF
/// orientation tag when given. `None` as for [`encode_luma`].
pub fn encode_rgb(
    width: u32,
    height: u32,
    rgb: &[u8],
    quality: u8,
    orientation: Option<u16>,
) -> Option<Vec<u8>> {
    let (w16, h16) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    let (w, h) = (usize::from(w16), usize::from(h16));
    if w == 0 || h == 0 || rgb.len() != w * h * 3 {
        return None;
    }
    let (ql, qc) = (scaled(&LUMA_Q, quality), scaled(&CHROMA_Q, quality));
    let c = basis();
    let mut out = Vec::new();
    header(&mut out, orientation);
    dqt(&mut out, 0, &ql);
    dqt(&mut out, 1, &qc);
    sof(&mut out, w16, h16, &[(1, 0x22, 0), (2, 0x11, 1), (3, 0x11, 1)]);
    dht(&mut out, 0x00, &DC_LUMA_BITS, &DC_VALUES);
    dht(&mut out, 0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES);
    dht(&mut out, 0x01, &DC_CHROMA_BITS, &DC_VALUES);
    dht(&mut out, 0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES);
    sos(&mut out, &[(1, 0x00), (2, 0x11), (3, 0x11)]);
    let dcl = Huffman::new(&DC_LUMA_BITS, &DC_VALUES);
    let acl = Huffman::new(&AC_LUMA_BITS, &AC_LUMA_VALUES);
    let dcc = Huffman::new(&DC_CHROMA_BITS, &DC_VALUES);
    let acc = Huffman::new(&AC_CHROMA_BITS, &AC_CHROMA_VALUES);
    let px = |x: usize, y: usize| -> (f64, f64, f64) {
        let i = (y.min(h - 1) * w + x.min(w - 1)) * 3;
        let g = |k: usize| f64::from(rgb.get(i + k).copied().unwrap_or(0));
        (g(0), g(1), g(2))
    };
    let ycc = |x: usize, y: usize| -> (f64, f64, f64) {
        let (r, g, b) = px(x, y);
        (
            0.299 * r + 0.587 * g + 0.114 * b,
            -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0,
            0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0,
        )
    };
    let mut writer = BitWriter { out: Vec::with_capacity(w * h / 4), acc: 0, n: 0 };
    let (mut py, mut pcb, mut pcr) = (0i32, 0i32, 0i32);
    for my in (0..h).step_by(16) {
        for mx in (0..w).step_by(16) {
            for (ox, oy) in [(0, 0), (8, 0), (0, 8), (8, 8)] {
                let mut s = [0.0f64; 64];
                for (i, v) in s.iter_mut().enumerate() {
                    *v = ycc(mx + ox + i % 8, my + oy + i / 8).0 - 128.0;
                }
                block(&mut writer, &s, &ql, &c, &mut py, &dcl, &acl);
            }
            let mut cb = [0.0f64; 64];
            let mut cr = [0.0f64; 64];
            for i in 0..64 {
                let (x, y) = (mx + 2 * (i % 8), my + 2 * (i / 8));
                let (mut sb, mut sr) = (0.0, 0.0);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (_, b, r) = ycc(x + dx, y + dy);
                    sb += b;
                    sr += r;
                }
                cb[i] = sb / 4.0 - 128.0;
                cr[i] = sr / 4.0 - 128.0;
            }
            block(&mut writer, &cb, &qc, &c, &mut pcb, &dcc, &acc);
            block(&mut writer, &cr, &qc, &c, &mut pcr, &dcc, &acc);
        }
    }
    out.extend(writer.finish());
    out.extend_from_slice(&[0xFF, 0xD9]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_tables_code_every_symbol() {
        // AC symbols: end of block, run of 16 zeros, and every (run, size) with size 1 to 10.
        let mut needed: Vec<u8> = vec![0x00, 0xF0];
        for run in 0..16u8 {
            for size in 1..=10u8 {
                needed.push((run << 4) | size);
            }
        }
        for values in [&AC_LUMA_VALUES, &AC_CHROMA_VALUES] {
            let mut have = values.to_vec();
            have.sort_unstable();
            let mut want = needed.clone();
            want.sort_unstable();
            assert_eq!(have, want);
        }
        let sum = |bits: &[u8; 16]| bits.iter().map(|&b| usize::from(b)).sum::<usize>();
        assert_eq!(sum(&AC_LUMA_BITS), 162);
        assert_eq!(sum(&AC_CHROMA_BITS), 162);
        assert_eq!(sum(&DC_LUMA_BITS), 12);
        assert_eq!(sum(&DC_CHROMA_BITS), 12);
    }

    #[test]
    fn categories() {
        assert_eq!(category(0), (0, 0));
        assert_eq!(category(1), (1, 1));
        assert_eq!(category(-1), (1, 0));
        assert_eq!(category(-3), (2, 0));
        assert_eq!(category(5), (3, 5));
        assert_eq!(category(-5), (3, 2));
    }

    #[test]
    fn quality_scaling() {
        assert_eq!(scaled(&LUMA_Q, 50), LUMA_Q);
        assert!(scaled(&LUMA_Q, 100).iter().all(|&v| v == 1));
        assert_eq!(scaled(&LUMA_Q, 80)[0], 6);
    }
}
