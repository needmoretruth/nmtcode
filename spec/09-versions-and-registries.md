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
| Dictionary IDs | chapter 6 | LEB128 in the container header, present only for codecs that take a dictionary (codecs 3 and 5); each ID is bound to the SHA-256 of its bytes |
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

These values are marked "tunable in 0.x, fixed before 1.0" in their chapters. Each is listed in the change log when it changes. A change to a value whose effect is "decoding" changes what existing symbols decode to, or whether they are valid, and raises the minor number (9.1). A change to a value whose effect is "generator" or "policy" raises the patch number.

| Parameter | Chapter | Value in 0.1 | Effect |
|---|---|---|---|
| `SEPARATOR_WIDTH` | 5.4 | 1 module | decoding |
| `TOKEN_TABLE_V0` | 6.8.2 | 240 tokens; serialisation SHA-256 `a8e0f1be…3e3717` | decoding |
| `MODEL0_WEIGHTS` | 6.8.3 | the class table of 6.8.3, total 65 536 | decoding |
| `HANGUL_SPACE_FINALS` | 6.9.2 | [0, 1, 4, 7, 8, 16, 17, 19, 20, 21, 23, 26] | decoding |
| `profile1_palette` | 7.4.1 | black, blue, yellow, white (`#000000`, `#0000FF`, `#FFFF00`, `#FFFFFF`) | decoding |
| `reference_modules_per_copy` | 7.5 | 32 | decoding |
| `colour_min_codewords` | 7.8.2 | 16 | decoding |
| `chroma_ecc_level` | 7.8.2 | 2 | decoding |
| `chroma_whitening_seed` | 7.8.4 | 0x644E9D0D | decoding |
| `colour_default_gain_min` | 7.1 | 1.3 | policy |
| `chroma_k_min_c1` | 7.11 | 4 camera pixels per module | generator |
| `print_dark_L_max`, `print_light_L_min`, `print_class_db_min` | 7.10 | 40, 80, 30 | generator |
| `qr_min_module` | 8.4.1 | 0.4 mm, or 4 device pixels when the pixel density is unknown | generator |

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

## 9.8 Reader errors

Every condition under which a reader rejects a symbol, or presents less than the whole content. The names are stable identifiers for implementations and test vectors; the wording shown to a user is not part of this specification. "Outcome" uses the classes of chapter 3 (3.9); in each case the reader presents nothing from the symbol unless the outcome says otherwise.

| Name | Section | Outcome | Meaning |
|---|---|---|---|
| `E_FORMAT_UNREADABLE` | 2.3, 2.7 | Damaged | No copy of the format word decodes within 3 bit errors to valid fields (in format version 0, W or H of 16, colour profile 2 or 3, or colour profile 0 with chroma cell size 1 count as not decoded) |
| `E_FORMAT_VERSION` | 2.3, 9.2 | Unsupported | The format version is 1, 2 or 3; a newer reader is needed |
| `E_TRANSFER_UNSUPPORTED` | 2.3, 3.3 | Unsupported | The symbol is a transfer tile and the reader does not implement transfer |
| `E_ECC_FAILED` | 4.9 | Damaged | A base-layer Reed-Solomon block could not be corrected |
| `E_LAYER_TOO_SMALL` | 4.6, 7.8.2 | Malformed | A layer has fewer codewords than its minimum; only a colour layer with N_c < 16 can meet this, since every allowed size has a base layer of at least 20 codewords |
| `E_LENGTH_FIELD` | 3.2.3, 3.7 | Damaged | The body length Lb is not a valid LEB128, or places the CRC-32C beyond the message capacity |
| `E_CRC_MISMATCH` | 3.7 | Damaged | The CRC-32C of the container does not match |
| `E_CONTAINER_VERSION` | 3.2.2 | Unsupported | The container version is 1, 2 or 3 |
| `E_TILE_RESERVED_BITS` | 3.3 | Malformed | A transfer tile's reserved lead-byte bits are not 0 |
| `E_TILE_COLOUR` | 3.3, 7.3 | Malformed | A transfer tile has a colour profile other than 0 |
| `E_COLOUR_FLAG` | 3.2.2 | Malformed | The container says C = 1 while the format word's colour profile is 0 |
| `E_LEB128` | 1.4, 3.9 | Malformed | A LEB128 field in the body is not minimal or is 2^32 or more |
| `E_CODEC_ESCAPE` | 3.2.4 | Malformed | The codec ID escape holds a value below 15 |
| `E_UNSUPPORTED_CODEC` | 6.2 | Unsupported | The codec ID is reserved, 16 or more, or not implemented by the reader |
| `E_UNKNOWN_DICTIONARY` | 6.11 | Unsupported | The dictionary ID is not 0 and the reader does not carry it, or it is in the private-use range |
| `E_DICTIONARY_MISMATCH` | 6.2, 6.11 | Malformed | The registered kind of the dictionary does not match the codec |
| `E_HASH_ID_INVALID` | 3.6 | Malformed | The hash algorithm ID is 0 or 3 to 6 |
| `E_UNKNOWN_HASH` | 3.6 | Unsupported | The hash algorithm ID is otherwise unknown to the reader |
| `E_HEADER_OVERRUN` | 3.9 | Malformed | The header fields run past the end of the body |
| `E_TOO_LARGE` | 6.4 | Unsupported | The decoded length L exceeds the reader's own limit |
| `E_MALFORMED` | 6.4 to 6.10 | Malformed | The decoded length exceeds `MAX_CONTENT_LEN_V0`, the coded field is invalid for its codec, or decoding gives a length other than L |
| `E_RECORD_LIST` | 3.2.5 | Malformed | Record count 0, a record that runs past the end, or bytes left over |
| `E_NO_BASE_RECORD` | 3.5 | Malformed | C = 1 and the base layer holds no record |
| `E_ACTION_RULE` | 3.4.4 | Malformed | More than one action record, or an action record that is not the first base record |
| `E_DIGEST_MISMATCH` | 3.5, 7.9.2 | Malformed | The digest over base and extension records differs from the base container's digest; no record is presented, base records included |
| `E_EXTENSION_UNREAD` | 3.5, 7.9 | Presented, base only | The colour layer was not read, failed error correction, failed its CRC-32C, or is unsupported or malformed |
| `E_BOOTSTRAP_MISMATCH` | 8.6 | Error reported | A QR symbol beside the NMT Code symbol differs from every known bootstrap URL; its content is never presented, and the NMT Code result MAY still be presented with the error |

Records that are presented as unknown data (chapter 3, 3.4.3 and 3.9) are not errors and have no name here.
