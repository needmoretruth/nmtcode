//! The grid of modules of one symbol: the value every layer above and below agrees on.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Smallest side of a symbol in modules (specification, chapter 1, 1.3).
pub const MIN_SIDE: u32 = 20;
/// Largest side of a symbol in modules (specification, chapter 1, 1.3).
pub const MAX_SIDE: u32 = 4108;

/// A `width` × `height` grid of modules, quiet zone excluded.
///
/// Coordinates follow the specification (chapter 1, 1.4): the origin is the top-left module,
/// `x` grows to the right and `y` grows downward. A module is dark (`true`, value 1) or light
/// (`false`, value 0). The grid holds any size from 1 × 1 so that tests and readers can build
/// partial grids; whether a size is a valid symbol size is decided by [`is_valid_side`].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ModuleGrid {
    width: u32,
    height: u32,
    /// Row-major, one bit per module, bit `i % 64` of word `i / 64` for module index `i`.
    words: Vec<u64>,
}

/// Why [`ModuleGrid::from_rows`] refused its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridTextError {
    /// There were no rows, or the first row was empty.
    Empty,
    /// Row `row` has a different length from row 0.
    Ragged {
        /// Index of the offending row.
        row: usize,
    },
    /// A character other than `#` (dark) or `.` (light).
    BadChar {
        /// Index of the offending row.
        row: usize,
        /// Index of the offending character within the row.
        column: usize,
    },
    /// The grid is larger than `u32::MAX` modules in one direction.
    TooLarge,
}

/// True when `side` is a valid symbol width or height: a multiple of 4 from 20 to 4108.
pub const fn is_valid_side(side: u32) -> bool {
    side >= MIN_SIDE && side <= MAX_SIDE && side.is_multiple_of(4)
}

impl ModuleGrid {
    /// A grid of `width` × `height` light modules.
    ///
    /// Returns `None` when either side is 0 or the module count does not fit in memory
    /// addressing on this platform.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let count = usize::try_from(u64::from(width) * u64::from(height)).ok()?;
        Some(Self { width, height, words: vec![0; count.div_ceil(64)] })
    }

    /// Width in modules.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in modules.
    pub fn height(&self) -> u32 {
        self.height
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()
    }

    /// The module at (`x`, `y`): `Some(true)` for dark, `Some(false)` for light, `None` outside
    /// the grid.
    pub fn get(&self, x: u32, y: u32) -> Option<bool> {
        let i = self.index(x, y)?;
        Some(self.words[i / 64] >> (i % 64) & 1 == 1)
    }

    /// Sets the module at (`x`, `y`). Returns `false`, and changes nothing, outside the grid.
    pub fn set(&mut self, x: u32, y: u32, dark: bool) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        let bit = 1u64 << (i % 64);
        if dark {
            self.words[i / 64] |= bit;
        } else {
            self.words[i / 64] &= !bit;
        }
        true
    }

    /// Flips the module at (`x`, `y`). Returns `false`, and changes nothing, outside the grid.
    pub fn toggle(&mut self, x: u32, y: u32) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        self.words[i / 64] ^= 1u64 << (i % 64);
        true
    }

    /// Number of dark modules.
    pub fn dark_count(&self) -> u64 {
        self.words.iter().map(|w| u64::from(w.count_ones())).sum()
    }

    /// Parses the text form used by the specification's module maps: one string per row,
    /// `#` for dark and `.` for light.
    ///
    /// # Errors
    ///
    /// [`GridTextError`] when the rows are empty, of unequal length, contain another character,
    /// or are too many.
    pub fn from_rows<S: AsRef<str>>(rows: &[S]) -> Result<Self, GridTextError> {
        let first = rows.first().ok_or(GridTextError::Empty)?.as_ref();
        let width = u32::try_from(first.chars().count()).map_err(|_| GridTextError::TooLarge)?;
        let height = u32::try_from(rows.len()).map_err(|_| GridTextError::TooLarge)?;
        let mut grid = Self::new(width, height).ok_or(GridTextError::Empty)?;
        for (y, row) in rows.iter().enumerate() {
            let row = row.as_ref();
            if row.chars().count() != first.chars().count() {
                return Err(GridTextError::Ragged { row: y });
            }
            for (x, c) in row.chars().enumerate() {
                let dark = match c {
                    '#' => true,
                    '.' => false,
                    _ => return Err(GridTextError::BadChar { row: y, column: x }),
                };
                // Both indices are below `width` and `height`, which fit in u32.
                let (Ok(xu), Ok(yu)) = (u32::try_from(x), u32::try_from(y)) else {
                    return Err(GridTextError::TooLarge);
                };
                grid.set(xu, yu, dark);
            }
        }
        Ok(grid)
    }

    /// The text form of [`ModuleGrid::from_rows`].
    pub fn to_rows(&self) -> Vec<String> {
        (0..self.height)
            .map(|y| {
                (0..self.width)
                    .map(|x| if self.get(x, y) == Some(true) { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }
}

impl core::fmt::Debug for ModuleGrid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "ModuleGrid {}x{}", self.width, self.height)?;
        for row in self.to_rows() {
            writeln!(f, "{row}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_toggle() {
        let mut g = ModuleGrid::new(20, 24).unwrap();
        assert_eq!(g.get(19, 23), Some(false));
        assert!(g.set(19, 23, true));
        assert_eq!(g.get(19, 23), Some(true));
        assert!(g.toggle(19, 23));
        assert_eq!(g.get(19, 23), Some(false));
        assert!(!g.set(20, 0, true));
        assert_eq!(g.get(0, 24), None);
        assert_eq!(g.dark_count(), 0);
    }

    #[test]
    fn text_round_trip() {
        let rows = ["#..#", ".##.", "#..."];
        let g = ModuleGrid::from_rows(&rows).unwrap();
        assert_eq!(g.width(), 4);
        assert_eq!(g.height(), 3);
        assert_eq!(g.dark_count(), 5);
        assert_eq!(g.to_rows(), rows);
        assert_eq!(ModuleGrid::from_rows(&["#.", "#"]), Err(GridTextError::Ragged { row: 1 }));
        assert_eq!(
            ModuleGrid::from_rows(&["#x"]),
            Err(GridTextError::BadChar { row: 0, column: 1 })
        );
    }

    #[test]
    fn valid_sides() {
        assert!(is_valid_side(20));
        assert!(is_valid_side(4108));
        assert!(!is_valid_side(16));
        assert!(!is_valid_side(22));
        assert!(!is_valid_side(4112));
    }
}
