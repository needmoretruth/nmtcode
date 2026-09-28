//! The canvas as a module grid: the symbol, its quiet zone and the bootstrap QR Code.

use nmtcode_core::ModuleGrid;

use crate::bootstrap::bootstrap_qr;
use crate::layout::{Layout, layout};
use crate::{RenderError, RenderOptions};

/// A drawn canvas: the [`Layout`] and every canvas module (dark = `true`). Module (0, 0) of
/// [`Canvas::modules`] is the canvas's top-left corner, at [`Layout::canvas_x`],
/// [`Layout::canvas_y`] in symbol coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    /// Where the symbol and the bootstrap QR Code are.
    pub layout: Layout,
    /// All modules of the canvas, `canvas_width` × `canvas_height`.
    pub modules: ModuleGrid,
}

/// Draws `grid` and, when enabled, the bootstrap QR Code into one module grid. Every module
/// outside the two symbols is light (8.4.3).
///
/// # Errors
///
/// The errors of [`layout`], and [`RenderError::Bootstrap`] if the QR encoder fails.
pub fn render_modules(grid: &ModuleGrid, options: &RenderOptions) -> Result<Canvas, RenderError> {
    let layout = layout(grid.width(), grid.height(), options)?;
    let mut modules =
        ModuleGrid::new(layout.canvas_width, layout.canvas_height).ok_or(RenderError::TooLarge)?;
    let (ox, oy) = layout.symbol_offset();
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            if grid.get(x, y) == Some(true) {
                modules.set(ox + x, oy + y, true);
            }
        }
    }
    if let Some(boot) = layout.bootstrap {
        let qr = bootstrap_qr(boot.level)?;
        let n = boot.scale;
        // The QR's top-left corner in canvas modules; inside the canvas by construction (8.4.3).
        let qx = u32::try_from(boot.x - layout.canvas_x).map_err(|_| RenderError::TooLarge)?;
        let qy = u32::try_from(boot.y - layout.canvas_y).map_err(|_| RenderError::TooLarge)?;
        for y in 0..qr.height() {
            for x in 0..qr.width() {
                if qr.get(x, y) != Some(true) {
                    continue;
                }
                for dy in 0..n {
                    for dx in 0..n {
                        modules.set(qx + x * n + dx, qy + y * n + dy, true);
                    }
                }
            }
        }
    }
    Ok(Canvas { layout, modules })
}
