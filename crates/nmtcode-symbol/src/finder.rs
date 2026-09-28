//! The four corner finder patterns (5.3).

use crate::FINDER_SIZE;

/// A corner of the symbol, and the finder pattern that sits there (5.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Corner {
    /// Top left: a 2 × 2 light hole in the corner of the inner area nearest the symbol's corner.
    TopLeft,
    /// Top right: two 2 × 1 light slots against the ring's outer side.
    TopRight,
    /// Bottom left: all dark, a solid 5 × 5 square.
    BottomLeft,
    /// Bottom right: a 3 × 3 light area with one dark module in the corner nearest the symbol's
    /// centre.
    BottomRight,
}

/// Turns the picture of 5.3.1 (`#` dark, `o` light) into module values.
const fn parse(rows: [&[u8; 5]; 5]) -> [[bool; 5]; 5] {
    let mut out = [[false; 5]; 5];
    let mut y = 0;
    while y < 5 {
        let mut x = 0;
        while x < 5 {
            out[y][x] = rows[y][x] == b'#';
            x += 1;
        }
        y += 1;
    }
    out
}

// The four patterns exactly as drawn in 5.3.1: left column is the finder's smallest x, top row
// its smallest y.
const TOP_LEFT: [[bool; 5]; 5] = parse([b"#####", b"#oo##", b"#oo##", b"#####", b"#####"]);
const TOP_RIGHT: [[bool; 5]; 5] = parse([b"#####", b"##oo#", b"#####", b"##oo#", b"#####"]);
const BOTTOM_LEFT: [[bool; 5]; 5] = parse([b"#####", b"#####", b"#####", b"#####", b"#####"]);
const BOTTOM_RIGHT: [[bool; 5]; 5] = parse([b"#####", b"##oo#", b"#ooo#", b"#ooo#", b"#####"]);

impl Corner {
    /// All four corners in the order top left, top right, bottom left, bottom right.
    pub const ALL: [Self; 4] = [Self::TopLeft, Self::TopRight, Self::BottomLeft, Self::BottomRight];

    /// The finder's 5 × 5 pattern as it appears in the upright symbol, `true` for dark.
    ///
    /// `pattern()[dy][dx]` is the module at (`dx`, `dy`) relative to the finder's top-left
    /// module: `dx = 0` is the finder's smallest x and `dy = 0` its smallest y.
    pub const fn pattern(self) -> [[bool; 5]; 5] {
        match self {
            Self::TopLeft => TOP_LEFT,
            Self::TopRight => TOP_RIGHT,
            Self::BottomLeft => BOTTOM_LEFT,
            Self::BottomRight => BOTTOM_RIGHT,
        }
    }

    /// The inner 3 × 3 of the pattern, without the dark ring: `inner()[dy][dx]` is the module
    /// at (`dx + 1`, `dy + 1`) of [`Corner::pattern`].
    pub const fn inner(self) -> [[bool; 3]; 3] {
        let p = self.pattern();
        [[p[1][1], p[1][2], p[1][3]], [p[2][1], p[2][2], p[2][3]], [p[3][1], p[3][2], p[3][3]]]
    }

    /// The module at (`dx`, `dy`) relative to the finder's top-left module: `Some(true)` for
    /// dark, `None` outside the 5 × 5 pattern.
    pub fn module(self, dx: u32, dy: u32) -> Option<bool> {
        let row = self.pattern().get(usize::try_from(dy).ok()?).copied()?;
        row.get(usize::try_from(dx).ok()?).copied()
    }

    /// The top-left module of this corner's finder in a `width` × `height` symbol (5.3.1), or
    /// `None` when a side is shorter than the finder.
    pub fn origin(self, width: u32, height: u32) -> Option<(u32, u32)> {
        let right = width.checked_sub(FINDER_SIZE)?;
        let bottom = height.checked_sub(FINDER_SIZE)?;
        Some(match self {
            Self::TopLeft => (0, 0),
            Self::TopRight => (right, 0),
            Self::BottomLeft => (0, bottom),
            Self::BottomRight => (right, bottom),
        })
    }

    /// The centre module of this corner's finder in a `width` × `height` symbol: (2, 2),
    /// (W − 3, 2), (2, H − 3) or (W − 3, H − 3) (5.6.1). `None` when a side is shorter than
    /// the finder.
    pub fn centre(self, width: u32, height: u32) -> Option<(u32, u32)> {
        let (x, y) = self.origin(width, height)?;
        Some((x + FINDER_SIZE / 2, y + FINDER_SIZE / 2))
    }
}
