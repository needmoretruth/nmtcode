//! Mutated and truncated JPEG files never panic `read_image` or `find`; the JPEG encoder of
//! this crate makes files that the reader decodes back.

use nmtcode_detect::{DetectOptions, find, read_image};
use nmtcode_sim::jpeg;
use proptest::prelude::*;

fn sample_jpeg(colour: bool) -> Vec<u8> {
    let (w, h) = (48u32, 40u32);
    let luma: Vec<u8> =
        (0..w * h).map(|i| u8::try_from((i * 7 + i / w * 13) % 256).unwrap()).collect();
    if colour {
        let rgb: Vec<u8> = luma.iter().flat_map(|&v| [v, 255 - v, v / 2]).collect();
        jpeg::encode_rgb(w, h, &rgb, 75, Some(6)).unwrap()
    } else {
        jpeg::encode_luma(w, h, &luma, 75, None).unwrap()
    }
}

#[test]
fn encoded_files_decode_back() {
    let (w, h) = (37u32, 23u32);
    let luma: Vec<u8> = (0..w * h).map(|i| u8::try_from(i % 256).unwrap()).collect();
    let image = read_image(&jpeg::encode_luma(w, h, &luma, 100, None).unwrap()).unwrap();
    assert_eq!((image.width, image.height), (w, h));
    let error: u32 = image
        .pixels
        .iter()
        .zip(luma.iter())
        .map(|(&a, &b)| u32::from(a.abs_diff(b)))
        .max()
        .unwrap();
    assert!(error <= 3, "largest error {error}");
    // Orientation 6: a quarter turn, so the sides swap.
    let turned = read_image(&sample_jpeg(true)).unwrap();
    assert_eq!((turned.width, turned.height), (40, 48));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn mutated_jpeg_never_panics(
        colour in any::<bool>(),
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        let mut file = sample_jpeg(colour);
        for (at, value) in edits {
            let i = at % file.len();
            file[i] = value;
        }
        let keep = file.len() - cut % (file.len() / 2);
        if let Ok(image) = read_image(&file[..keep]) {
            let _ = find(&image, &DetectOptions::default());
        }
    }
}
