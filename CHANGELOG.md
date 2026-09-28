# Changelog

English | [한국어](CHANGELOG.ko.md)

## 0.0.1 — unreleased

### Added

- Make black-and-white NMT Code symbols from text, a URL or a file, as PNG or SVG.
- Read the symbols back from PNG images that `nmtcode` made, and save files with `--out`.
- Choose the error-correction level (0–3), the size in modules, the module size and the DPI.
- Pick the shortest of six codecs for the content, including Hangul packing and brotli.
- Add a standard QR Code that links to this repository beside the symbol with `--qr`; printed codes
  have it by default.
- Publish draft 0.2 of the format specification under CC BY 4.0.
