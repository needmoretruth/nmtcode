//! The properties of the finder patterns that 5.3.2 promises, checked on the drawn symbol.

use nmtcode_symbol::{Corner, Layout, ModuleClass, ModuleGrid};

/// The eight rotations and mirror images of a 3 × 3 array.
fn transforms(a: [[bool; 3]; 3]) -> Vec<[[bool; 3]; 3]> {
    let rotate = |m: [[bool; 3]; 3]| {
        let mut out = [[false; 3]; 3];
        for (y, row) in out.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                *cell = m[2 - x][y];
            }
        }
        out
    };
    let mirror = |m: [[bool; 3]; 3]| m.map(|row| [row[2], row[1], row[0]]);
    let mut all = Vec::new();
    for start in [a, mirror(a)] {
        let mut m = start;
        for _ in 0..4 {
            all.push(m);
            m = rotate(m);
        }
    }
    all
}

/// The four pictures of 5.3.1, row by row, `#` dark.
#[test]
fn finder_pictures_of_5_3_1() {
    let pictures = [
        (Corner::TopLeft, ["#####", "#oo##", "#oo##", "#####", "#####"]),
        (Corner::TopRight, ["#####", "##oo#", "#####", "##oo#", "#####"]),
        (Corner::BottomLeft, ["#####", "#####", "#####", "#####", "#####"]),
        (Corner::BottomRight, ["#####", "##oo#", "#ooo#", "#ooo#", "#####"]),
    ];
    for (corner, rows) in pictures {
        let drawn: Vec<String> = corner
            .pattern()
            .iter()
            .map(|row| row.iter().map(|&d| if d { '#' } else { 'o' }).collect())
            .collect();
        assert_eq!(drawn, rows, "{corner:?}");
    }
}

/// 5.3.2 property 1: the smallest distance between two different finders, over the eight
/// transforms, is exactly 4.
#[test]
fn smallest_distance_between_finders_is_four() {
    let mut smallest = 9;
    for f in Corner::ALL {
        for g in Corner::ALL {
            if f != g {
                for t in transforms(g.inner()) {
                    let d = (0..9).filter(|&i| f.inner()[i / 3][i % 3] != t[i / 3][i % 3]).count();
                    smallest = smallest.min(d);
                }
            }
        }
    }
    assert_eq!(smallest, 4);
}

/// 5.3.2 property 1: two different finders differ in at least 4 of 9 inner modules under every
/// transform.
#[test]
fn inner_patterns_differ_in_four_modules_under_every_transform() {
    for f in Corner::ALL {
        for g in Corner::ALL {
            if f == g {
                continue;
            }
            for t in transforms(g.inner()) {
                let distance =
                    (0..9).filter(|&i| f.inner()[i / 3][i % 3] != t[i / 3][i % 3]).count();
                assert!(distance >= 4, "{f:?} against {g:?}: {distance}");
            }
        }
    }
}

/// 5.3.1: each finder is a dark ring around its inner 3 × 3, drawn in its corner.
#[test]
fn finders_are_drawn_in_their_corners() {
    let layout = Layout::new(28, 40).unwrap();
    let grid = layout.draw_copies([0, 0], &vec![0; layout.codeword_count()]).unwrap();
    for corner in Corner::ALL {
        let (ox, oy) = corner.origin(28, 40).unwrap();
        for dy in 0..5 {
            for dx in 0..5 {
                let ring = dx == 0 || dy == 0 || dx == 4 || dy == 4;
                let expected = ring || corner.inner()[dy as usize - 1][dx as usize - 1];
                assert_eq!(grid.get(ox + dx, oy + dy), Some(expected), "{corner:?} ({dx}, {dy})");
                assert_eq!(layout.module_class(ox + dx, oy + dy), Some(ModuleClass::Finder));
            }
        }
    }
    assert_eq!(Corner::TopLeft.centre(28, 40), Some((2, 2)));
    assert_eq!(Corner::BottomRight.centre(28, 40), Some((25, 37)));
}

/// A module of a line through a finder, with a quiet zone added at both ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// A finder module with its value.
    Finder(bool),
    /// A separator or quiet-zone module: always light.
    Light,
    /// A data, format or reference-mark module: any value.
    Free,
}

/// Every row and every column that crosses a finder, as cells, with `quiet` quiet-zone modules
/// at each end.
fn lines_through_finders(layout: &Layout, quiet: usize) -> Vec<Vec<Cell>> {
    let (w, h) = (layout.width(), layout.height());
    let cell = |x: u32, y: u32| match layout.module_class(x, y).unwrap() {
        ModuleClass::Finder => Cell::Finder(layout.function_value(x, y).unwrap()),
        ModuleClass::Separator => Cell::Light,
        _ => Cell::Free,
    };
    let wrap = |inner: Vec<Cell>| {
        let mut line = vec![Cell::Light; quiet];
        line.extend(inner);
        line.extend(vec![Cell::Light; quiet]);
        line
    };
    let mut lines = Vec::new();
    for y in (0..5).chain(h - 5..h) {
        lines.push(wrap((0..w).map(|x| cell(x, y)).collect()));
    }
    for x in (0..5).chain(w - 5..w) {
        lines.push(wrap((0..h).map(|y| cell(x, y)).collect()));
    }
    lines
}

