//! Everything that depends only on the symbol size: the class of every module (5.2 to 5.7), the
//! counts D, N and R (5.7), the placement order (5.8), and drawing and reading the modules of a
//! symbol (5.9, 5.10).

use alloc::vec;
use alloc::vec::Vec;
use core::iter::FusedIterator;

use nmtcode_core::{ModuleGrid, is_valid_side};

use crate::finder::Corner;
use crate::format::{FORMAT_BITS, FORMAT_COPY_A, check_codeword, is_copy_a, write_format_copies};
use crate::marks;
use crate::whitening::Whitening;
use crate::{FINDER_SIZE, REFERENCE_MARK_SIZE, SEPARATOR_WIDTH, SymbolError};

/// Side of the square at each corner that holds the finder and its separator (5.3, 5.4).
const CORNER_BLOCK: u32 = FINDER_SIZE + SEPARATOR_WIDTH;

/// Function modules that every symbol has, whatever its size: 4 finders of 25 modules (5.3),
/// 44 separator modules (5.4) and two format copies of 47 modules (5.5).
const FIXED_FUNCTION_MODULES: u64 = 4 * 25 + 44 + 2 * 47;

/// Modules of one reference mark (5.6).
const MARK_MODULES: u64 = 9;

/// The class of a module (5.2). Every module of a symbol belongs to exactly one.
///
/// Finder, separator, format and reference-mark modules are the function modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleClass {
    /// Part of one of the four 5 × 5 finder patterns (5.3).
    Finder,
    /// Part of the light border, one module wide, on the two inner sides of a finder (5.4).
    Separator,
    /// One of the 47 modules of format copy A or of format copy B (5.5).
    Format,
    /// Part of a 3 × 3 reference mark (5.6).
    ReferenceMark,
    /// A data module: carries a whitened codeword bit or a whitened remainder bit (5.7 to 5.10).
    Data,
}

impl ModuleClass {
    /// True for the function modules: finder, separator, format and reference mark (5.2).
    pub const fn is_function(self) -> bool {
        !matches!(self, Self::Data)
    }
}

/// The counts of 5.6.1 and 5.7 for one symbol size, computed from the formula of 5.7.
///
/// [`Layout`] gives the same numbers from a module-by-module count; this type gives them
/// without building the placement order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolCounts {
    /// M, the number of reference marks: (`n_W` + 1) · (`n_H` + 1) − 4 (5.6.1).
    pub reference_marks: usize,
    /// D, the number of data modules: W · H − 100 − 44 − 94 − 9 · M (5.7).
    pub data_modules: usize,
    /// N = floor(D / 8), the number of codewords (5.7).
    pub codewords: usize,
    /// R = D mod 8, the number of remainder modules after the last codeword (5.7, 5.9).
    pub remainder_bits: usize,
}

impl SymbolCounts {
    /// The counts of a `width` × `height` symbol.
    ///
    /// # Errors
    ///
    /// [`SymbolError::InvalidSize`] when a side is not a multiple of 4 from 20 to 4108 (5.2),
    /// and [`SymbolError::TooLarge`] when a count does not fit in `usize` on this platform.
    pub fn new(width: u32, height: u32) -> Result<Self, SymbolError> {
        if !is_valid_side(width) || !is_valid_side(height) {
            return Err(SymbolError::InvalidSize { width, height });
        }
        let lines_w = u64::from(marks::interval_count(width)) + 1;
        let lines_h = u64::from(marks::interval_count(height)) + 1;
        let marks = lines_w * lines_h - 4;
        let data =
            u64::from(width) * u64::from(height) - FIXED_FUNCTION_MODULES - MARK_MODULES * marks;
        Self::from_counts(marks, data)
    }

    fn from_counts(marks: u64, data: u64) -> Result<Self, SymbolError> {
        let reference_marks = usize::try_from(marks).map_err(|_| SymbolError::TooLarge)?;
        let data_modules = usize::try_from(data).map_err(|_| SymbolError::TooLarge)?;
        Ok(Self {
            reference_marks,
            data_modules,
            codewords: data_modules / 8,
            remainder_bits: data_modules % 8,
        })
    }
}

