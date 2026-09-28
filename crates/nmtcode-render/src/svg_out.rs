//! SVG output: one white background rectangle and one black path of merged rectangles.

use core::fmt::Write;

use nmtcode_core::ModuleGrid;

use crate::RenderOptions;
use crate::canvas::Canvas;

/// A rectangle of dark modules in canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rect {
    pub y: u32,
    pub x: u32,
    pub width: u32,
    pub height: u32,
}

/// Merges the dark modules into rectangles: runs within a row first, then runs of the same
/// position and length in consecutive rows. Every dark module is covered by exactly one
/// rectangle and no light module is covered.
pub(crate) fn merge_rects(modules: &ModuleGrid) -> Vec<Rect> {
    let mut done = Vec::new();
    // Open rectangles: (x, width, first row), sorted by x.
    let mut open: Vec<(u32, u32, u32)> = Vec::new();
    let mut next: Vec<(u32, u32, u32)> = Vec::new();
    for y in 0..modules.height() {
        next.clear();
        let mut prev = open.iter().peekable();
        let mut x = 0;
        while x < modules.width() {
            if modules.get(x, y) != Some(true) {
                x += 1;
                continue;
            }
            let start = x;
            while x < modules.width() && modules.get(x, y) == Some(true) {
                x += 1;
            }
            let width = x - start;
            // Close every open rectangle that starts left of this run.
            let mut first_row = y;
            while let Some(&&(ox, ow, oy)) = prev.peek() {
                if ox < start || (ox == start && ow != width) {
                    done.push(Rect { y: oy, x: ox, width: ow, height: y - oy });
                    prev.next();
                } else {
                    if ox == start && ow == width {
                        first_row = oy;
                        prev.next();
                    }
                    break;
                }
            }
            next.push((start, width, first_row));
        }
        for &(ox, ow, oy) in prev {
            done.push(Rect { y: oy, x: ox, width: ow, height: y - oy });
        }
        core::mem::swap(&mut open, &mut next);
    }
    let end = modules.height();
    for &(ox, ow, oy) in &open {
        done.push(Rect { y: oy, x: ox, width: ow, height: end - oy });
    }
    done.sort_unstable();
    done
}

/// `value` / 10 000 as a decimal with at most four places and no trailing zeros.
fn decimal_e4(value: u64) -> String {
    let int = value / 10_000;
    let frac = value % 10_000;
    if frac == 0 {
        return int.to_string();
    }
    let mut text = format!("{int}.{frac:04}");
    while text.ends_with('0') {
        text.pop();
    }
    text
}

/// Physical length in millimetres of `px` pixels at `dpi`, as an SVG length.
fn millimetres(px: u64, dpi: u32) -> String {
    // px · 25.4 / dpi mm, in units of 10^-4 mm, rounded.
    let dpi = u64::from(dpi.max(1));
    let e4 = (u128::from(px) * 254_000 + u128::from(dpi / 2)) / u128::from(dpi);
    let e4 = u64::try_from(e4).unwrap_or(u64::MAX);
    format!("{}mm", decimal_e4(e4))
}

pub(crate) fn write(canvas: &Canvas, options: &RenderOptions) -> String {
    let layout = &canvas.layout;
    let (cw, ch) = (layout.canvas_width, layout.canvas_height);
    let (width, height) = match options.dpi {
        Some(dpi) => {
            (millimetres(layout.pixel_width(), dpi), millimetres(layout.pixel_height(), dpi))
        }
        None => (layout.pixel_width().to_string(), layout.pixel_height().to_string()),
    };
    let rects = merge_rects(&canvas.modules);
    let mut svg = String::with_capacity(400 + rects.len() * 24);
    // `write!` into a String cannot fail.
    let _ = writeln!(svg, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{width}" height="{height}" viewBox="0 0 {cw} {ch}" shape-rendering="crispEdges">"#
    );
    let _ = writeln!(
        svg,
        "<title>NMT Code, {} × {} modules</title>",
        layout.symbol_width, layout.symbol_height
    );
    let _ = writeln!(svg, r##"<rect width="{cw}" height="{ch}" fill="#ffffff"/>"##);
    if !rects.is_empty() {
        svg.push_str(r##"<path fill="#000000" d=""##);
        for r in &rects {
            let _ = write!(svg, "M{} {}h{}v{}h-{}z", r.x, r.y, r.width, r.height, r.width);
        }
        svg.push_str("\"/>\n");
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals() {
        assert_eq!(decimal_e4(390_000), "39");
        assert_eq!(decimal_e4(225_000), "22.5");
        assert_eq!(decimal_e4(4_233), "0.4233");
        assert_eq!(millimetres(1560, 1016), "39mm");
        assert_eq!(millimetres(900, 1016), "22.5mm");
        assert_eq!(millimetres(10, 600), "0.4233mm");
    }

    #[test]
    fn merge_covers_exactly() {
        let rows = ["##..##", "##..##", "#....#", "######", "......", "..##.."];
        let grid = ModuleGrid::from_rows(&rows).unwrap();
        let rects = merge_rects(&grid);
        let mut seen = ModuleGrid::new(6, 6).unwrap();
        for r in &rects {
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    assert_eq!(seen.get(x, y), Some(false), "overlap at ({x}, {y})");
                    seen.set(x, y, true);
                }
            }
        }
        assert_eq!(seen, grid);
        // "##" at x = 0 and x = 4 merge over rows 0 and 1.
        assert!(rects.contains(&Rect { y: 0, x: 0, width: 2, height: 2 }));
        assert!(rects.contains(&Rect { y: 0, x: 4, width: 2, height: 2 }));
        assert_eq!(rects.len(), 6);
    }
}
