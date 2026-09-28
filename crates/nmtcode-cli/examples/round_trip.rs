//! Make a symbol from a URL, write it as a PNG, and read the PNG back.
//!
//! Run with `cargo run -p nmtcode-cli --example round_trip`.

use std::error::Error;

use nmtcode::{DecodeOptions, EncodeOptions};
use nmtcode_render::RenderOptions;

fn main() -> Result<(), Box<dyn Error>> {
    // Encode: content -> module grid -> PNG bytes.
    let url = "https://github.com/needmoretruth/nmtcode";
    let symbol = nmtcode::encode_url(url, &EncodeOptions::default())?;
    let png = nmtcode_render::render_png(symbol.grid(), &RenderOptions::default())?;
    println!("{} x {} modules, {} bytes of PNG", symbol.width(), symbol.height(), png.len());

    // Decode: PNG bytes -> module grids -> records.
    for grid in nmtcode_detect::read_png(&png)? {
        let decoded = nmtcode::decode(&grid, &DecodeOptions::default())?;
        for record in &decoded.records {
            println!("{}", record.text().unwrap_or("(binary record)"));
        }
    }
    Ok(())
}