/// Everything about a symbol that depends only on its width and height (chapter 5).
///
/// A layout is built once per size and then draws or reads any number of symbols of that size.
/// It holds the placement order of 5.8 as one bit per module in walk order, about W · H / 8
/// bytes, plus half of that for the rank table: about 3 MiB at 4108 × 4108.
///
/// Coordinates are (x, y) in modules with the origin at the top-left module and the quiet zone
/// excluded (chapter 1, 1.4).
#[derive(Clone, PartialEq, Eq)]
pub struct Layout {
    width: u32,
    height: u32,
    counts: SymbolCounts,
    x_lines: Vec<u32>,
    y_lines: Vec<u32>,
    /// `mark_columns[x]`: x is within one module of an x line of 5.6.1.
    mark_columns: Vec<bool>,
    /// `mark_rows[y]`: y is within one module of a y line of 5.6.1.
    mark_rows: Vec<bool>,
    /// 64-bit words per column pair; the walk of one pair is 2 · H bits (5.8).
    words_per_pair: u32,
    /// The walk of 5.8, one bit per module, set for a data module. Column pair p starts at word
    /// `p · words_per_pair`; in the pair, bit `2 · s + c` is column `2p + c` at step `s`, which
    /// is row `s` for an even pair and row `H − 1 − s` for an odd pair.
    walk: Vec<u64>,
    /// `rank[i]`: the number of data modules before word `i` of `walk`, that is the placement
    /// index k of the first data module in that word.
    rank: Vec<u32>,
}

impl core::fmt::Debug for Layout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Layout")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("counts", &self.counts)
            .field("x_lines", &self.x_lines)
            .field("y_lines", &self.y_lines)
            .finish_non_exhaustive()
    }
}

/// The word and the bit of `walk` that hold module (`x`, `y`). `x` < W and `y` < H.
fn walk_position(height: u32, words_per_pair: u32, x: u32, y: u32) -> (usize, u32) {
    let pair = x / 2;
    let step = if pair.is_multiple_of(2) { y } else { height - 1 - y };
    let bit = 2 * step + (x & 1);
    let word = u64::from(pair) * u64::from(words_per_pair) + u64::from(bit / 64);
    // The word index is below the length of `walk`, which is a `usize`.
    (usize::try_from(word).unwrap_or(usize::MAX), bit % 64)
}

impl Layout {
    /// The layout of a `width` × `height` symbol.
    ///
    /// # Errors
    ///
    /// [`SymbolError::InvalidSize`] when a side is not a multiple of 4 from 20 to 4108 (5.2),
    /// and [`SymbolError::TooLarge`] when the tables do not fit in memory addressing on this
    /// platform.
    pub fn new(width: u32, height: u32) -> Result<Self, SymbolError> {
        if !is_valid_side(width) || !is_valid_side(height) {
            return Err(SymbolError::InvalidSize { width, height });
        }
        let x_lines = marks::lines(width);
        let y_lines = marks::lines(height);
        let mark_columns = near_lines(width, &x_lines)?;
        let mark_rows = near_lines(height, &y_lines)?;

        let words_per_pair = (2 * height).div_ceil(64);
        let word_count = u64::from(width / 2) * u64::from(words_per_pair);
        let word_count = usize::try_from(word_count).map_err(|_| SymbolError::TooLarge)?;
        let per_pair = usize::try_from(words_per_pair).map_err(|_| SymbolError::TooLarge)?;
        let full_words = usize::try_from(2 * height / 64).map_err(|_| SymbolError::TooLarge)?;
        let last_bits = 2 * height % 64;

        // Every module of every column pair starts as a data module ...
        let mut walk = vec![0u64; word_count];
        for pair in walk.chunks_exact_mut(per_pair) {
            // `full_words` ≤ `per_pair`, so the split always succeeds.
            let Some((full, rest)) = pair.split_at_mut_checked(full_words) else {
                continue;
            };
            full.fill(u64::MAX);
            if last_bits > 0
                && let Some(word) = rest.first_mut()
            {
                *word = (1u64 << last_bits) - 1;
            }
        }
        // ... and every function module is taken out.
        for_each_function_module(width, height, &x_lines, &y_lines, |x, y| {
            let (word, bit) = walk_position(height, words_per_pair, x, y);
            if let Some(w) = walk.get_mut(word) {
                *w &= !(1u64 << bit);
            }
        });

        let mut rank = Vec::with_capacity(walk.len());
        let mut data = 0u32;
        for word in &walk {
            rank.push(data);
            data += word.count_ones();
        }
        let marks = mark_centres(&x_lines, &y_lines).count();
        let counts = SymbolCounts::from_counts(
            u64::try_from(marks).map_err(|_| SymbolError::TooLarge)?,
            u64::from(data),
        )?;
        Ok(Self {
            width,
            height,
            counts,
            x_lines,
            y_lines,
            mark_columns,
            mark_rows,
            words_per_pair,
            walk,
            rank,
        })
    }

