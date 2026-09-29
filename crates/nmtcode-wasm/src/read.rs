//! Reading symbols from an RGBA image, such as a camera frame or a decoded image file, and the
//! result as JSON for the page.

use nmtcode::{
    DecodeError, DecodeOptions, Decoded, DecodedRecord, Outcome, PresentAs, SpecError, ValueNotice,
};
use nmtcode_detect::LumaImage;

use crate::json::Json;

/// The luma of one sRGB-encoded pixel with the ITU-R BT.601 weights of chapter 1 (1.4),
/// Y′ = 0.299 R′ + 0.587 G′ + 0.114 B′, rounded, after compositing its alpha over white.
pub fn luma(red: u8, green: u8, blue: u8, alpha: u8) -> u8 {
    // 299 + 587 + 114 = 1000, so the weighted sum over 1000 is at most 255.
    let y = (299 * u32::from(red) + 587 * u32::from(green) + 114 * u32::from(blue) + 500) / 1000;
    let alpha = u32::from(alpha);
    // Over white: y · a / 255 + 255 · (255 − a) / 255, rounded; at most 255.
    let composited = (y * alpha + 255 * (255 - alpha) + 127) / 255;
    u8::try_from(composited).unwrap_or(u8::MAX)
}

/// The luminance image of `rgba`: `width × height` pixels of 4 bytes (red, green, blue,
/// alpha), row by row from the top-left, as a canvas's `ImageData` holds them. `None` when a
/// side is 0, the buffer is not exactly `width × height × 4` bytes, or the image has more than
/// [`nmtcode_detect::MAX_PIXELS`] pixels.
pub fn rgba_to_luma(width: u32, height: u32, rgba: &[u8]) -> Option<LumaImage> {
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels == 0 || pixels > nmtcode_detect::MAX_PIXELS {
        return None;
    }
    let expected = usize::try_from(pixels.checked_mul(4)?).ok()?;
    if rgba.len() != expected {
        return None;
    }
    // The length is a multiple of 4 here, so no bytes are left over.
    let (pixels, _) = rgba.as_chunks::<4>();
    let luma =
        pixels.iter().map(|&[red, green, blue, alpha]| luma(red, green, blue, alpha)).collect();
    LumaImage::new(width, height, luma)
}

/// What one symbol found in the image gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolReading {
    /// The symbol passed every check; its base records are presented.
    Decoded(Decoded),
    /// The symbol was found but failed a check (chapter 9, 9.8).
    Failed(DecodeError),
}

/// Everything read from one image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    /// False when the pixel buffer did not match the width and height, or was too large.
    pub image_ok: bool,
    /// One entry per symbol found, in the order the detector found them.
    pub symbols: Vec<SymbolReading>,
}

impl Reading {
    /// The value of record `record` of symbol `symbol`; `None` when there is no such record.
    pub fn value(&self, symbol: usize, record: usize) -> Option<&[u8]> {
        match self.symbols.get(symbol)? {
            SymbolReading::Decoded(decoded) => {
                decoded.records.get(record).map(|record| record.value.as_slice())
            }
            SymbolReading::Failed(_) => None,
        }
    }

    /// The reading as JSON text, without the values of the records (see [`Reading::value`]):
    ///
    /// ```text
    /// {"image_ok":true,"symbols":[
    ///   {"ok":true,"outcome":"presented","notice":null,"width":20,"height":28,"level":0,
    ///    "colour_profile":0,"codec":3,"dictionary":0,"corrected":0,"records":[
    ///      {"type":2,"type_name":"URL","present_as":"url","length":40,
    ///       "text":"https://…","file_name":null,"notice":null}]},
    ///   {"ok":false,"error":"E_CRC_MISMATCH","outcome":"damaged"}]}
    /// ```
    ///
    /// `present_as` is `bytes`, `text`, `url`, `file`, `psbt`, `file_name` or `unknown`
    /// (chapter 3, 3.4.3). `text` is set for `text`, `url` and `file_name` records. `file_name`
    /// is the name to offer when saving the record: the file name record before it, made safe
    /// by the rules of 3.4.3, or `null`. A record's `notice` is `not_utf8` or `not_psbt`; a
    /// symbol's `notice` is `E_EXTENSION_UNREAD` when it holds colour content this reader did
    /// not read (3.5 rule 2).
    pub fn to_json(&self) -> String {
        Json::Object(vec![
            ("image_ok", Json::Bool(self.image_ok)),
            ("symbols", Json::Array(self.symbols.iter().map(symbol_json).collect())),
        ])
        .to_text()
    }
}

