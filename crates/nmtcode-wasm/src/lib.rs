//! WebAssembly bindings of NMT Code for the browser page in `web/`: make a symbol, and read
//! symbols from camera frames or image files.
//!
//! The logic is in plain Rust functions that native tests call; the `wasm-bindgen` layer only
//! converts types.
//!
//! | Rust | JavaScript (`wasm-bindgen --target web`) |
//! |---|---|
//! | [`make::make`] | `encode(kind, data, fileName, level, modulePx, bootstrap) → Encoded`, throws `Error(code)` |
//! | [`read::read_rgba`] | `decode_rgba(width, height, rgba) → Reading` with `.json` and `.value(symbol, record)` |
//! | [`read::rgba_to_luma`] | used by `decode_rgba` |
//! | `env!("CARGO_PKG_VERSION")` | `version() → string` |
//!
//! Reading converts every pixel to luma with the ITU-R BT.601 weights of the specification
//! (chapter 1, 1.4) after compositing its alpha over white, finds the symbols with
//! `nmtcode-detect` and decodes each with `nmtcode`. Presenting the records by the rules of
//! chapter 3 (3.4.3) is the page's job; this crate supplies the presentation class of every
//! record and the safe file name of 3.4.3.

pub mod bindings;
mod json;
pub mod make;
pub mod read;

pub use make::{Kind, Made, MakeError, MakeOptions, make};
pub use read::{Reading, SymbolReading, luma, read_rgba, rgba_to_luma};

#[cfg(test)]
mod tests {
    use nmtcode::{
        ContentType, DecodeError, Decoded, DecodedRecord, FormatWord, Outcome, PresentAs,
        RecordForm, SpecError, SymbolClass,
    };

    use super::*;

    /// The RGBA pixels of a PNG made by `nmtcode-render`, as a canvas would hold them.
    fn png_to_rgba(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().expect("PNG header");
        let mut buffer = vec![0; reader.output_buffer_size().expect("buffer size")];
        let info = reader.next_frame(&mut buffer).expect("PNG frame");
        assert_eq!(info.color_type, png::ColorType::Grayscale);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        let grey = buffer.get(..info.buffer_size()).expect("frame bytes");
        let rgba = grey.iter().flat_map(|&y| [y, y, y, 255]).collect();
        (info.width, info.height, rgba)
    }

    /// Makes a symbol and reads its PNG back through the RGBA path.
    fn round_trip(kind: Kind, data: &[u8], name: &str, options: MakeOptions) -> Decoded {
        let made = make(kind, data, name, options).expect("make");
        let (width, height, rgba) = png_to_rgba(&made.png);
        assert_eq!((width, height), (made.image_width, made.image_height));
        let reading = read_rgba(width, height, &rgba);
        assert!(reading.image_ok);
        assert_eq!(reading.symbols.len(), 1, "one symbol in the image");
        match reading.symbols.into_iter().next() {
            Some(SymbolReading::Decoded(decoded)) => {
                assert_eq!(
                    (decoded.format.width(), decoded.format.height()),
                    (made.width, made.height)
                );
                assert_eq!(decoded.format.level(), made.level);
                decoded
            }
            other => panic!("not decoded: {other:?}"),
        }
    }

    #[test]
    fn text_round_trip() {
        let text = "안녕하세요 NMT Code\nline two\twith a tab";
        let decoded = round_trip(Kind::Text, text.as_bytes(), "", MakeOptions::default());
        assert_eq!(decoded.outcome, Outcome::Presented);
        assert_eq!(decoded.records.len(), 1);
        assert_eq!(decoded.records[0].present_as, PresentAs::Text);
        assert_eq!(decoded.records[0].text(), Some(text));
    }

