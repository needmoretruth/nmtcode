//! The bootstrap QR Code of specification chapter 8, built with the `qrcode` crate.

use nmtcode_core::ModuleGrid;
use qrcode::QrCode;
use qrcode::bits::Bits;
use qrcode::types::{Color, EcLevel, Version};

use crate::{BootstrapLevel, RenderError};

/// The content of the bootstrap QR Code: the current value (version 0) of the URL registry of
/// specification 8.2, as ASCII bytes.
pub const BOOTSTRAP_URL: &str = "https://github.com/needmoretruth/nmtcode";

/// Largest QR Code Model 2 version.
const MAX_QR_VERSION: u8 = 40;

pub(crate) fn ec_level(level: BootstrapLevel) -> EcLevel {
    match level {
        BootstrapLevel::M => EcLevel::M,
        BootstrapLevel::Q => EcLevel::Q,
        BootstrapLevel::H => EcLevel::H,
    }
}

/// The bit stream of 8.3 for QR version `version`: one byte-mode segment holding the whole URL,
/// no ECI, then the terminator and padding. `None` when the URL does not fit that version.
pub(crate) fn encode_bits(version: u8, level: BootstrapLevel) -> Option<Bits> {
    let mut bits = Bits::new(Version::Normal(i16::from(version)));
    bits.push_byte_data(BOOTSTRAP_URL.as_bytes()).ok()?;
    bits.push_terminator(ec_level(level)).ok()?;
    Some(bits)
}

/// The smallest QR version that holds the URL at `level` (8.3): 3 at M, 4 at Q, 5 at H.
pub(crate) fn smallest_version(level: BootstrapLevel) -> Result<u8, RenderError> {
    (1..=MAX_QR_VERSION)
        .find(|&version| encode_bits(version, level).is_some())
        .ok_or(RenderError::Bootstrap)
}

/// The module matrix of the bootstrap QR Code at `level`, quiet zone excluded, dark = `true`.
///
/// The symbol is QR Code Model 2 at the smallest version that holds [`BOOTSTRAP_URL`] in one
/// byte-mode segment (specification 8.3); the mask is the encoder's choice. The matrix is
/// (17 + 4v) modules square: 29 × 29 at the default level M.
///
/// # Errors
///
/// [`RenderError::Bootstrap`] if the encoder refuses the URL, which does not happen for the fixed
/// URL constant.
pub fn bootstrap_qr(level: BootstrapLevel) -> Result<ModuleGrid, RenderError> {
    let version = smallest_version(level)?;
    let bits = encode_bits(version, level).ok_or(RenderError::Bootstrap)?;
    let code = QrCode::with_bits(bits, ec_level(level)).map_err(|_| RenderError::Bootstrap)?;
    let side = code.width();
    let expected = 17 + 4 * usize::from(version);
    let colors = code.to_colors();
    if side != expected || colors.len() != side * side {
        return Err(RenderError::Bootstrap);
    }
    let side_u32 = u32::try_from(side).map_err(|_| RenderError::Bootstrap)?;
    let mut grid = ModuleGrid::new(side_u32, side_u32).ok_or(RenderError::Bootstrap)?;
    for (y, row) in (0..side_u32).zip(colors.chunks(side)) {
        for (x, color) in (0..side_u32).zip(row) {
            grid.set(x, y, *color == Color::Dark);
        }
    }
    Ok(grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        text.split_whitespace().map(|pair| u8::from_str_radix(pair, 16).unwrap()).collect()
    }

    #[test]
    fn url_is_the_registry_value() {
        assert_eq!(BOOTSTRAP_URL.len(), 40);
        assert!(BOOTSTRAP_URL.is_ascii());
        assert!(BOOTSTRAP_URL.starts_with("https://"));
        // 8.8.1 value bytes.
        let value = hex("68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E \
             65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65");
        assert_eq!(BOOTSTRAP_URL.as_bytes(), value.as_slice());
    }

    #[test]
    fn smallest_versions_of_8_3() {
        assert_eq!(smallest_version(BootstrapLevel::M), Ok(3));
        assert_eq!(smallest_version(BootstrapLevel::Q), Ok(4));
        assert_eq!(smallest_version(BootstrapLevel::H), Ok(5));
        // Version 2 at M holds 28 data codewords: too few for 42.
        assert!(encode_bits(2, BootstrapLevel::M).is_none());
    }

    #[test]
    fn codewords_of_8_8_1() {
        let data = hex("42 86 87 47 47 07 33 A2 F2 F6 76 97 46 87 56 22 E6 36 F6 D2 F6 E6 \
             56 56 46 D6 F7 26 57 47 27 57 46 82 F6 E6 D7 46 36 F6 46 50 EC 11");
        let parity =
            hex("3D 7E 96 5A A8 1D 01 67 EA 68 9C D9 12 4C D4 F3 33 BC E8 1A DD 81 C4 14 2B 16");
        let bits = encode_bits(3, BootstrapLevel::M).unwrap();
        // 4 + 8 + 320 + 4 = 336 bits before padding (8.8.1).
        let raw = bits.into_bytes();
        let (data_cw, ec_cw) =
            qrcode::ec::construct_codewords(&raw, Version::Normal(3), EcLevel::M).unwrap();
        assert_eq!(data_cw, data);
        assert_eq!(ec_cw, parity);
    }

    #[test]
    fn matrix_sizes() {
        assert_eq!(bootstrap_qr(BootstrapLevel::M).unwrap().width(), 29);
        assert_eq!(bootstrap_qr(BootstrapLevel::Q).unwrap().width(), 33);
        assert_eq!(bootstrap_qr(BootstrapLevel::H).unwrap().width(), 37);
    }

    #[test]
    fn matrix_has_the_three_qr_finders() {
        let qr = bootstrap_qr(BootstrapLevel::M).unwrap();
        let side = qr.width();
        // Nested squares: 7×7 dark ring, 5×5 light ring, 3×3 dark core, at three corners.
        for (ox, oy) in [(0, 0), (side - 7, 0), (0, side - 7)] {
            for y in 0..7 {
                for x in 0..7 {
                    let ring = x.min(y).min(6 - x).min(6 - y);
                    let dark = ring != 1;
                    assert_eq!(qr.get(ox + x, oy + y), Some(dark), "({x}, {y}) at ({ox}, {oy})");
                }
            }
        }
    }
}