/// Finds every symbol in the RGBA image and decodes each (see [`rgba_to_luma`] for the
/// buffer layout). A buffer that does not match the sides gives a reading with
/// [`Reading::image_ok`] false and no symbols.
pub fn read_rgba(width: u32, height: u32, rgba: &[u8]) -> Reading {
    let Some(image) = rgba_to_luma(width, height, rgba) else {
        return Reading { image_ok: false, symbols: Vec::new() };
    };
    let options = DecodeOptions::default();
    let detect_options =
        nmtcode_detect::DetectOptions { max_area: options.max_area, ..Default::default() };
    let scan = nmtcode_detect::find(&image, &detect_options);
    // Found symbols are decoded with the detector's uncertain modules as erasures (chapter 4,
    // 4.9). Rejected symbols are reported by the detector's rule (`Scan::reported_errors`):
    // nested pairs once, candidates without a readable format word only when nothing else was
    // read.
    let mut symbols: Vec<SymbolReading> = scan
        .found
        .iter()
        .map(|found| match nmtcode::decode_with_erasures(&found.grid, &found.uncertain, &options) {
            Ok(decoded) => SymbolReading::Decoded(decoded),
            Err(error) => SymbolReading::Failed(error),
        })
        .collect();
    symbols.extend(
        scan.reported_errors().into_iter().map(|error| SymbolReading::Failed(error.into())),
    );
    Reading { image_ok: true, symbols }
}

/// The name of an outcome class of chapter 3 (3.9).
pub const fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Damaged => "damaged",
        Outcome::Unsupported => "unsupported",
        Outcome::Malformed => "malformed",
        Outcome::Presented => "presented",
        Outcome::PresentedBaseOnly => "presented_base_only",
        Outcome::PresentedWithError => "presented_with_error",
    }
}

/// The name of a presentation of chapter 3 (3.4.3).
pub const fn present_as_name(present_as: PresentAs) -> &'static str {
    match present_as {
        PresentAs::Bytes => "bytes",
        PresentAs::Text => "text",
        PresentAs::Url => "url",
        PresentAs::File => "file",
        PresentAs::Psbt => "psbt",
        PresentAs::FileName => "file_name",
        PresentAs::Unknown => "unknown",
    }
}

const fn notice_name(notice: ValueNotice) -> &'static str {
    match notice {
        ValueNotice::NotUtf8 => "not_utf8",
        ValueNotice::NotPsbt => "not_psbt",
    }
}

fn record_json(decoded: &Decoded, index: usize, record: &DecodedRecord) -> Json {
    Json::Object(vec![
        ("type", Json::Number(u64::from(record.content_type.0))),
        ("type_name", Json::optional(record.content_type.name())),
        ("present_as", Json::string(present_as_name(record.present_as))),
        ("length", Json::count(record.value.len())),
        ("text", Json::optional(record.text())),
        ("file_name", Json::optional(decoded.file_name_for(index))),
        ("notice", Json::optional(record.notice.map(notice_name))),
    ])
}

fn symbol_json(symbol: &SymbolReading) -> Json {
    match symbol {
        SymbolReading::Decoded(decoded) => Json::Object(vec![
            ("ok", Json::Bool(true)),
            ("outcome", Json::string(outcome_name(decoded.outcome))),
            ("notice", Json::optional(decoded.notice.map(SpecError::name))),
            ("width", Json::Number(u64::from(decoded.format.width()))),
            ("height", Json::Number(u64::from(decoded.format.height()))),
            ("level", Json::Number(u64::from(decoded.format.level()))),
            ("colour_profile", Json::Number(u64::from(decoded.format.colour_profile()))),
            ("codec", Json::Number(u64::from(decoded.codec))),
            ("dictionary", Json::Number(u64::from(decoded.dictionary))),
            ("corrected", Json::count(decoded.corrected)),
            (
                "records",
                Json::Array(
                    decoded
                        .records
                        .iter()
                        .enumerate()
                        .map(|(index, record)| record_json(decoded, index, record))
                        .collect(),
                ),
            ),
        ]),
        SymbolReading::Failed(error) => Json::Object(vec![
            ("ok", Json::Bool(false)),
            ("error", Json::string(error.name())),
            ("outcome", Json::string(outcome_name(error.outcome()))),
        ]),
    }
}
