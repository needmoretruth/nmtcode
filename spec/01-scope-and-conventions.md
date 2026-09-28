# NMT Code — 1. Scope and conventions

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft). CC BY 4.0 grants no patent rights.

Preferred attribution: "NMT Code specification, version 0.2 (draft), by needmoretruth, https://github.com/needmoretruth/nmtcode, licensed under CC BY 4.0." State the changes you made, as CC BY 4.0 requires.

## 1.1 What NMT Code is

NMT Code (full name "needmoretruth code") is a two-dimensional optical code for cameras on current phones and computers. It has two uses:

1. A single code, printed or shown on a screen. It is designed to need less area than a QR Code for the same content, mainly for URLs, short text and Korean text. How much less depends on the content, the size and the error-correction level; it has been computed from the rules of this specification, not yet measured on camera captures.
2. A multi-frame transfer: a screen shows a sequence of codes and a camera receives a file, with no network, cable or pairing.

Area comparisons in this specification count the symbol and its quiet zone only, without the QR bootstrap of chapter 8; chapter 8 (8.5) gives the area with it. The QR Code they compare against encodes the exact same bytes, with its cheapest mode segmentation and no change of letter case, at the QR level with the same share of correctable bytes (chapter 4, 4.5).

This version of the specification defines the single code (chapters 2 to 8). The multi-frame transfer uses the same symbol in "transfer tile" mode; its chapter is added in a later 0.x version.

NMT Code does not replace QR Code and is not compatible with QR readers. A symbol may carry a separate, standard QR Code beside it that gives a QR reader a link to an NMT Code reader (chapter 8).

## 1.2 Design baseline

- Target devices: phones and computers with a camera of at least 1280×720 pixels and continuous or fixed focus, with a Samsung Galaxy S20 (2020) as the reference phone. Printers are a second target.
- A reader is expected to do more work per frame than a QR reader. The budget is one 1920×1080 frame decoded in about 25 ms on the reference phone.
- Deliberately not supported: scanners that read only QR Code or 1D barcodes; laser scanners; readers with less than about 2 camera pixels per module.
- Laptop webcams resolve few modules: a 720p webcam with a field of view of about 70° sees a 60 mm wide phone-screen symbol at 0.5 m with about 110 pixels, enough for about 55 modules at 2 camera pixels per module or 37 at 3 (arithmetic, not measured).

QR Code conventions that NMT Code drops:

| QR Code | NMT Code |
|---|---|
| 40 fixed versions, square only | any W × H with sides that are multiples of 4 from 20 to 4108 (chapter 2) |
| 4-module quiet zone | 2 modules (chapter 5, 5.2) |
| 3 finder patterns with the 1:1:3:1:1 ratio | 4 corner finders, any 3 of which suffice (chapter 5, 5.3) |
| a choice of 8 masks with penalty scoring | one fixed whitening sequence (chapter 5, 5.10) |
| largest symbol 177 × 177 modules | largest symbol 4108 × 4108 modules |
| mode segments in the data stream | codecs selected per content, including compression (chapter 6) |

## 1.3 Structure of a symbol

A symbol is a rectangle of W × H square modules surrounded by a quiet zone. W and H are multiples of 4 from 20 to 4108 (chapter 2).

| Layer | Carried by | Content | Chapter |
|---|---|---|---|
| Function patterns | fixed modules | four corner finders, reference marks | 5 |
| Format word | fixed modules, two copies | format version, symbol class, W, H, error-correction level, colour profile, chroma cell size | 2 |
| Base layer | luminance of data modules | error-corrected container: header with a copy of the format fields, base records, integrity check | 3, 4, 5, 6 |
| Colour layer (optional) | chroma of data cells | error-corrected extension records | 7 |

