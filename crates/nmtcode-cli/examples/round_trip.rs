//! Make a symbol from a URL, write it as a PNG, and read it back the way a camera photo is read.
//!
//! Run with `cargo run -p nmtcode-cli --example round_trip`.

use std::error::Error;

use nmtcode::{DecodeOptions, EncodeOptions};
use nmtcode_detect::DetectOptions;
use nmtcode_render::RenderOptions;

fn main() -> Result<(), Box<dyn Error>> {
    // Make a symbol and write it as a PNG.
    let url = "https://github.com/needmoretruth/nmtcode";
    let symbol = nmtcode::encode_url(url, &EncodeOptions::default())?;
    let png = nmtcode_render::render_png(symbol.grid(), &RenderOptions::default())?;
    println!("{} x {} modules, {} bytes of PNG", symbol.width(), symbol.height(), png.len());

    // Read a PNG or JPEG: a camera photo, a screenshot, or the PNG above.
    let image = nmtcode_detect::read_image(&png)?;
    for found in nmtcode_detect::find(&image, &DetectOptions::default()).found {
        let options = DecodeOptions::default();
        let decoded = nmtcode::decode_with_erasures(&found.grid, &found.uncertain, &options)?;
        println!("{}", decoded.records[0].text().unwrap_or("(binary record)"));
    }
    Ok(())
}
