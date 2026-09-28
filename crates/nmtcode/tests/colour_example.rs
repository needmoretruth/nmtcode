//! Chapter 7 by hand: the colour cells, reference cells and colour layer of the worked example
//! of 7.13 (the colour symbol of chapter 3, 3.10 e), and the smallest valid colour sizes of
//! 7.8.2. This version makes no colour symbols, so the test builds the layer from the layer
//! crates and the rules of 7.5 to 7.8.

use nmtcode_core::{FormatWord, SymbolClass, crc32c, pad_message};
use nmtcode_symbol::{Layout, ModuleClass, Whitening};

/// Reference modules per copy (7.5, `reference_modules_per_copy`).
const REFERENCE_MODULES_PER_COPY: usize = 32;
/// Distance of the reference anchors from the symbol's edges, in modules (7.5).
const ANCHOR: u32 = 10;
/// Seed of the colour layer's whitening (7.8.4).
const CHROMA_SEED: u32 = 0x644E_9D0D;

/// A colour cell: its position in the cell grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    i: u32,
    j: u32,
}

/// The colour-cell layout of 7.5 and 7.7 for a symbol of `layout` with cells of `c` × `c`.
struct ColourLayout {
    c: u32,
    /// Every colour cell in colour-cell order.
    cells: Vec<Cell>,
    /// Indices into `cells` of the four reference groups TL, TR, BL, BR, each in colour-cell
    /// order; copy A is TL and BR, copy B is TR and BL.
    groups: [Vec<usize>; 4],
    /// Indices into `cells` of the data cells d(0), d(1), … in colour-cell order.
    data: Vec<usize>,
}

impl ColourLayout {
    fn new(layout: &Layout, c: u32) -> Self {
        let (w, h) = (layout.width(), layout.height());
        let mut cells = Vec::new();
        for j in 0..h / c {
            for i in 0..w / c {
                let all_data = (0..c).all(|dy| {
                    (0..c).all(|dx| {
                        layout.module_class(c * i + dx, c * j + dy) == Some(ModuleClass::Data)
                    })
                });
                if all_data {
                    cells.push(Cell { i, j });
                }
            }
        }
        let per_group = REFERENCE_MODULES_PER_COPY / (c * c) as usize / 2;
        // Anchors in doubled module coordinates, corners in the order TL, TR, BL, BR.
        let anchors = [
            (2 * ANCHOR, 2 * ANCHOR),
            (2 * (w - ANCHOR), 2 * ANCHOR),
            (2 * ANCHOR, 2 * (h - ANCHOR)),
            (2 * (w - ANCHOR), 2 * (h - ANCHOR)),
        ];
        let mut taken = vec![false; cells.len()];
        let mut groups: [Vec<usize>; 4] = Default::default();
        for (group, &(ax, ay)) in groups.iter_mut().zip(anchors.iter()) {
            let mut order: Vec<(i64, usize)> = cells
                .iter()
                .enumerate()
                .filter(|&(k, _)| !taken[k])
                .map(|(k, cell)| {
                    let cx = i64::from(2 * c * cell.i + c) - i64::from(ax);
                    let cy = i64::from(2 * c * cell.j + c) - i64::from(ay);
                    (cx * cx + cy * cy, k)
                })
                .collect();
            order.sort_unstable();
            let mut chosen: Vec<usize> = order.iter().take(per_group).map(|&(_, k)| k).collect();
            chosen.sort_unstable();
            for &k in &chosen {
                taken[k] = true;
            }
            *group = chosen;
        }
        let data = (0..cells.len()).filter(|&k| !taken[k]).collect();
        Self { c, cells, groups, data }
    }

    /// The fixed chroma bit of reference cell `position` within its group (7.7): the first
    /// half of each group has bit 0, the second half bit 1.
    fn reference_bit(&self, position: usize) -> bool {
        let per_group = self.groups[0].len();
        position >= per_group / 2
    }

    /// `N_c` = floor(`N_data` / 8) (7.8.2).
    fn codewords(&self) -> usize {
        self.data.len() / 8
    }

    /// Every module of every reference cell, as (x, y).
    fn reference_modules(&self) -> Vec<(u32, u32)> {
        let c = self.c;
        self.groups
            .iter()
            .flatten()
            .flat_map(|&k| {
                let cell = self.cells[k];
                (0..c).flat_map(move |dy| (0..c).map(move |dx| (c * cell.i + dx, c * cell.j + dy)))
            })
            .collect()
    }
}