Every symbol has a base layer that a black-and-white reader decodes by itself. In a colour symbol, the base layer carries the records that every reader shows and a digest of the whole content. This split is provisional in 0.2.

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
| Variable-length integers | Unsigned LEB128, at most 5 bytes, value below 2^32. Byte i (from 0) holds bits 7i to 7i + 6 of the value; bit 7 of a byte is set when another byte follows. A decoder stops with an error at a fifth byte that has bit 7 set or a value above 0x0F, so it never shifts by 35 or more. A non-minimal encoding is one longer than one byte whose last byte is 0x00; a decoder MUST reject it |
| Lengths and offsets | Every comparison of a length or an offset against a buffer, such as `2 + size(Lb) + Lb + 4 ≤ K` (chapter 3, 3.2.3), is done without overflow: in integers of at least 64 bits, or as a subtraction from the bytes that remain after checking that enough remain. A 32-bit reader, for example one compiled to wasm32, must still reject Lb = 2^32 − 1 |
| Base-layer module value | Dark = 1, light = 0 |
| Polarity | A generator MUST draw dark modules darker than light modules, on a light quiet zone; it MUST NOT produce a reflectance-reversed symbol (light modules on dark). A reader decides polarity from the finder patterns (chapter 5, 5.11). It MAY also try the inverted image, and then reads it as if it had been captured dark on light |
| Luminance | A reader classifies the modules of the base layer by luma Y′ = 0.299 R′ + 0.587 G′ + 0.114 B′ of the sRGB-encoded values R′, G′, B′ in [0, 1] (the weights of ITU-R BT.601), and by no other measure: two measures that disagree on a module let one symbol decode to two contents (chapter 7, 7.9.3). Chapter 7 (7.9.3) defines the module colour check that a reader that samples colour applies |
| Coordinates | (x, y) in modules. Origin at the top-left module of the symbol, quiet zone excluded. x grows to the right, y grows downward |
| Size | W = width, H = height, in modules |
| Device pixel | One physical pixel of a display or one dot of a printer |
| CSS pixel | The reference pixel of CSS: 1/96 inch at the nominal viewing distance. A browser shows one CSS pixel as `devicePixelRatio` device pixels. An image with no stated density is taken as one image pixel per CSS pixel |
| CRC-32C | Polynomial 0x1EDC6F41 (Castagnoli), reflected input and output, initial value 0xFFFFFFFF, final XOR 0xFFFFFFFF, as in RFC 3720. The check value of the ASCII string "123456789" is 0xE3069283 |
| Hash | SHA-256 (FIPS 180-4) |
| Hexadecimal | `0x` prefix for single values. Byte strings are written as space-separated pairs, such as `4E 4D 54` |
| Spelling | The text writes "colour"; file names and the profile name `color` keep the spelling that software uses |

Terms used before the chapter that defines them:

| Term | Meaning | Defined in |
|---|---|---|
| Module | One square cell of the symbol grid, dark or light | 5.2 |
| Quiet zone | The light margin around the symbol | 5.2 |
| Function module | A module of a finder, a separator, a format copy or a reference mark | 5.2 |
| Data module | A module that is not a function module | 5.7 |
| Format word | The 47-bit word that gives the size, the level and the profile, written twice with a different mask in each copy | 2 |
| Codeword | One byte of a Reed-Solomon block | 4.2 |
| Message | The data bytes of one layer after error correction | 3.1 |
| Container | The structure at the start of a message: header, content, CRC-32C | 3.2 |
| Record | One item of content: a type ID and a value | 3.1 |
| Base records, extension records | The records of the base layer and of the colour layer | 3.5 |
| Whitening | The fixed bit sequence XORed onto the data modules | 5.10 |
| Chroma, cell | The colour swing of a module, and the c × c block of modules that carries one colour bit | 7.2 |
| Transfer tile | One symbol of a multi-frame transfer (symbol class 1) | 3.3 |
| Bootstrap QR | A standard QR Code beside the symbol that links to an NMT Code reader | 8 |

## 1.5 Profiles

A profile is a named set of defaults. All values a reader needs are stated in the symbol itself, so a reader does not need to know which profile made a symbol.

| Profile | Colour | Error-correction level | Module size default | Camera pixels per module k | Bootstrap QR (chapter 8) | Use |
|---|---|---|---|---|---|---|
| `screen` | black and white | 0 (recovers about 7.5% of bytes, like QR level L) | the smallest whole number of device pixels that is at least 0.4 mm and at least 4 device pixels; 4 CSS pixels when the pixel density is unknown | 3 | off | codes shown on a display |
| `print` | black and white | 1 (about 15%, like QR level M) | at least 0.4 mm, a whole number of printer dots, at least 4 dots | 3 | on | printed codes |
| `lowend` | black and white | 1 (about 15%, like QR level M) | large modules for readers at about 2 camera pixels per module | 2 | off | cameras with low resolution or fixed focus |
| `color` | profile 1 of chapter 7 | 0 | as `screen` | 4 with 1 × 1 chroma cells, as `screen` with 2 × 2 cells | off | experimental in 0.2; print uses 2 × 2 chroma cells (chapter 7, 7.10) |

