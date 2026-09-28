//! Canvas and bootstrap placement of specification 8.4.

use nmtcode_core::is_valid_side;

use crate::bootstrap::smallest_version;
use crate::{BootstrapLevel, BootstrapSide, MIN_QUIET_ZONE, RenderError, RenderOptions};

/// Smallest bootstrap QR module when the physical size is known (`qr_min_module`, 8.4.1), in
/// micrometres.
const QR_MIN_MODULE_UM: u64 = 400;
/// Smallest bootstrap QR module on a screen of unknown density (`qr_min_module`, 8.4.1), in
/// pixels.
const QR_MIN_MODULE_PX: u64 = 4;
/// Quiet zone of the QR Code in QR modules (8.3).
const QR_QUIET_ZONE: i64 = 4;

/// Largest canvas side in modules.
const MAX_CANVAS_SIDE: i64 = 1 << 20;
/// Largest canvas area in modules.
const MAX_CANVAS_MODULES: i64 = 1 << 26;
/// Largest image side in pixels (PNG allows 2^31 − 1).
const MAX_PIXEL_SIDE: u64 = (1 << 31) - 1;
/// Largest image area in pixels.
const MAX_PIXELS: u64 = 1 << 32;

/// Where everything goes on the canvas, in the module coordinates of specification 1.4: the
/// origin is the symbol's top-left module, quiet zone excluded; x grows right, y grows down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Symbol width W in modules.
    pub symbol_width: u32,
    /// Symbol height H in modules.
    pub symbol_height: u32,
    /// Quiet zone Q in modules.
    pub quiet_zone: u32,
    /// Module side in pixels.
    pub module_px: u32,
    /// Left edge of the canvas (inclusive), in modules. Negative: the canvas starts left of the
    /// symbol.
    pub canvas_x: i64,
    /// Top edge of the canvas (inclusive), in modules.
    pub canvas_y: i64,
    /// Canvas width in modules.
    pub canvas_width: u32,
    /// Canvas height in modules.
    pub canvas_height: u32,
    /// The bootstrap QR Code, when it is drawn.
    pub bootstrap: Option<BootstrapLayout>,
}

/// Placement of the bootstrap QR Code (specification 8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapLayout {
    /// Side of the NMT Code symbol it sits on.
    pub side: BootstrapSide,
    /// Error-correction level.
    pub level: BootstrapLevel,
    /// QR version v (8.3).
    pub version: u8,
    /// Width n of one QR module in NMT Code modules (8.4.1).
    pub scale: u32,
    /// Gap G between the facing edges of the two symbols, in NMT Code modules (8.4.2).
    pub gap: u32,
    /// Side S = (17 + 4v) · n of the QR symbol in NMT Code modules, quiet zone excluded.
    pub size: u32,
    /// Left edge of the QR symbol, quiet zone excluded, in NMT Code modules.
    pub x: i64,
    /// Top edge of the QR symbol, quiet zone excluded, in NMT Code modules.
    pub y: i64,
}

impl Layout {
    /// Canvas width in pixels.
    pub fn pixel_width(&self) -> u64 {
        u64::from(self.canvas_width) * u64::from(self.module_px)
    }

    /// Canvas height in pixels.
    pub fn pixel_height(&self) -> u64 {
        u64::from(self.canvas_height) * u64::from(self.module_px)
    }

    /// Position of the symbol's module (0, 0) on the canvas, in modules from the canvas's
    /// top-left corner.
    pub fn symbol_offset(&self) -> (u32, u32) {
        // `canvas_x` and `canvas_y` are at most 0 and above −2^20 (checked in `layout`).
        let x = u32::try_from(-self.canvas_x).unwrap_or(0);
        let y = u32::try_from(-self.canvas_y).unwrap_or(0);
        (x, y)
    }
}

/// The smallest n of 8.4.1: the least integer ≥ 1 with n · s ≥ `s_min`.
fn min_scale(module_px: u32, dpi: Option<u32>) -> u32 {
    let px = u64::from(module_px.max(1));
    let n = match dpi {
        // n · px · 25 400 / dpi µm ≥ 400 µm.
        Some(dpi) => (QR_MIN_MODULE_UM * u64::from(dpi)).div_ceil(px * 25_400),
        None => QR_MIN_MODULE_PX.div_ceil(px),
    };
    u32::try_from(n.max(1)).unwrap_or(u32::MAX)
}