fn is_1_1_3_1_1(runs: &[usize]) -> bool {
    let &[first, second, middle, fourth, fifth] = runs else {
        return false;
    };
    first == second && second == fourth && fourth == fifth && middle == 3 * first
}

/// 5.3.2 property 2, first part: every dark run that contains a finder module is bounded by
/// light function modules and has length 1, 2 or 5, so it is never the 3u middle of a
/// 1:1:3:1:1 sequence, whatever the data and format modules hold.
#[test]
fn dark_finder_runs_are_closed_and_never_3u() {
    for (w, h) in [(20, 20), (24, 24), (20, 28), (100, 48), (48, 100), (4108, 20)] {
        let layout = Layout::new(w, h).unwrap();
        for line in lines_through_finders(&layout, 2) {
            let mut i = 0;
            while i < line.len() {
                if line[i] != Cell::Finder(true) {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < line.len() && line[i] == Cell::Finder(true) {
                    i += 1;
                }
                let before = start.checked_sub(1).map(|j| line[j]);
                let after = line.get(i).copied();
                let closes = |c: Option<Cell>| matches!(c, Some(Cell::Light | Cell::Finder(false)));
                assert!(closes(before) && closes(after), "{w} x {h}: open dark run at {start}");
                let len = i - start;
                assert!([1, 2, 5].contains(&len), "{w} x {h}: dark finder run of {len}");
            }
        }
    }
}

/// 5.3.2 property 2, second part: no five consecutive runs in the ratio 1:1:3:1:1, of either
/// colour order, consist only of finder, separator and quiet-zone modules. The quiet zone is
/// any width from 2; beyond it anything may follow.
#[test]
fn no_1_1_3_1_1_of_function_modules_alone() {
    for (w, h) in [(20, 20), (24, 24), (20, 28), (100, 48), (48, 100)] {
        let layout = Layout::new(w, h).unwrap();
        for quiet in 2..=12 {
            for line in lines_through_finders(&layout, quiet) {
                // Stretches of function modules between free modules; the line's ends are free.
                for stretch in line.split(|&c| c == Cell::Free) {
                    let mut runs: Vec<usize> = Vec::new();
                    let mut last = None;
                    for &c in stretch {
                        let dark = c == Cell::Finder(true);
                        if last == Some(dark) {
                            *runs.last_mut().unwrap() += 1;
                        } else {
                            runs.push(1);
                            last = Some(dark);
                        }
                    }
                    for window in runs.windows(5) {
                        assert!(!is_1_1_3_1_1(window), "{w} x {h}, quiet {quiet}: {window:?}");
                    }
                }
            }
        }
    }
}

/// 5.3.2 property 2 on drawn symbols: in every row and column through a finder, with a quiet
/// zone of 2, no dark-first u, u, 3u, u, u sequence has its middle run on a finder module.
#[test]
fn drawn_symbols_have_no_qr_signature_on_a_finder() {
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    for (w, h) in [(20, 20), (24, 24), (28, 20), (52, 52), (100, 48)] {
        let layout = Layout::new(w, h).unwrap();
        for format in [0, (1 << 47) - 1, 0x51F3_694E_AFAA] {
            for fill in 0..40u32 {
                let stream: Vec<u8> = (0..layout.codeword_count())
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        match fill {
                            0 => 0,
                            1 => 0xFF,
                            _ => seed.to_le_bytes()[0],
                        }
                    })
                    .collect();
                let grid =
                    layout.draw_copies([format, !format & ((1 << 47) - 1)], &stream).unwrap();
                check_lines(&layout, &grid);
            }
        }
    }
}

fn check_lines(layout: &Layout, grid: &ModuleGrid) {
    let (w, h) = (layout.width(), layout.height());
    let mut lines: Vec<Vec<(bool, bool)>> = Vec::new();
    let module = |x: u32, y: u32| {
        (grid.get(x, y).unwrap(), layout.module_class(x, y) == Some(ModuleClass::Finder))
    };
    for y in (0..5).chain(h - 5..h) {
        lines.push((0..w).map(|x| module(x, y)).collect());
    }
    for x in (0..5).chain(w - 5..w) {
        lines.push((0..h).map(|y| module(x, y)).collect());
    }
    for inner in lines {
        let mut line = vec![(false, false); 2];
        line.extend(inner);
        line.extend([(false, false); 2]);
        // Runs as (dark, length, touches a finder module).
        let mut runs: Vec<(bool, usize, bool)> = Vec::new();
        for (dark, finder) in line {
            match runs.last_mut() {
                Some(run) if run.0 == dark => {
                    run.1 += 1;
                    run.2 |= finder;
                }
                _ => runs.push((dark, 1, finder)),
            }
        }
        for window in runs.windows(5) {
            let lengths: Vec<usize> = window.iter().map(|r| r.1).collect();
            let dark_first = window[0].0;
            assert!(
                !(dark_first && window[2].2 && is_1_1_3_1_1(&lengths)),
                "{w} x {h}: {window:?}"
            );
        }
    }
}
