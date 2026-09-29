//! The page reads camera frames: a symbol photographed at an angle, blurred, noisy and
//! JPEG-compressed (condition A of the channel simulator) decodes through the RGBA path.

use nmtcode_sim::{Channel, Rng, canvas, random_symbol, render};
use nmtcode_wasm::read::{SymbolReading, read_rgba};

#[test]
fn a_simulated_camera_frame_reads_through_rgba() {
    let channel = Channel { width: 1280, height: 720, ..Channel::default() };
    let mut read = 0;
    for seed in 1..=6u64 {
        let mut rng = Rng::new(seed);
        let (symbol, content) = random_symbol(40, 40, 0, &mut rng).unwrap();
        let canvas = canvas(&symbol, false).unwrap();
        let frame = render(&canvas, &channel, rng.next_u64()).unwrap();
        let (width, height) = (frame.image.width, frame.image.height);
        let rgba: Vec<u8> = frame.image.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect();
        let reading = read_rgba(width, height, &rgba);
        assert!(reading.image_ok);
        for symbol in &reading.symbols {
            if let SymbolReading::Decoded(decoded) = symbol {
                let got: Vec<u8> = decoded.records.iter().flat_map(|r| r.value.clone()).collect();
                // Wrong data is never presented.
                assert_eq!(got, content);
                read += 1;
            }
        }
    }
    assert!(read >= 5, "{read} of 6 frames read");
}