    #[test]
    fn url_round_trip_at_every_level() {
        let url = "https://github.com/needmoretruth/nmtcode";
        for level in 0..=3 {
            let options = MakeOptions { level, ..MakeOptions::default() };
            let decoded = round_trip(Kind::Url, url.as_bytes(), "ignored", options);
            assert_eq!(decoded.records.len(), 1);
            assert_eq!(decoded.records[0].content_type, ContentType::URL);
            assert_eq!(decoded.records[0].present_as, PresentAs::Url);
            assert_eq!(decoded.records[0].text(), Some(url));
        }
    }

    #[test]
    fn default_url_symbol_is_the_readme_size() {
        let made = make(
            Kind::Url,
            b"https://github.com/needmoretruth/nmtcode",
            "",
            MakeOptions::default(),
        )
        .expect("make");
        assert_eq!((made.width, made.height, made.level), (20, 28, 0));
        // Quiet zone of 2 modules on every side, 4 pixels per module.
        assert_eq!((made.preview_width, made.preview_height), (24, 32));
        assert_eq!((made.image_width, made.image_height), (96, 128));
        assert_eq!(made.preview.len(), 24 * 32);
        assert!(made.bytes_used <= made.capacity);
        assert!(made.svg.starts_with("<svg") || made.svg.starts_with("<?xml"));
        assert!(made.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn file_round_trip_with_its_name() {
        let bytes: Vec<u8> = (0..=255u8).cycle().take(700).collect();
        let decoded = round_trip(Kind::File, &bytes, "report.pdf", MakeOptions::default());
        assert_eq!(decoded.records.len(), 2);
        assert_eq!(decoded.records[0].present_as, PresentAs::FileName);
        assert_eq!(decoded.records[1].present_as, PresentAs::File);
        assert_eq!(decoded.records[1].value, bytes);
        assert_eq!(decoded.file_name_for(1).as_deref(), Some("report.pdf"));
    }

    #[test]
    fn file_without_a_name_is_one_record() {
        let decoded = round_trip(Kind::File, b"hello\n", "", MakeOptions::default());
        assert_eq!(decoded.records.len(), 1);
        assert_eq!(decoded.records[0].present_as, PresentAs::File);
        assert_eq!(decoded.file_name_for(0), None);
    }

    #[test]
    fn small_modules_and_the_bootstrap_code_round_trip() {
        let options = MakeOptions { level: 1, module_px: 2, bootstrap: true };
        let plain = make(Kind::Text, b"NMT Code", "", MakeOptions { bootstrap: false, ..options })
            .expect("make");
        let with_qr = make(Kind::Text, b"NMT Code", "", options).expect("make");
        assert!(
            with_qr.preview_width * with_qr.preview_height
                > plain.preview_width * plain.preview_height
        );
        let modules = usize::try_from(with_qr.preview_width * with_qr.preview_height);
        assert_eq!(Ok(with_qr.preview.len()), modules);
        let decoded = round_trip(Kind::Text, b"NMT Code", "", options);
        assert_eq!(decoded.records[0].text(), Some("NMT Code"));
    }

    #[test]
    fn unsafe_file_names_are_made_safe() {
        for (name, safe) in [
            ("../.bashrc", Some("bashrc")),
            ("invoice\u{202E}fdp.exe", Some("invoicefdp.exe")),
            ("C:\\Users\\a\\photo.jpg", Some("photo.jpg")),
            ("con.txt", Some("_con.txt")),
            ("LPT9", Some("_LPT9")),
            ("what?.txt", Some("what_.txt")),
            ("name. . .", Some("name")),
            ("...", None),
        ] {
            let decoded = round_trip(Kind::File, b"x", name, MakeOptions::default());
            assert_eq!(decoded.file_name_for(1).as_deref(), safe, "{name:?}");
            let json =
                read::Reading { image_ok: true, symbols: vec![SymbolReading::Decoded(decoded)] }
                    .to_json();
            let expected = match safe {
                Some(safe) => format!("\"file_name\":\"{safe}\""),
                None => "\"file_name\":null".to_owned(),
            };
            assert!(json.contains(&expected), "{json}");
        }
        let long = "é".repeat(200);
        let decoded = round_trip(Kind::File, b"x", &long, MakeOptions::default());
        let name = decoded.file_name_for(1).expect("a name");
        assert_eq!(name.len(), 254, "cut at a character boundary below 255 bytes");
        assert!(long.starts_with(&name));
    }

    #[test]
    fn make_errors_have_their_codes() {
        let options = MakeOptions::default();
        let error = |kind, data: &[u8], options: MakeOptions| {
            make(kind, data, "", options).map(|_| ()).unwrap_err().code()
        };
        assert_eq!(error(Kind::Text, &[0xFF, 0xFE], options), "not_utf8");
        assert_eq!(error(Kind::Url, &[0xC3], options), "not_utf8");
        assert_eq!(error(Kind::Text, b"a", MakeOptions { level: 4, ..options }), "invalid_level");
        assert_eq!(
            error(Kind::Text, b"a", MakeOptions { module_px: 0, ..options }),
            "invalid_module_size"
        );
        assert_eq!(
            error(Kind::Text, b"a", MakeOptions { module_px: 65, ..options }),
            "invalid_module_size"
        );
        let too_big = vec![0u8; 1_048_577];
        assert_eq!(error(Kind::File, &too_big, options), "too_large");
        // About 20 KB that does not compress needs a symbol of about 400 × 400 modules; at 64
        // pixels per module the PNG would be far above 8192 × 8192 pixels.
        let mut state = 0x1234_5678_u32;
        let noise: Vec<u8> = (0..20_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            })
            .collect();
        assert_eq!(
            error(Kind::File, &noise, MakeOptions { module_px: 64, ..options }),
            "image_too_large"
        );
        assert_eq!(Kind::from_name("pdf"), None);
        assert_eq!(Kind::from_name("url"), Some(Kind::Url));
        let codes = [
            MakeError::UnknownKind,
            MakeError::NotUtf8,
            MakeError::InvalidLevel,
            MakeError::InvalidModuleSize,
            MakeError::TooLarge,
            MakeError::NoSize,
            MakeError::ImageTooLarge,
            MakeError::Internal,
        ]
        .map(MakeError::code);
        // The page (web/strings.js) has one sentence per code, under `make_error_<code>`.
        assert_eq!(
            codes,
            [
                "unknown_kind",
                "not_utf8",
                "invalid_level",
                "invalid_module_size",
                "too_large",
                "no_size",
                "image_too_large",
                "internal",
            ]
        );
    }

    #[test]
    fn luma_uses_the_bt601_weights_over_white() {
        assert_eq!(luma(0, 0, 0, 255), 0);
        assert_eq!(luma(255, 255, 255, 255), 255);
        assert_eq!(luma(255, 0, 0, 255), 76);
        assert_eq!(luma(0, 255, 0, 255), 150);
        assert_eq!(luma(0, 0, 255, 255), 29);
        assert_eq!(luma(0, 0, 0, 0), 255, "transparent is white");
        assert_eq!(luma(0, 0, 0, 128), 127);
    }

    #[test]
    fn rgba_buffers_must_match_their_sides() {
        assert!(rgba_to_luma(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]).is_some());
        assert!(rgba_to_luma(2, 1, &[0; 7]).is_none());
        assert!(rgba_to_luma(2, 1, &[0; 9]).is_none());
        assert!(rgba_to_luma(0, 1, &[]).is_none());
        assert!(rgba_to_luma(u32::MAX, u32::MAX, &[]).is_none());
        let reading = read_rgba(3, 3, &[0; 5]);
        assert!(!reading.image_ok);
        assert!(reading.symbols.is_empty());
        assert_eq!(reading.to_json(), r#"{"image_ok":false,"symbols":[]}"#);
    }

    #[test]
    fn images_without_a_symbol_give_no_symbols() {
        let white = vec![255u8; 64 * 48 * 4];
        let reading = read_rgba(64, 48, &white);
        assert!(reading.image_ok);
        assert!(reading.symbols.is_empty());
        let mut state = 0x9E37_79B9_u32;
        let noise: Vec<u8> = (0..160 * 120 * 4)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[1]
            })
            .collect();
        assert!(read_rgba(160, 120, &noise).image_ok);
    }

    #[test]
    fn every_reader_error_keeps_its_name_in_the_json() {
        for error in SpecError::ALL {
            let reading = read::Reading {
                image_ok: true,
                symbols: vec![SymbolReading::Failed(DecodeError::from(error))],
            };
            let expected = format!(
                r#"{{"image_ok":true,"symbols":[{{"ok":false,"error":"{}","outcome":"{}"}}]}}"#,
                error.name(),
                read::outcome_name(error.outcome())
            );
            assert_eq!(reading.to_json(), expected);
            assert_eq!(reading.value(0, 0), None);
        }
    }

    #[test]
    fn a_damaged_symbol_is_reported_by_name() {
        let symbol =
            nmtcode::encode_text("NMT Code", &nmtcode::EncodeOptions::default()).expect("encode");
        let mut grid = symbol.into_grid();
        // Flip every module of the rows between the finders: the format copies and finders
        // stay, the data codewords are lost.
        for y in 7..grid.height() - 7 {
            for x in 0..grid.width() {
                grid.toggle(x, y);
            }
        }
        let error = nmtcode::decode(&grid, &nmtcode::DecodeOptions::default())
            .expect_err("too damaged to decode");
        assert_eq!(error.outcome(), Outcome::Damaged);
        let json =
            read::Reading { image_ok: true, symbols: vec![SymbolReading::Failed(error)] }.to_json();
        assert!(json.contains(&format!("\"error\":\"{}\"", error.name())), "{json}");
    }

    #[test]
    fn colour_content_that_was_not_read_is_named() {
        let format = FormatWord::new(SymbolClass::Static, 32, 32, 0, 1, 0).expect("format word");
        let decoded = Decoded {
            records: vec![DecodedRecord {
                content_type: ContentType::TEXT,
                value: b"NMT Code".to_vec(),
                present_as: PresentAs::Text,
                notice: None,
            }],
            outcome: Outcome::PresentedBaseOnly,
            notice: Some(SpecError::ExtensionUnread),
            format,
            codec: 0,
            dictionary: 0,
            record_form: RecordForm::Single(ContentType::TEXT),
            corrected: 0,
        };
        let reading =
            read::Reading { image_ok: true, symbols: vec![SymbolReading::Decoded(decoded)] };
        let json = reading.to_json();
        assert!(
            json.contains(r#""outcome":"presented_base_only","notice":"E_EXTENSION_UNREAD""#),
            "{json}"
        );
        assert!(json.contains(r#""colour_profile":1"#), "{json}");
        assert!(json.contains(r#""text":"NMT Code""#), "{json}");
        assert_eq!(reading.value(0, 0), Some(&b"NMT Code"[..]));
        assert_eq!(reading.value(0, 1), None);
        assert_eq!(reading.value(1, 0), None);
    }

    #[test]
    fn record_json_names_the_presentation() {
        let url = "https://example.com/a\"b";
        let made = make(Kind::Url, url.as_bytes(), "", MakeOptions::default()).expect("make");
        let (width, height, rgba) = png_to_rgba(&made.png);
        let json = read_rgba(width, height, &rgba).to_json();
        assert!(
            json.starts_with(
                r#"{"image_ok":true,"symbols":[{"ok":true,"outcome":"presented","notice":null,"#
            ),
            "{json}"
        );
        assert!(
            json.contains(r#"{"type":2,"type_name":"URL","present_as":"url","length":23,"text":"https://example.com/a\"b","file_name":null,"notice":null}"#),
            "{json}"
        );
    }
}