/// The colour of a module (7.4.1) of class `dark` and chroma bit `bit`: K, B, Y or W.
fn palette(dark: bool, bit: bool) -> char {
    match (dark, bit) {
        (true, false) => 'K',
        (true, true) => 'B',
        (false, false) => 'Y',
        (false, true) => 'W',
    }
}

fn bytes(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|pair| u8::from_str_radix(pair, 16).unwrap()).collect()
}

/// The base container of 3.10 e (49 bytes) and its extension container (23 bytes).
fn containers() -> (Vec<u8>, Vec<u8>) {
    let digest = [
        0x88, 0x42, 0x74, 0x7E, 0x65, 0xE1, 0xE1, 0xBB, 0xCF, 0x64, 0x6D, 0x36, 0xFC, 0x53, 0x53,
        0x49, 0x0E, 0xB4, 0x4D, 0x5E, 0xEC, 0x5A, 0xFF, 0xC7, 0xBD, 0x1F, 0x41, 0x03, 0xBE, 0xF9,
        0xEA, 0xD5,
    ];
    let mut base = vec![0x10, 0x10, 0x2A, 0x01];
    base.extend_from_slice(&digest);
    base.push(0x01);
    base.extend_from_slice(b"NMT Code");
    let crc = crc32c(&base);
    base.extend_from_slice(&crc.to_be_bytes());
    let mut extension = vec![0x00, 0x10, 0x10, 0x01];
    extension.extend_from_slice("안녕하세요".as_bytes());
    let crc = crc32c(&extension);
    extension.extend_from_slice(&crc.to_be_bytes());
    (base, extension)
}

/// The symbol of 3.10 e as a generator draws its base layer (chapters 2 to 5).
fn example_symbol() -> (Layout, nmtcode_core::ModuleGrid, FormatWord) {
    let (base, _) = containers();
    let layout = Layout::new(32, 32).unwrap();
    let split = nmtcode_ecc::split(layout.codeword_count(), 0).unwrap();
    assert_eq!((layout.codeword_count(), split.block_count()), (98, 1));
    assert_eq!((split.parity_per_block(), split.capacity()), (16, 82));
    let mut message = base;
    pad_message(&mut message, split.capacity()).unwrap();
    let stream = nmtcode_ecc::encode_stream(&split, &message).unwrap();
    let word = FormatWord::new(SymbolClass::Static, 32, 32, 0, 1, 0).unwrap();
    let grid = layout.draw_copies(word.encode_copies(), &stream).unwrap();
    (layout, grid, word)
}

/// 7.13.1: the symbol and the sizes of its two layers.
#[test]
fn symbol_and_layers_of_7_13_1() {
    let (base, extension) = containers();
    assert_eq!(base.len(), 49);
    assert_eq!(&base[45..], &[0x03, 0xDB, 0x0A, 0xA4]);
    assert_eq!(extension.len(), 23);
    assert_eq!(&extension[19..], &[0x28, 0xCA, 0x4D, 0x68]);
    let (layout, _, word) = example_symbol();
    assert_eq!(word.data(), 0x002_0082);
    assert_eq!(word.encode_copies(), [0x51E3_6D58_9747, 0x3FDA_30A8_27CB]);
    let colour = ColourLayout::new(&layout, 1);
    assert_eq!(colour.cells.len(), 786);
    assert_eq!(colour.cells.len(), layout.data_module_count());
    assert_eq!(colour.groups.clone().map(|g| g.len()), [16; 4]);
    assert_eq!(colour.data.len(), 722);
    assert_eq!((colour.codewords(), colour.data.len() - 8 * colour.codewords()), (90, 2));
    let split = nmtcode_ecc::split(90, 2).unwrap();
    assert_eq!((split.block_count(), split.parity_per_block(), split.capacity()), (1, 46, 44));
    // With c = 2 the same size is too small for the extension container.
    let coarse = ColourLayout::new(&layout, 2);
    assert_eq!(coarse.codewords(), 22);
    assert_eq!(nmtcode_ecc::split(22, 2).unwrap().capacity(), 10);
}

