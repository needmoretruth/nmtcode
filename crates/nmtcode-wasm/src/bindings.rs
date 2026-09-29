//! The `wasm-bindgen` layer: it converts types and calls the plain functions of [`crate::make`]
//! and [`crate::read`], and holds no logic of its own.

use wasm_bindgen::prelude::*;

use crate::make::{self, Kind, MakeOptions};
use crate::read;

/// The version of this crate, which is the version of NMT Code it implements.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// A symbol made by [`encode`]. In JavaScript every field is a read-only property.
#[wasm_bindgen]
pub struct Encoded {
    made: make::Made,
}

#[wasm_bindgen]
impl Encoded {
    /// The PNG file.
    #[wasm_bindgen(getter)]
    pub fn png(&self) -> Vec<u8> {
        self.made.png.clone()
    }

    /// The SVG document.
    #[wasm_bindgen(getter)]
    pub fn svg(&self) -> String {
        self.made.svg.clone()
    }

    /// One byte per canvas module, row-major: 1 = dark, 0 = light.
    #[wasm_bindgen(getter)]
    pub fn preview(&self) -> Vec<u8> {
        self.made.preview.clone()
    }

    /// Width of the canvas in modules: symbol, quiet zone and bootstrap QR Code.
    #[wasm_bindgen(getter, js_name = previewWidth)]
    pub fn preview_width(&self) -> u32 {
        self.made.preview_width
    }

    /// Height of the canvas in modules.
    #[wasm_bindgen(getter, js_name = previewHeight)]
    pub fn preview_height(&self) -> u32 {
        self.made.preview_height
    }

    /// Width of the PNG in pixels.
    #[wasm_bindgen(getter, js_name = imageWidth)]
    pub fn image_width(&self) -> u32 {
        self.made.image_width
    }

    /// Height of the PNG in pixels.
    #[wasm_bindgen(getter, js_name = imageHeight)]
    pub fn image_height(&self) -> u32 {
        self.made.image_height
    }

    /// Width W of the symbol in modules.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.made.width
    }

    /// Height H of the symbol in modules.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.made.height
    }

    /// Error-correction level, 0 to 3.
    #[wasm_bindgen(getter)]
    pub fn level(&self) -> u8 {
        self.made.level
    }

    /// The codec ID the encoder chose.
    #[wasm_bindgen(getter)]
    pub fn codec(&self) -> u32 {
        self.made.codec
    }

    /// The dictionary ID.
    #[wasm_bindgen(getter)]
    pub fn dictionary(&self) -> u32 {
        self.made.dictionary
    }

    /// Bytes of the message that the container uses, CRC-32C included.
    #[wasm_bindgen(getter, js_name = bytesUsed)]
    pub fn bytes_used(&self) -> usize {
        self.made.bytes_used
    }

    /// The message capacity K in bytes.
    #[wasm_bindgen(getter)]
    pub fn capacity(&self) -> usize {
        self.made.capacity
    }
}

/// Makes a symbol: `kind` is `"text"`, `"url"` or `"file"`, `data` the UTF-8 text or the file
/// bytes, `file_name` the file's name (used for `"file"` only). See [`make::make`].
///
/// # Errors
///
/// A JavaScript `Error` whose message is one of the codes of [`make::MakeError::code`].
#[wasm_bindgen]
pub fn encode(
    kind: &str,
    data: &[u8],
    file_name: &str,
    level: u8,
    module_px: u32,
    bootstrap: bool,
) -> Result<Encoded, JsError> {
    let kind = Kind::from_name(kind).ok_or_else(|| JsError::new("unknown_kind"))?;
    let options = MakeOptions { level, module_px, bootstrap };
    make::make(kind, data, file_name, options)
        .map(|made| Encoded { made })
        .map_err(|error| JsError::new(error.code()))
}

/// What [`decode_rgba`] read from one image.
#[wasm_bindgen]
pub struct Reading {
    reading: read::Reading,
}

#[wasm_bindgen]
impl Reading {
    /// The reading as JSON text; see [`read::Reading::to_json`].
    #[wasm_bindgen(getter)]
    pub fn json(&self) -> String {
        self.reading.to_json()
    }

    /// The value of record `record` of symbol `symbol`; empty when there is no such record.
    pub fn value(&self, symbol: usize, record: usize) -> Vec<u8> {
        self.reading.value(symbol, record).map(<[u8]>::to_vec).unwrap_or_default()
    }
}

/// Finds and decodes every symbol in an RGBA image of `width × height` pixels, as a canvas's
/// `ImageData` holds it. See [`read::read_rgba`].
#[wasm_bindgen]
pub fn decode_rgba(width: u32, height: u32, rgba: &[u8]) -> Reading {
    Reading { reading: read::read_rgba(width, height, rgba) }
}