/// Computes the canvas of a `width` × `height` symbol drawn with `options`: its extent and, when
/// the bootstrap QR Code is on, the QR placement of specification 8.4 (module size 8.4.1, gap
/// 8.4.2, position and canvas 8.4.3).
///
/// # Errors
///
/// [`RenderError`] when the size is not a valid symbol size, an option is out of range, or the
/// canvas would be too large.
pub fn layout(width: u32, height: u32, options: &RenderOptions) -> Result<Layout, RenderError> {
    if !is_valid_side(width) || !is_valid_side(height) {
        return Err(RenderError::InvalidSymbolSize { width, height });
    }
    if options.module_px == 0 {
        return Err(RenderError::ModuleSizeZero);
    }
    if options.quiet_zone < MIN_QUIET_ZONE {
        return Err(RenderError::QuietZoneTooSmall { quiet_zone: options.quiet_zone });
    }
    if options.dpi == Some(0) {
        return Err(RenderError::InvalidDpi);
    }
    let w = i64::from(width);
    let h = i64::from(height);
    let q = i64::from(options.quiet_zone);

    let (x0, x1, y0, y1, bootstrap) = if options.bootstrap {
        let level = options.bootstrap_level;
        let version = smallest_version(level)?;
        let minimum = min_scale(options.module_px, options.dpi);
        let scale = match options.bootstrap_scale {
            None => minimum,
            Some(requested) if requested >= minimum => requested,
            Some(requested) => {
                return Err(RenderError::BootstrapScaleTooSmall { requested, minimum });
            }
        };
        let n = i64::from(scale);
        // G = n · max(4, ceil(Q / n)) (8.4.2).
        let gap = n * QR_QUIET_ZONE.max((q + n - 1) / n);
        // S = (17 + 4v) · n (8.4.1).
        let qr_side = (17 + 4 * i64::from(version)) * n;
        let qr_quiet = QR_QUIET_ZONE * n;
        let side = options.bootstrap_side.unwrap_or(if width >= height {
            BootstrapSide::Left
        } else {
            BootstrapSide::Above
        });
        // 8.4.3, both tables.
        let (qx, qy, x0, x1, y0, y1) = match side {
            BootstrapSide::Left => (
                -gap - qr_side,
                0,
                -(gap + qr_side + qr_quiet),
                w + q,
                -q.max(qr_quiet),
                (h + q).max(qr_side + qr_quiet),
            ),
            BootstrapSide::Above => (
                0,
                -gap - qr_side,
                -q.max(qr_quiet),
                (w + q).max(qr_side + qr_quiet),
                -(gap + qr_side + qr_quiet),
                h + q,
            ),
        };
        let too_large = |v: i64| u32::try_from(v).map_err(|_| RenderError::TooLarge);
        let placement = BootstrapLayout {
            side,
            level,
            version,
            scale,
            gap: too_large(gap)?,
            size: too_large(qr_side)?,
            x: qx,
            y: qy,
        };
        (x0, x1, y0, y1, Some(placement))
    } else {
        (-q, w + q, -q, h + q, None)
    };

    let cw = x1 - x0;
    let ch = y1 - y0;
    if cw > MAX_CANVAS_SIDE || ch > MAX_CANVAS_SIDE || cw * ch > MAX_CANVAS_MODULES {
        return Err(RenderError::TooLarge);
    }
    let canvas_width = u32::try_from(cw).map_err(|_| RenderError::TooLarge)?;
    let canvas_height = u32::try_from(ch).map_err(|_| RenderError::TooLarge)?;
    let px = u64::from(options.module_px);
    let (pw, ph) = (u64::from(canvas_width) * px, u64::from(canvas_height) * px);
    if pw > MAX_PIXEL_SIDE || ph > MAX_PIXEL_SIDE || pw.saturating_mul(ph) > MAX_PIXELS {
        return Err(RenderError::TooLarge);
    }
    Ok(Layout {
        symbol_width: width,
        symbol_height: height,
        quiet_zone: options.quiet_zone,
        module_px: options.module_px,
        canvas_x: x0,
        canvas_y: y0,
        canvas_width,
        canvas_height,
        bootstrap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_scale_of_8_4_1() {
        assert_eq!(min_scale(4, None), 1);
        assert_eq!(min_scale(3, None), 2);
        assert_eq!(min_scale(2, None), 2);
        assert_eq!(min_scale(1, None), 4);
        // 0.25 mm modules (10 px at 1016 dpi): n = 2 as in 8.8.2.
        assert_eq!(min_scale(10, Some(1016)), 2);
        // 0.4 mm exactly: n = 1.
        assert_eq!(min_scale(16, Some(1016)), 1);
        // 5 dots at 300 dpi = 0.423 mm: n = 1.
        assert_eq!(min_scale(5, Some(300)), 1);
    }
}