/// 7.13.2: the four reference groups, their fixed bits and module colours.
#[test]
fn reference_cells_of_7_13_2() {
    let (layout, grid, _) = example_symbol();
    let colour = ColourLayout::new(&layout, 1);
    let expected: [(u32, u32, [&str; 4]); 4] = [
        (8, 8, ["YYKK", "KKYK", "WWBW", "WBWW"]),
        (20, 8, ["YKYK", "KKYY", "BWBW", "WBBW"]),
        (8, 20, ["KYYK", "KYKY", "BBBW", "BBWB"]),
        (20, 20, ["YKKK", "KYKY", "WBWB", "WWBB"]),
    ];
    let mut totals = [0usize; 4];
    for (group, (x0, y0, rows)) in colour.groups.iter().zip(expected) {
        let cells: Vec<Cell> = group.iter().map(|&k| colour.cells[k]).collect();
        let block: Vec<Cell> =
            (y0..y0 + 4).flat_map(|j| (x0..x0 + 4).map(move |i| Cell { i, j })).collect();
        assert_eq!(cells, block);
        for (position, cell) in cells.iter().enumerate() {
            let bit = colour.reference_bit(position);
            assert_eq!(bit, cell.j >= y0 + 2);
            let got = palette(grid.get(cell.i, cell.j).unwrap(), bit);
            let row = rows[usize::try_from(cell.j - y0).unwrap()];
            assert_eq!(got, row.as_bytes()[usize::try_from(cell.i - x0).unwrap()] as char);
            totals["KYBW".find(got).unwrap()] += 1;
        }
    }
    // 18 black, 14 yellow, 16 blue, 16 white.
    assert_eq!(totals, [18, 14, 16, 16]);
}

/// 7.13.3: the colour message, its parity and the first 16 data cells.
#[test]
fn colour_layer_of_7_13_3() {
    let (layout, grid, _) = example_symbol();
    let (_, extension) = containers();
    let colour = ColourLayout::new(&layout, 1);
    let split = nmtcode_ecc::split(colour.codewords(), 2).unwrap();
    let mut message = extension;
    pad_message(&mut message, split.capacity()).unwrap();
    assert_eq!(
        message,
        bytes(
            "00 10 10 01 EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 28 CA 4D 68 EC \
             11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC"
        )
    );
    let stream = nmtcode_ecc::encode_stream(&split, &message).unwrap();
    assert_eq!(&stream[..44], message.as_slice());
    assert_eq!(
        &stream[44..],
        bytes(
            "3E F4 A3 BB 11 95 9C 99 A7 DE B7 27 E0 79 F4 AA DA BE BC 44 6D F9 BA 7A \
             EB 3D 49 2F EA 0C 59 20 18 15 C1 DC DC 79 D0 4F 01 20 28 F1 F9 43"
        )
        .as_slice()
    );
    let mut whitening = Whitening::with_seed(CHROMA_SEED);
    let u: Vec<bool> = (0..16).map(|_| whitening.next_bit()).collect();
    // (x of the module in row 0, s, u, t, dark, colour) for d(0) … d(15).
    let rows: [(u32, u8, u8, u8, bool, char); 16] = [
        (10, 0, 1, 1, true, 'B'),
        (11, 0, 1, 1, false, 'W'),
        (12, 0, 0, 0, false, 'Y'),
        (13, 0, 0, 0, true, 'K'),
        (14, 0, 1, 1, true, 'B'),
        (15, 0, 0, 0, true, 'K'),
        (16, 0, 0, 0, true, 'K'),
        (17, 0, 0, 0, true, 'K'),
        (18, 0, 1, 1, true, 'B'),
        (19, 0, 0, 0, false, 'Y'),
        (20, 0, 0, 0, true, 'K'),
        (21, 1, 1, 0, false, 'Y'),
        (22, 0, 1, 1, false, 'W'),
        (23, 0, 1, 1, true, 'B'),
        (24, 0, 0, 0, false, 'Y'),
        (25, 0, 1, 1, false, 'W'),
    ];
    for (k, (x, s, uk, t, dark, name)) in rows.into_iter().enumerate() {
        let cell = colour.cells[colour.data[k]];
        assert_eq!((cell.i, cell.j), (x, 0), "d({k})");
        let bit = stream[k / 8] >> (7 - k % 8) & 1;
        assert_eq!(bit, s, "s[{k}]");
        assert_eq!(u8::from(u[k]), uk, "u[{k}]");
        assert_eq!(bit ^ uk, t, "t[{k}]");
        assert_eq!(grid.get(x, 0), Some(dark), "d({k})");
        assert_eq!(palette(dark, t == 1), name, "d({k})");
    }
}

