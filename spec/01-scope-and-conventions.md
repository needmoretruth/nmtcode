# NMT Code — 1. Scope and conventions

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder). CC BY 4.0 grants no patent rights.

## 1.1 What NMT Code is

NMT Code (full name "needmoretruth code") is a two-dimensional optical code for cameras on current phones and computers. It has two uses:

1. A single code, printed or shown on a screen, that holds more data than a QR Code of the same area for the same content.
2. A multi-frame transfer: a screen shows a sequence of codes and a camera receives a file, with no network, cable or pairing.

This version of the specification defines the single code (chapters 2 to 8). The multi-frame transfer uses the same symbol in "transfer tile" mode; its chapter is added in a later 0.x version.

NMT Code does not replace QR Code and is not compatible with QR readers. A symbol may carry a separate, standard QR Code beside it that gives a QR reader a link to an NMT Code reader (chapter 8).

## 1.2 Design baseline

- Target devices: phones and computers with a camera of at least 1280×720 pixels and continuous or fixed focus, with a Samsung Galaxy S20 (2020) as the reference phone. Printers are a second target.
- A reader is expected to do more work per frame than a QR reader. The budget is one 1920×1080 frame decoded in about 25 ms on the reference phone.
- Deliberately not supported: scanners that read only QR Code or 1D barcodes; laser scanners; readers with less than about 2 camera pixels per module; printers that cannot place a module on a whole number of printer dots.

## 1.3 Structure of a symbol

A symbol is a rectangle of W × H square modules surrounded by a quiet zone.

| Layer | Carried by | Content | Chapter |
|---|---|---|---|
| Function patterns | fixed modules | four corner finders, reference marks | 5 |
| Format word | fixed modules, two copies | format version, symbol class, W, H, error-correction level, colour profile | 2 |
| Base layer | luminance of data modules | error-corrected container: header, base records, integrity check | 3, 4, 5, 6 |
| Colour layer (optional) | chroma of data cells | error-corrected extension records | 7 |

Every symbol has a base layer that a black-and-white reader decodes by itself. In a colour symbol, the base layer carries the records that every reader shows and a digest of the whole content.

Encoding order for the base layer:

1. Choose a codec and encode the content (chapter 6).
2. Build the container: header, records, content, CRC-32C, padding (chapter 3).
3. Split the container into error-correction blocks and add parity (chapter 4).
4. Place the interleaved codewords on the data modules and apply whitening (chapter 5).
5. Write the format word (chapter 2) and the function patterns (chapter 5).

A reader runs these steps in reverse order.

## 1.4 Conventions

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY" and "OPTIONAL" in this specification are to be interpreted as described in BCP 14 (RFC 2119, RFC 8174) when, and only when, they appear in all capitals.

| Topic | Convention |
|---|---|
| Bit order | Most significant bit first, within bytes and within multi-bit fields |
| Byte | An octet |
| Variable-length integers | Unsigned LEB128, at most 5 bytes, value below 2^32. A decoder MUST reject a non-minimal encoding and a value of 2^32 or more |
| Base-layer module value | Dark = 1, light = 0 |
| Coordinates | (x, y) in modules. Origin at the top-left module of the symbol, quiet zone excluded. x grows to the right, y grows downward |
| Size | W = width, H = height, in modules |
| CRC-32C | Polynomial 0x1EDC6F41 (Castagnoli), reflected input and output, initial value 0xFFFFFFFF, final XOR 0xFFFFFFFF, as in RFC 3720. The check value of the ASCII string "123456789" is 0xE3069283 |
| Hash | SHA-256 (FIPS 180-4) |
| Hexadecimal | `0x` prefix for single values. Byte strings are written as space-separated pairs, such as `4E 4D 54` |

## 1.5 Profiles

A profile is a named set of defaults. All values a reader needs are stated in the symbol itself, so a reader does not need to know which profile made a symbol.

| Profile | Colour | Error-correction level | Module size default | Use |
|---|---|---|---|---|
| `screen` | black and white | 0 (recovers about 7.5% of bytes, like QR level L) | 4 screen pixels | codes shown on a display |
| `print` | black and white | 1 (about 15%, like QR level M) | at least 0.4 mm, a whole number of printer dots, at least 4 dots | printed codes |
| `lowend` | black and white | 1 (about 15%, like QR level M) | large modules for readers at about 2 camera pixels per module | cameras with low resolution or fixed focus |
| `color` | profile 1 of chapter 7 | 0 | as `screen` | experimental in 0.1 |

A generator MUST let the user change the size, the error-correction level, the colour profile, the quiet zone (2 modules or more) and the module size.

## 1.6 Conformance

- A **generator** conforms if every symbol it produces follows chapters 2 to 6, and chapter 7 or 8 when it uses a colour profile or a QR bootstrap.
- A **reader** conforms if it decodes every conforming black-and-white symbol, rejects every symbol that fails a check this specification defines, and never presents data that failed a check as a result.
- A reader that supports only black and white conforms. When it reads a colour symbol it MUST present the base records (chapter 3).

## 1.7 Chapters

| File | Chapter |
|---|---|
| `01-scope-and-conventions.md` | 1. Scope and conventions |
| `02-format-word.md` | 2. Format word |
| `03-container-and-records.md` | 3. Container and records |
| `04-error-correction.md` | 4. Error correction |
| `05-geometry-and-placement.md` | 5. Geometry and placement |
| `06-payload-coding.md` | 6. Payload coding |
| `07-color.md` | 7. Colour profiles |
| `08-qr-bootstrap.md` | 8. QR bootstrap |
| `09-versions-and-registries.md` | 9. Versions and registries |
| `annex-a-test-vectors.md` | Annex A. Test vectors |