    /// Width W in modules.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height H in modules.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// M, D, N and R of this size (5.6.1, 5.7).
    pub fn counts(&self) -> SymbolCounts {
        self.counts
    }

    /// D, the number of data modules (5.7).
    pub fn data_module_count(&self) -> usize {
        self.counts.data_modules
    }

    /// N = floor(D / 8), the number of codewords: the length of the codeword stream (5.7).
    pub fn codeword_count(&self) -> usize {
        self.counts.codewords
    }

    /// R = D mod 8, the number of remainder modules after the last codeword (5.7, 5.9).
    pub fn remainder_bits(&self) -> usize {
        self.counts.remainder_bits
    }

    /// M, the number of reference marks (5.6.1).
    pub fn reference_mark_count(&self) -> usize {
        self.counts.reference_marks
    }

    /// The x lines of 5.6.1, `xline[0]` = 2 to `xline[n_W]` = W − 3.
    pub fn x_lines(&self) -> &[u32] {
        &self.x_lines
    }

    /// The y lines of 5.6.1, `yline[0]` = 2 to `yline[n_H]` = H − 3.
    pub fn y_lines(&self) -> &[u32] {
        &self.y_lines
    }

    /// The centre of every reference mark (5.6.1): every (xline\[i\], yline\[j\]) except the
    /// four finder centres, by y line and then by x line.
    pub fn reference_mark_centres(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        mark_centres(&self.x_lines, &self.y_lines)
    }

    /// The class of module (`x`, `y`) (5.2), or `None` outside the symbol.
    pub fn module_class(&self, x: u32, y: u32) -> Option<ModuleClass> {
        let (w, h) = (self.width, self.height);
        if x >= w || y >= h {
            return None;
        }
        let side_x = x < CORNER_BLOCK || x >= w - CORNER_BLOCK;
        let side_y = y < CORNER_BLOCK || y >= h - CORNER_BLOCK;
        if side_x && side_y {
            let finder_x = x < FINDER_SIZE || x >= w - FINDER_SIZE;
            let finder_y = y < FINDER_SIZE || y >= h - FINDER_SIZE;
            return Some(if finder_x && finder_y {
                ModuleClass::Finder
            } else {
                ModuleClass::Separator
            });
        }
        if is_copy_a(x, y) || is_copy_a(w - 1 - x, h - 1 - y) {
            return Some(ModuleClass::Format);
        }
        let mark_x = self.mark_columns.get(usize::try_from(x).ok()?).copied();
        let mark_y = self.mark_rows.get(usize::try_from(y).ok()?).copied();
        if mark_x == Some(true) && mark_y == Some(true) {
            return Some(ModuleClass::ReferenceMark);
        }
        Some(ModuleClass::Data)
    }

    /// The fixed value of a finder, separator or reference-mark module, `true` for dark
    /// (5.3.1, 5.4, 5.6). `None` for format and data modules, whose value depends on the
    /// content, and outside the symbol.
    pub fn function_value(&self, x: u32, y: u32) -> Option<bool> {
        match self.module_class(x, y)? {
            ModuleClass::Finder => {
                let corner = match (x < FINDER_SIZE, y < FINDER_SIZE) {
                    (true, true) => Corner::TopLeft,
                    (false, true) => Corner::TopRight,
                    (true, false) => Corner::BottomLeft,
                    (false, false) => Corner::BottomRight,
                };
                let (ox, oy) = corner.origin(self.width, self.height)?;
                corner.module(x - ox, y - oy)
            }
            ModuleClass::Separator => Some(false),
            ModuleClass::ReferenceMark => {
                let centre = self.x_lines.binary_search(&x).is_ok()
                    && self.y_lines.binary_search(&y).is_ok();
                Some(!centre)
            }
            ModuleClass::Format | ModuleClass::Data => None,
        }
    }

