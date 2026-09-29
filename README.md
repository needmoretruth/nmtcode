# NMT Code

English | [한국어](README.ko.md)

NMT Code (needmoretruth code) is a two-dimensional code for the cameras of current phones and
computers. The same content needs fewer modules than a QR Code.

![An NMT Code symbol holding this repository's address, below a QR Code](docs/images/example-url.png)

The NMT Code symbol at the bottom holds `https://github.com/needmoretruth/nmtcode`. The QR Code
above it holds the same address, so a phone that reads only QR Codes gets a link to this page.
`--qr` adds that QR Code; printed codes (`--profile print`) have it by default.

> **Version 0.0.1** makes black-and-white codes as PNG or SVG, and reads them from camera photos,
> screenshots and image files (PNG, JPEG). It comes as a command-line tool, a web page and a Rust
> library. Colour codes and file transfer over several codes are still being built. Codes made with
> 0.0.1 stay readable by every later 0.x version.

## Size compared with QR Code

| Content | Bytes | NMT Code | QR Code |
|---|---:|---:|---:|
| `https://github.com/needmoretruth/nmtcode` | 40 | 20 × 28 = 560 | 29 × 29 = 841 |
| A Korean sentence, 84 bytes in UTF-8 | 84 | 24 × 36 = 864 | 37 × 37 = 1,369 (UTF-8) · 33 × 33 = 1,089 (EUC-KR) |
| An 87-byte English sentence | 87 | 24 × 36 = 864 | 37 × 37 = 1,369 |
| 50 digits | 50 | 20 × 28 = 560 | 25 × 25 = 625 |

- Both codes use their lowest error-correction level, which restores about 7.5% (NMT Code) and 7%
  (QR Code) of the bytes. QR Code is the smallest version that holds the content. Many QR readers
  decode Korean correctly only in UTF-8.
- The counts leave out the quiet zone: NMT Code needs 2 modules on each side, QR Code 4.
- NMT Code symbols are rectangles whose sides are within a factor of 2, which is why they are not square.
- The counts are for the symbol alone. With the QR Code of `--qr` beside it, the image is larger than a
  QR Code of the same content.
- Measured on 2026-09-29 with nmtcode 0.0.1, zxing-cpp 3.1.1 and segno 1.6.6.

## Reading camera photos

On synthetic camera photos (1920 × 1080, 3 camera pixels per module, blur of half a module, noise at
30 dB, JPEG quality 80, a tilt of up to 30°, any rotation, a cluttered background), 992 of 1,000
codes were read and none was read wrongly. Reading one photo took 11.9 ms (median) on a desktop
processor (AMD Ryzen 5 7500F).

- At a tilt of 45° with blur of half a module, 32–53% of the photos were read; with blur of 0.7
  module, at most 58%. No photo was read wrongly in any condition.
- Photos from real phones are not measured yet.
- `cargo run -p nmtcode-sim --release --example measure` repeats the measurement.

## Install

Download the file for your system from the [latest release](https://github.com/needmoretruth/nmtcode/releases/latest):

| System | File |
|---|---|
| Linux, x86_64 | `nmtcode-<version>-x86_64-unknown-linux-musl.tar.gz` |
| Linux, ARM64 | `nmtcode-<version>-aarch64-unknown-linux-musl.tar.gz` |
| macOS, Apple silicon | `nmtcode-<version>-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `nmtcode-<version>-x86_64-apple-darwin.tar.gz` |
| Windows, x86_64 | `nmtcode-<version>-x86_64-pc-windows-msvc.zip` |

Unpack it and put `nmtcode` (`nmtcode.exe` on Windows) on your `PATH`. `SHA256SUMS` in the release
lists the checksum of every file. The macOS binaries are not signed, so macOS asks you to allow them
the first time.

To build from source instead, install [Rust](https://rustup.rs) and run:

```sh
git clone https://github.com/needmoretruth/nmtcode.git
cd nmtcode
cargo install --path crates/nmtcode-cli --locked
nmtcode --version
```

The repository pins Rust 1.98.1, and `rustup` installs it on the first build.

## Use the command

```sh
nmtcode make --url https://example.com -o link.png
nmtcode make --text "Meet at the north entrance at 7 pm" -o note.svg
nmtcode make --file report.pdf -o report.png
nmtcode make --profile print --dpi 600 --text "Printed label" -o label.png

nmtcode read photo.jpg
nmtcode read report.png --out received/
nmtcode read link.png --json
```

- `make` writes PNG or SVG by the file extension. `--level 0-3` sets error correction, and
  `--size WxH`, `--max-width` and `--max-height` set the size in modules.
- `read` takes a PNG or JPEG: a camera photo, a screenshot or a file from `make`. It prints text
  and URLs, and never opens a URL. Files are saved only with `--out`, never over an existing file,
  and under a name stripped of directories.
- `nmtcode make --help` and `nmtcode read --help` list every option.

## Use the web page

The release has `nmtcode-web-<version>.zip`, a page that makes codes and reads them with the camera
or from an image file. Everything runs in the browser, and nothing you type or photograph leaves the
device.

```sh
unzip nmtcode-web-0.0.1.zip
cd nmtcode-web-0.0.1
python3 -m http.server 8000
```

Open `http://localhost:8000`. Browsers turn on the camera only for `https://` pages and
`localhost`; reading an image file works from any address.

## Use the library

```rust
use nmtcode::{DecodeOptions, EncodeOptions};
use nmtcode_detect::DetectOptions;
use nmtcode_render::RenderOptions;

let url = "https://github.com/needmoretruth/nmtcode";
let symbol = nmtcode::encode_url(url, &EncodeOptions::default())?;
let png = nmtcode_render::render_png(symbol.grid(), &RenderOptions::default())?;

// Any PNG or JPEG: a camera photo, a screenshot, or the PNG above.
let image = nmtcode_detect::read_image(&png)?;
for found in nmtcode_detect::find(&image, &DetectOptions::default()).found {
    let decoded =
        nmtcode::decode_with_erasures(&found.grid, &found.uncertain, &DecodeOptions::default())?;
    println!("{}", decoded.records[0].text().unwrap_or("(binary record)"));
}
```

The full program is [`crates/nmtcode-cli/examples/round_trip.rs`](crates/nmtcode-cli/examples/round_trip.rs).
The crates are not on crates.io yet; depend on them by Git URL.

| Crate | What it does | `no_std` |
|---|---|---|
| `nmtcode` | Encode records to a module grid and decode them back, with erasures from the detector | |
| `nmtcode-core` | Module grid, format word, container, records, CRC-32C, reader errors | ✅ |
| `nmtcode-ecc` | Reed-Solomon error correction over GF(2^8) | ✅ |
| `nmtcode-payload` | Codecs: stored, digits, alphanumeric, token table, Hangul packing, brotli | ✅ without brotli |
| `nmtcode-symbol` | Finder patterns, reference marks, module placement, whitening | ✅ |
| `nmtcode-render` | PNG and SVG output, with or without a QR Code beside the symbol | |
| `nmtcode-detect` | Finds symbols in camera photos, screenshots and image files (PNG, JPEG) | |
| `nmtcode-cli` | The `nmtcode` command | |
| `nmtcode-wasm` | The WebAssembly module of the web page | |
| `nmtcode-sim` | Synthetic camera photos for tests and measurement (not published) | |

The `no_std` crates also build for `wasm32-unknown-unknown`. No crate uses `unsafe`.

## Specification

The format is written in [`spec/`](spec/01-scope-and-conventions.md), version 0.2. It defines every
bit, so another program can make and read NMT Code without this code. The worked examples of the
specification and its complete test symbols ([Annex A](spec/annex-a-test-vectors.md)) are tests of
this repository.

## Check

```sh
scripts/check.sh
```

It runs formatting, clippy, every test, the `wasm32` builds, a check that no crate version reaches
1.0.0, the licences of every dependency, and a check of the files the repository may hold.

## Contribute

Pull requests are welcome. Each needs one line agreeing to the
[Contributor License Agreement](CLA.md) ([한국어 설명](CLA.ko.md)); the pull request template says
where. Report security problems privately, as [SECURITY.md](SECURITY.md) describes.

## License

- Code: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your choice.
- Specification (`spec/`): [CC BY 4.0](spec/LICENSE). CC BY 4.0 grants no patent rights.

QR Code is a registered trademark of DENSO WAVE INCORPORATED.
