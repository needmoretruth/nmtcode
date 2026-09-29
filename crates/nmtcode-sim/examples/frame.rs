//! Writes one synthetic camera photo of a symbol as a JPEG file: condition A of the
//! measurement (1920 × 1080, 3 camera pixels per module, blur, noise, JPEG quality 80, a tilt
//! of up to 30° and any rotation).
//!
//! ```text
//! cargo run -p nmtcode-sim --release --example frame -- "text to encode" photo.jpg [seed]
//! ```

use std::process::ExitCode;

use nmtcode::EncodeOptions;
use nmtcode_sim::{Channel, canvas, render};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(text), Some(path)) = (args.first(), args.get(1)) else {
        eprintln!("usage: frame TEXT OUTPUT.jpg [SEED]");
        return ExitCode::from(2);
    };
    let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let Ok(symbol) = nmtcode::encode_text(text, &EncodeOptions::default()) else {
        eprintln!("the text does not fit in one symbol");
        return ExitCode::FAILURE;
    };
    let Some(frame) = canvas(&symbol, false).and_then(|c| render(&c, &Channel::default(), seed))
    else {
        eprintln!("the symbol does not fit in the frame");
        return ExitCode::FAILURE;
    };
    let Some(jpeg) = frame.jpeg else {
        eprintln!("the channel wrote no JPEG");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(path, jpeg) {
        eprintln!("cannot write {path}: {error}");
        return ExitCode::FAILURE;
    }
    println!("{} x {} modules -> {path}", symbol.width(), symbol.height());
    ExitCode::SUCCESS
}