/// 7.13.4: the classification arithmetic of two modules (7.9.2 step 5).
#[test]
fn classification_of_7_13_4() {
    let k = [18.0, 20.0, 30.0];
    let wt = [230.0, 225.0, 210.0];
    let normalise = |v: [f64; 3]| -> [f64; 3] {
        [0, 1, 2].map(|i| ((v[i] - k[i]) / (wt[i] - k[i])).clamp(-0.25, 1.25))
    };
    let score = |v: [f64; 3], p0: [f64; 3], p1: [f64; 3]| -> (f64, f64) {
        let axis = [0, 1, 2].map(|i| p1[i] - p0[i]);
        let dot: f64 = (0..3).map(|i| (v[i] - p0[i]) * axis[i]).sum();
        let norm: f64 = axis.iter().map(|a| a * a).sum();
        let t = dot / norm;
        (t, (t - 0.5).clamp(-1.0, 1.0))
    };
    // Values in ten-thousandths, as 7.13.4 prints them to four decimals.
    #[allow(clippy::cast_possible_truncation)]
    let round4 = |x: f64| (x * 10_000.0).round() as i64;
    let dark = normalise([40.0, 45.0, 190.0]);
    assert_eq!(dark.map(round4), [1038, 1220, 8889]);
    let (t1, e1) = score(dark, [0.02, 0.03, 0.05], [0.10, 0.12, 0.80]);
    assert_eq!((round4(t1), round4(e1)), (11_164, 6164));
    let light = normalise([215.0, 212.0, 120.0]);
    assert_eq!(light.map(round4), [9292, 9366, 5000]);
    let (t2, e2) = score(light, [0.97, 0.95, 0.12], [0.98, 0.97, 0.96]);
    assert_eq!((round4(t2), round4(e2)), (4511, -489));
    assert_eq!(round4(e1 + e2), 5675);
    assert!(e1 + e2 > 0.0);
}

/// 7.8.2: the smallest valid sizes of colour profile 1, where the colour layer has at least 16
/// codewords, and the sizes the table gives for comparison.
#[test]
fn smallest_valid_colour_sizes_of_7_8_2() {
    let n_c =
        |w: u32, h: u32, c: u32| ColourLayout::new(&Layout::new(w, h).unwrap(), c).codewords();
    for (w, h, c, expected) in [
        (20, 24, 1, 22),
        (24, 20, 1, 22),
        (24, 24, 1, 34),
        (20, 20, 1, 12),
        (24, 36, 2, 17),
        (36, 24, 2, 17),
        (32, 32, 2, 22),
        (28, 28, 2, 15),
        (24, 32, 2, 14),
    ] {
        assert_eq!(n_c(w, h, c), expected, "{w} x {h}, c = {c}");
    }
    for (c, smallest, square) in [(1, 480, 24), (2, 864, 32)] {
        let valid: Vec<(u32, u32)> = (20..=48)
            .step_by(4)
            .flat_map(|w| (20..=48).step_by(4).map(move |h| (w, h)))
            .filter(|&(w, h)| n_c(w, h, c) >= 16)
            .collect();
        assert_eq!(valid.iter().map(|&(w, h)| w * h).min(), Some(smallest), "c = {c}");
        assert_eq!(valid.iter().filter(|(w, h)| w == h).map(|&(w, _)| w).min(), Some(square));
    }
}

/// 7.5: the reference groups take 2 · `R_ref` distinct colour cells in every size, and in a
/// 20 × 20 symbol the four anchors coincide.
#[test]
fn reference_groups_of_7_5() {
    for c in [1u32, 2] {
        for (w, h) in [(20, 24), (24, 24), (32, 32), (48, 20), (64, 64), (100, 48)] {
            let layout = Layout::new(w, h).unwrap();
            let colour = ColourLayout::new(&layout, c);
            let per_group = 16 / usize::try_from(c * c).unwrap();
            assert_eq!(colour.groups.clone().map(|g| g.len()), [per_group; 4], "{w} x {h}");
            let mut all: Vec<usize> = colour.groups.iter().flatten().copied().collect();
            all.sort_unstable();
            all.dedup();
            assert_eq!(all.len(), 4 * per_group);
            assert_eq!(colour.data.len(), colour.cells.len() - 4 * per_group);
            // No reference module lies in the two outermost rows or columns.
            for (x, y) in colour.reference_modules() {
                assert!(x >= 2 && y >= 2 && x < w - 2 && y < h - 2, "{w} x {h}: ({x}, {y})");
            }
        }
    }
    let small = ColourLayout::new(&Layout::new(20, 20).unwrap(), 1);
    let first: Vec<Cell> = small.groups[0].iter().map(|&k| small.cells[k]).collect();
    assert_eq!(first[0], Cell { i: 8, j: 8 });
    assert!(first.iter().all(|cell| (8..12).contains(&cell.i) && (8..12).contains(&cell.j)));
}