k is the number of camera pixels a receiver needs per module along each axis. The value 2 of `lowend` is at the sampling limit of a camera; measurement decides whether it holds.

A generator MUST let the user change the size, the error-correction level, the colour profile, the quiet zone (2 modules or more) and the module size, and MUST let the user turn the bootstrap QR on and off.

Size selection is a generator choice, not part of the format; a reader accepts any valid size. The RECOMMENDED default:

1. The candidates are the valid sizes whose message capacity K (chapter 4, 4.6) holds the container and whose sides are within a factor of 2 of each other (W ≤ 2H and H ≤ 2W).
2. Pick the candidate with the smallest area W × H. Among equal areas pick the one closest to square (the smallest max(W, H) / min(W, H)), then the one with the larger H.
3. User constraints override the window of step 1:
   - A maximum width or height removes the sizes beyond it. If no size within the maxima both holds the container and has its sides within a factor of 2, every size within the maxima that holds the container is a candidate.
   - An aspect ratio a:b replaces the window: a size meets it when its longer side is the multiple of 4 nearest to its shorter side times the ratio (for a ≥ b, W nearest to H · a / b; otherwise H nearest to W · b / a; an exact half rounds up). Among equal areas pick the one closest to the ratio, then the one with the larger H.

When the display resolution or the receiving camera is known, the RECOMMENDED upper bound on each side, in modules, is min(display pixels in that direction ÷ 2, 0.9 × the camera's shorter side in pixels ÷ k): every module then gets at least 2 display pixels, and the symbol fits in 90% of the camera's shorter side at k camera pixels per module.

Rendering rules:

- A screen image SHOULD keep every module on whole device pixels: size the image in device pixels (CSS size = modules × pixels per module ÷ `devicePixelRatio`) and scale it with nearest-neighbour sampling (`image-rendering: pixelated`), or draw it as SVG with `shape-rendering="crispEdges"`. Smooth scaling makes module widths uneven and edges grey.
- A generator of print output SHOULD put one image sample per module or draw vector shapes, SHOULD write PDF images with `/Interpolate false`, and SHOULD scale the output to a whole number of printer dots per module. A generator cannot verify what a driver does, so a reader does not depend on it.
- Black-and-white print output SHOULD use black ink only (DeviceGray, or K alone in DeviceCMYK), not composite black: misregistered colour inks put coloured fringes on dark modules.
- `print_growth_dots` (a generator value of chapter 9, 9.5; default 0): ink spread on paper grows dark areas from every side. With a value k ≥ 1 the generator shrinks the union of all dark modules of its output, bootstrap QR included, by k printer dots on every edge that faces a light module. Edges between two dark modules do not move, so no light seam appears between them; a single dark module loses k dots on each side. The result is the erosion of the dark pixels by a square of (2k + 1) × (2k + 1) dots. 2k MUST be less than the module size in dots. RECOMMENDED: 1 at 600 dpi for an inkjet printer on uncoated paper; 0 otherwise until measured.

## 1.6 Conformance

- A **generator** conforms if every symbol it produces follows chapters 2 to 6, and chapter 7 or 8 when it uses a colour profile or a QR bootstrap.
- A **reader** conforms if it decodes every conforming black-and-white symbol, rejects every symbol that fails a check this specification defines, and never presents data that failed a check as a result. It implements codecs 0 to 5 (chapter 6) and every dictionary registered up to the specification version it states (chapter 6, 6.11).
- A reader that supports only black and white conforms. When it reads a colour symbol it MUST present the base records (chapter 3). This rule is provisional in 0.2.

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

The chapters follow the structure of a symbol, not the order of work. An encoder implementer reads 6, 3, 4, 5 and 2; a decoder implementer reads 5, 2, 4, 3 and 6, then 7 for colour. Chapter 9 collects the registries, the tunable parameters and the reader errors.
