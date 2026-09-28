# NMT Code

English | [한국어](README.ko.md)

NMT Code (needmoretruth code) is a two-dimensional code for the cameras of current phones and
computers. The same content needs fewer modules than a QR Code.

![An NMT Code symbol holding this repository's address, with a small QR Code beside it](docs/images/example-url.png)

The symbol on the left holds `https://github.com/needmoretruth/nmtcode`. The QR Code on the right
holds the same address, so a phone that reads only QR Codes gets a link to this page.

> **Version 0.0.1 is a prototype.** It makes black-and-white codes as PNG or SVG and reads back PNG
> files it made. Reading camera photos, colour codes and multi-frame file transfer are still being
> built. The format can change in any 0.x version.

## Size compared with QR Code

| Content | Bytes | NMT Code | QR Code |
|---|---:|---:|---:|
| `https://github.com/needmoretruth/nmtcode` | 40 | 24 × 24 = 576 | 29 × 29 = 841 |
| A Korean sentence, 84 bytes in UTF-8 | 84 | 32 × 32 = 1,024 | 37 × 37 = 1,369 (UTF-8) · 33 × 33 = 1,089 (EUC-KR) |
| An 87-byte English sentence | 87 | 32 × 32 = 1,024 | 37 × 37 = 1,369 |
| 50 digits | 50 | 24 × 24 = 576 | 25 × 25 = 625 |

- Both codes use their lowest error-correction level, which restores about 7.5% (NMT Code) and 7%
  (QR Code) of the bytes. QR Code is the smallest version that holds the content. Many QR readers
  decode Korean correctly only in UTF-8.
- The counts leave out the quiet zone: NMT Code needs 2 modules on each side, QR Code 4.
- The QR Code beside the symbol adds to the image. `--no-qr` leaves it out.
- Measured on 2026-09-29 with nmtcode 0.0.1, zxing-cpp 3.1.1 and segno 1.6.6. Camera reading
  distance and failure rate are not measured yet.

## Install

Build from source with [Rust](https://rustup.rs). The repository pins Rust 1.98.1, and `rustup`
installs it on the first build.

```sh
git clone https://github.com/needmoretruth/nmtcode.git
cd nmtcode
cargo install --path crates/nmtcode-cli --locked
nmtcode --version
```

## Use the command

```sh
nmtcode make --url https://example.com -o link.png
nmtcode make --text "Meet at the north entrance at 7 pm" -o note.svg
nmtcode make --file report.pdf -o report.png
nmtcode make --profile print --dpi 600 --text "Printed label" -o label.png

nmtcode read link.png
nmtcode read report.png --out received/
nmtcode read note.png --json
```

- `make` writes PNG or SVG by the file extension. `--level 0-3` sets error correction, and
  `--size WxH`, `--max-width` and `--max-height` set the size in modules.
- `read` prints text and URLs, and never opens a URL. Files are saved only with `--out`, never over
  an existing file, and under a name stripped of directories.
- `nmtcode make --help` and `nmtcode read --help` list every option.

## Use the library

```rust
use nmtcode::{DecodeOptions, EncodeOptions};
use nmtcode_render::RenderOptions;

let url = "https://github.com/needmoretruth/nmtcode";
let symbol = nmtcode::encode_url(url, &EncodeOptions::default())?;
let png = nmtcode_render::render_png(symbol.grid(), &RenderOptions::default())?;

for grid in nmtcode_detect::read_png(&png)? {
    let decoded = nmtcode::decode(&grid, &DecodeOptions::default())?;
    println!("{}", decoded.records[0].text().unwrap_or("(binary record)"));
}
```

The full program is [`crates/nmtcode-cli/examples/round_trip.rs`](crates/nmtcode-cli/examples/round_trip.rs).

| Crate | What it does | `no_std` |
|---|---|---|
| `nmtcode` | Encode records to a module grid and decode them back | |
| `nmtcode-core` | Module grid, format word, container, records, CRC-32C, reader errors | ✅ |
| `nmtcode-ecc` | Reed-Solomon error correction over GF(2^8) | ✅ |
| `nmtcode-payload` | Codecs: stored, digits, alphanumeric, token table, Hangul packing, brotli | ✅ without brotli |
| `nmtcode-symbol` | Finder patterns, reference marks, module placement, whitening | ✅ |
| `nmtcode-render` | PNG and SVG output with the QR Code beside the symbol | |
| `nmtcode-detect` | Finds symbols in PNG images the renderer made | |
| `nmtcode-cli` | The `nmtcode` command | |

The `no_std` crates also build for `wasm32-unknown-unknown`. No crate uses `unsafe`.

## Specification

The format is written in [`spec/`](spec/01-scope-and-conventions.md), draft 0.1. It defines every
bit, so another program can make and read NMT Code without this code. The worked examples of the
specification and its complete test symbol ([Annex A](spec/annex-a-test-vectors.md)) are tests of
this repository.

## Check

```sh
scripts/check.sh
```

It runs formatting, clippy, every test, the `wasm32` build of the `no_std` crates, and a check that
no crate version reaches 1.0.0.

## License

- Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your choice.
- Specification (`spec/`): [CC BY 4.0](spec/LICENSE). CC BY 4.0 grants no patent rights.

QR Code is a registered trademark of DENSO WAVE INCORPORATED.