    /// The data modules in placement order (5.8): the k-th item is P\[k\]. Yields exactly D
    /// items.
    pub fn placement(&self) -> Placement<'_> {
        Placement {
            walk: &self.walk,
            words_per_pair: self.words_per_pair,
            height: self.height,
            word: 0,
            pair: 0,
            in_pair: 0,
            bits: self.walk.first().copied().unwrap_or(0),
            remaining: self.counts.data_modules,
        }
    }

    /// The placement index k with P\[k\] = (`x`, `y`) (5.8), or `None` when (`x`, `y`) is not a
    /// data module. Codeword floor(k / 8) holds the module's bit when k < 8 · N (5.9).
    pub fn placement_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let (word, bit) = walk_position(self.height, self.words_per_pair, x, y);
        let bits = *self.walk.get(word)?;
        if bits >> bit & 1 == 0 {
            return None;
        }
        let before = (bits & ((1u64 << bit) - 1)).count_ones();
        usize::try_from(*self.rank.get(word)? + before).ok()
    }

    /// Draws a symbol: the function patterns (5.3, 5.4, 5.6), the two copies of the format word
    /// (2.6, 5.5), the codeword stream on the data modules in placement order with each
    /// codeword's most significant bit first (5.8, 5.9), and the remainder modules with bit 0,
    /// every data module combined by exclusive or with the whitening sequence (5.10).
    ///
    /// `format` is `[F_A, F_B]`, the sent words of copy A and copy B of chapter 2 (2.5): the
    /// codeword U XOR `MASK_A` and U XOR `MASK_B`, as `nmtcode_core::FormatWord::encode_copies`
    /// returns them. In each, integer bit `46 − i` is bit index `i` of 2.4.3: bit `i` of `F_A`
    /// goes on module A\[i\] and bit `i` of `F_B` on module B\[i\]. `stream` is c\[0\] …
    /// c\[N − 1\] of chapter 4 (4.8.2).
    ///
    /// # Errors
    ///
    /// [`SymbolError::StreamLength`] when `stream` does not have exactly N bytes,
    /// [`SymbolError::FormatCodewordTooWide`] when a format word has a bit above bit 46, and
    /// [`SymbolError::TooLarge`] when the grid cannot be allocated.
    pub fn draw_copies(&self, format: [u64; 2], stream: &[u8]) -> Result<ModuleGrid, SymbolError> {
        if stream.len() != self.counts.codewords {
            return Err(SymbolError::StreamLength {
                expected: self.counts.codewords,
                actual: stream.len(),
            });
        }
        for word in format {
            check_codeword(word)?;
        }
        let mut grid = ModuleGrid::new(self.width, self.height).ok_or(SymbolError::TooLarge)?;
        self.draw_function_patterns(&mut grid);
        write_format_copies(&mut grid, format)?;

        let mut positions = self.placement();
        let mut whitening = Whitening::new();
        for &codeword in stream {
            let value = codeword ^ whitening.next_byte();
            for shift in (0..8).rev() {
                let Some((x, y)) = positions.next() else {
                    return Ok(grid);
                };
                if value >> shift & 1 == 1 {
                    grid.set(x, y, true);
                }
            }
        }
        // Remainder modules: b = 0, so the module value is the whitening bit (5.9, 5.10).
        for (x, y) in positions {
            if whitening.next_bit() {
                grid.set(x, y, true);
            }
        }
        Ok(grid)
    }

    /// Draws the finders and reference marks on `grid`; separators stay light (5.3, 5.4, 5.6).
    fn draw_function_patterns(&self, grid: &mut ModuleGrid) {
        for corner in Corner::ALL {
            let Some((ox, oy)) = corner.origin(self.width, self.height) else {
                continue;
            };
            for (dy, row) in (0..).zip(corner.pattern()) {
                for (dx, dark) in (0..).zip(row) {
                    if dark {
                        grid.set(ox + dx, oy + dy, true);
                    }
                }
            }
        }
        let half = REFERENCE_MARK_SIZE / 2;
        for (cx, cy) in self.reference_mark_centres() {
            for y in cy - half..=cy + half {
                for x in cx - half..=cx + half {
                    if (x, y) != (cx, cy) {
                        grid.set(x, y, true);
                    }
                }
            }
        }
    }

    /// Reads the codeword stream c\[0\] … c\[N − 1\] from `grid` (5.9): the value of P\[k\],
    /// combined by exclusive or with w\[k\], is bit b\[k\], and each eight bits make one codeword, most significant
    /// first. The remainder modules are ignored.
    ///
    /// # Errors
    ///
    /// [`SymbolError::GridSizeMismatch`] when `grid` is not W × H.
    pub fn read_stream(&self, grid: &ModuleGrid) -> Result<Vec<u8>, SymbolError> {
        if grid.width() != self.width || grid.height() != self.height {
            return Err(SymbolError::GridSizeMismatch {
                expected_width: self.width,
                expected_height: self.height,
                width: grid.width(),
                height: grid.height(),
            });
        }
        let mut positions = self.placement();
        let mut whitening = Whitening::new();
        let mut stream = Vec::with_capacity(self.counts.codewords);
        for _ in 0..self.counts.codewords {
            let mut value = 0u8;
            for _ in 0..8 {
                let dark = positions.next().is_some_and(|(x, y)| grid.get(x, y) == Some(true));
                value = value << 1 | u8::from(dark);
            }
            stream.push(value ^ whitening.next_byte());
        }
        Ok(stream)
    }
}

