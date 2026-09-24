# NMT Code — 9. Versions and registries

Specification version 0.1 (draft).

## 9.1 Specification version

The specification uses Semantic Versioning. While the major number is 0, a change that breaks existing symbols or readers raises the minor number, and an addition or correction raises the patch number. Version 1.0.0 has not been released.

The specification version is not written into symbols. Symbols carry the **format version** (chapter 2), which changes only when the bit layout changes.

## 9.2 Format version

| Value | Meaning |
|---|---|
| 0 | This specification (0.x) |
| 1–3 | Reserved. A reader that meets a reserved value MUST reject the symbol and SHOULD tell the user that a newer reader is needed |

Within format version 0, a change to anything listed in 9.4 is not allowed after the specification reaches 1.0.0. Before 1.0.0, such a change raises the specification's minor number and is listed in the change log.

## 9.3 Registries

Each registry is append-only. An identifier, once published in a released version of this specification, keeps its meaning and its exact bytes for ever.

| Registry | Defined in | Identifier size |
|---|---|---|
| Codec IDs | chapter 6 | 4 bits in the container's lead byte; the value 15 means a LEB128 codec ID follows (chapter 3) |
| Dictionary IDs | chapter 6 | LEB128 in the container header; each ID is bound to the SHA-256 of its bytes |
| Content and record type IDs (one registry) | chapter 3 | LEB128 in the container header or record |
| Hash algorithm IDs | chapter 3 | as defined in chapter 3; each ID includes the digest length |
| QR bootstrap URL constant | chapter 8 | versioned constant |

## 9.4 Parts that are fixed once released

- The fields and widths of the format word, its code and its masking constant (chapter 2).
- The finder patterns, reference marks, placement order and whitening sequence (chapter 5).
- The Reed-Solomon field, generator polynomials and block-splitting algorithm (chapter 4).
- The container header layout and CRC-32C (chapter 3).
- Every published registry entry (9.3), including the bytes of every published dictionary.
- The QR bootstrap URL constant for each published version (chapter 8).

## 9.5 Parts that may be tuned before 1.0.0

Values marked "tunable in 0.x, fixed before 1.0" in chapters 2 to 8. Each such value is listed in the change log when it changes.

## 9.6 Borrowed and new parts

| Part | Borrowed from | What NMT Code does differently |
|---|---|---|
| Reed-Solomon over GF(2^8), systematic encoding | QR Code (ISO/IEC 18004:2024), Data Matrix | blocks are sized by an algorithm for any symbol size instead of a fixed table |
| Separate format information in two copies | QR Code | 28 data bits, any rectangular size up to 4108 modules per side |
| Corner finder patterns with distinct inner patterns | JAB Code (BSI TR-03137 Part 2, 2020; ISO/IEC 23634:2022) uses distinct finders per corner | square 5×5 finders at all four corners; any three suffice; no 1:1:3:1:1 run-length ratio |
| Reference marks at regular spacing | Aztec Code (ISO/IEC 24778) reference grid, QR Code alignment patterns | 3×3 marks every 24 modules |
| Numeric and alphanumeric packing | QR Code modes | selected per content by the codec layer, next to general compression |
| Colour modules with calibration cells | JAB Code, HCC2D | a luminance base layer that a black-and-white reader always decodes; colour carries extension records only |
| Pre-shared compression dictionaries | Brotli (RFC 7932), shared Brotli (RFC 9841) | dictionary IDs bound to SHA-256 in an append-only registry, because printed codes cannot negotiate |
| Whitening by a fixed sequence | common in radio and storage codes | replaces QR Code's choice among eight masks |

## 9.7 Trademarks

"QR Code" is a registered trademark of DENSO WAVE INCORPORATED. NMT Code is not a QR Code and is not endorsed by DENSO WAVE.