/// Every (xline\[i\], yline\[j\]) except the four finder centres (5.6.1), by y line and then by
/// x line. Both slices hold at least two lines.
fn mark_centres<'a>(
    x_lines: &'a [u32],
    y_lines: &'a [u32],
) -> impl Iterator<Item = (u32, u32)> + 'a {
    let last_x = x_lines.len().saturating_sub(1);
    let last_y = y_lines.len().saturating_sub(1);
    y_lines.iter().enumerate().flat_map(move |(j, &y)| {
        x_lines.iter().enumerate().filter_map(move |(i, &x)| {
            let finder_centre = (i == 0 || i == last_x) && (j == 0 || j == last_y);
            (!finder_centre).then_some((x, y))
        })
    })
}

/// Calls `f` once for every function module of a `width` × `height` symbol: the four 6 × 6
/// corner blocks of finder and separator (5.3, 5.4), both format copies (5.5) and every
/// reference mark (5.6). Both sides are valid symbol sides.
fn for_each_function_module(
    width: u32,
    height: u32,
    x_lines: &[u32],
    y_lines: &[u32],
    mut f: impl FnMut(u32, u32),
) {
    let (right, bottom) = (width - CORNER_BLOCK, height - CORNER_BLOCK);
    for (x0, y0) in [(0, 0), (right, 0), (0, bottom), (right, bottom)] {
        for y in y0..y0 + CORNER_BLOCK {
            for x in x0..x0 + CORNER_BLOCK {
                f(x, y);
            }
        }
    }
    for (x, y) in FORMAT_COPY_A {
        f(x, y);
        f(width - 1 - x, height - 1 - y);
    }
    let half = REFERENCE_MARK_SIZE / 2;
    for (cx, cy) in mark_centres(x_lines, y_lines) {
        for y in cy - half..=cy + half {
            for x in cx - half..=cx + half {
                f(x, y);
            }
        }
    }
}

/// `near[z]` for z < `side`: z is within one module of one of `lines`.
fn near_lines(side: u32, lines: &[u32]) -> Result<Vec<bool>, SymbolError> {
    let len = usize::try_from(side).map_err(|_| SymbolError::TooLarge)?;
    let mut near = vec![false; len];
    for &line in lines {
        for z in line - 1..=line + 1 {
            if let Some(slot) = usize::try_from(z).ok().and_then(|z| near.get_mut(z)) {
                *slot = true;
            }
        }
    }
    Ok(near)
}

/// The data modules of a [`Layout`] in placement order (5.8), made by [`Layout::placement`].
#[derive(Debug, Clone)]
pub struct Placement<'a> {
    walk: &'a [u64],
    words_per_pair: u32,
    height: u32,
    /// Index of the current word in `walk`.
    word: usize,
    /// Column pair of the current word.
    pair: u32,
    /// Index of the current word within its column pair.
    in_pair: u32,
    /// Bits of the current word not yet yielded.
    bits: u64,
    remaining: usize,
}

impl Iterator for Placement<'_> {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<(u32, u32)> {
        while self.bits == 0 {
            let next = self.word + 1;
            self.bits = *self.walk.get(next)?;
            self.word = next;
            self.in_pair += 1;
            if self.in_pair == self.words_per_pair {
                self.in_pair = 0;
                self.pair += 1;
            }
        }
        let bit = self.in_pair * 64 + self.bits.trailing_zeros();
        self.bits &= self.bits - 1;
        self.remaining = self.remaining.saturating_sub(1);
        let step = bit / 2;
        let x = 2 * self.pair + (bit & 1);
        let y =
            if self.pair.is_multiple_of(2) { step } else { (self.height - 1).saturating_sub(step) };
        Some((x, y))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for Placement<'_> {}

impl FusedIterator for Placement<'_> {}

// Both format copies have `FORMAT_BITS` modules; the fixed count above assumes 47.
const _: () = assert!(FORMAT_BITS == 47);
