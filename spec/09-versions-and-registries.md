# NMT Code — 9. Versions and registries

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft).

## 9.1 Specification version

The specification uses Semantic Versioning. While the major number is 0, a change that breaks existing symbols or readers raises the minor number, and an addition or correction raises the patch number. Version 1.0.0 has not been released, and no rule of this specification depends on it.

The specification version is not written into symbols. Symbols carry the **format version** (chapter 2), which changes only when the bit layout changes.

The first public release of this specification is the earlier of two points: the repository that holds it becomes public, or software that makes symbols is first released to the public, for example inside another product. From then on, symbols exist that readers must keep decoding. Until then, symbols made under different 0.x minor versions need not be interoperable: a decoding value of 9.5 may have changed between them, and nothing in a symbol says which one it used. From the first public release on, a symbol made under one 0.x version decodes to the same content in a reader of any later version within format version 0.

## 9.2 Format version

| Value | Meaning |
|---|---|
| 0 | This specification (0.x) |
| 1, 2 | Reserved |
| 3 | Reserved as the extended format version: a later version of this specification gives the actual format version in the container and defines its layout. This version defines none |

A reader that meets format version 1, 2 or 3 MUST reject the symbol, and SHOULD tell the user that a newer reader is needed, under the condition of chapter 2 (2.3).

Within format version 0, nothing listed in 9.4 changes after the first public release of this specification. Before that release, such a change raises the specification's minor number and is listed in the change log.

## 9.3 Registries

Each registry is append-only. An identifier, once published in a released version of this specification, keeps its meaning and its exact bytes for ever.

| Registry | Defined in | Identifier size |
|---|---|---|
| Codec IDs | chapter 6 | 4 bits in the container's lead byte; the value 15 means a LEB128 codec ID follows (chapter 3) |
| Dictionary IDs | chapter 6 | LEB128 in the container header, present only for codecs that take a dictionary (codecs 3 and 5); each ID is bound to the SHA-256 of its bytes |
| Content and record type IDs (one registry) | chapter 3 | LEB128 in the container header or record |
| Hash algorithm IDs | chapter 3 | as defined in chapter 3; each ID includes the digest length |
| QR bootstrap URL constant | chapter 8 | versioned constant; every value has the shape of 8.2 |

## 9.4 Parts that are fixed once released

Every normative rule of chapters 2 to 8 is fixed once released, except the parameters of 9.5. This includes in particular:

- the fields and widths of the format word, its code and its masks (chapter 2);
- the finder patterns, reference marks, placement order and whitening sequence (chapter 5);
- the Reed-Solomon field, generator polynomials and block-splitting algorithm (chapter 4);
- the container header layout, the format echo byte, the CRC-32C, the record forms, the ordering and presentation rules and the padding pattern (chapter 3);
- the codec algorithms and the constants of the range coder (chapter 6);
- the colour cell order, the reference cells and the colour layer layout (chapter 7);
- the bootstrap placement rules (chapter 8);
- every published registry entry (9.3), including the bytes of every published dictionary and the QR bootstrap URL constant of each published version.

## 9.5 Tunable parameters

These values are marked as tunable in their chapters. Each is listed in the change log when it changes.

- A value whose effect is "decoding" changes what existing symbols decode to, or whether they are valid. It may change only until the first public release of this specification; such a change raises the minor number (9.1). After that release it never changes: a different value takes a new codec ID or dictionary ID (chapter 6), a new colour-profile value (chapter 7) or a new format version.
- A value whose effect is "generator" or "policy" changes only what a generator makes by default, or what a reader recommends. It may change in any later version and raises the patch number.

| Parameter | Chapter | Value in 0.2 | Effect |
|---|---|---|---|
| `SEPARATOR_WIDTH` | 5.4 | 1 module | decoding |
| `TOKEN_TABLE_V0` | 6.8.2 | 240 tokens; serialisation SHA-256 `a8e0f1be…3e3717` | decoding |
| `MODEL0_WEIGHTS` | 6.8.3 | the class table of 6.8.3, total 65 536 | decoding |
| `HANGUL_SPACE_FINALS` | 6.9.2 | [0, 1, 4, 7, 8, 16, 17, 19, 20, 21, 23, 26] | decoding |
| `profile1_palette` | 7.4.1 | black, blue, yellow, white (`#000000`, `#0000FF`, `#FFFF00`, `#FFFFFF`) | decoding |
| `reference_modules_per_copy` | 7.5 | 32 | decoding |
| `colour_min_codewords` | 7.8.2 | 16 | decoding |
| `chroma_whitening_seed` | 7.8.4 | 0x644E9D0D | decoding |
| `colour_mismatch_share_max` | 7.9.3 | 1% of the data modules | decoding |
| `colour_default_gain_min` | 7.1 | 1.3 | policy |
| `chroma_k_min_c1` | 7.11 | 4 camera pixels per module | generator |
| `print_dark_L_max`, `print_light_L_min`, `print_class_db_min` | 7.10 | 40, 80, 30 | generator |
| `print_growth_dots` | 1.5 | default 0; RECOMMENDED 1 at 600 dpi for an inkjet printer on uncoated paper | generator |
| `qr_min_module` | 8.4.1 | 0.4 mm, or 4 CSS pixels when the pixel density is unknown | generator |

The error-correction level of the colour layer is not tunable: it is fixed at level 2 for colour profile 1 (chapter 7, 7.8.2), and a different level would take a new colour-profile value.

## 9.6 Borrowed and new parts

| Part | Borrowed from | What NMT Code does differently |
|---|---|---|
| Reed-Solomon over GF(2^8), systematic encoding | QR Code (ISO/IEC 18004:2024), Data Matrix | blocks are sized by an algorithm for any symbol size instead of a fixed table |
| Separate format information in two copies | QR Code | 28 data bits, any rectangular size up to 4108 modules per side; each copy has its own mask, and the container repeats the level and the profile |
| Corner finder patterns with distinct inner patterns | JAB Code (BSI TR-03137 Part 2, 2020; ISO/IEC 23634:2022) uses distinct finders per corner | square 5×5 finders at all four corners; any three suffice; no 1:1:3:1:1 run-length ratio |
| Reference marks at regular spacing | Aztec Code (ISO/IEC 24778) reference grid, QR Code alignment patterns | 3×3 marks at most 24 modules apart |
| Numeric and alphanumeric packing | QR Code modes | selected per content by the codec layer, next to general compression |
| Colour modules with calibration cells | JAB Code, HCC2D | a luminance base layer that a black-and-white reader always decodes; colour carries extension records only |
| Pre-shared compression dictionaries | Brotli (RFC 7932), shared Brotli (RFC 9841) | dictionary IDs bound to SHA-256 in an append-only registry, because printed codes cannot negotiate |
| Whitening by a fixed sequence | common in radio and storage codes | replaces QR Code's choice among eight masks |

## 9.7 Trademarks

"QR Code" is a registered trademark of DENSO WAVE INCORPORATED. NMT Code is not a QR Code and is not endorsed by DENSO WAVE.

## 9.8 Reader errors

Every condition under which a reader rejects a symbol, or presents less than the whole content. The names are stable identifiers for implementations and test vectors; the wording shown to a user is not part of this specification. "Outcome" uses the classes of chapter 3 (3.9); in each case the reader presents nothing from the symbol unless the outcome says otherwise. The order of the checks is given in chapter 2 (2.7) for the format word and in chapter 3 (3.9) for the container; a test vector names the first error a reader meets.

| Name | Section | Outcome | Meaning |
|---|---|---|---|
| `E_NESTED_SYMBOL` | 5.11 | Malformed | A detected symbol lies inside the area of another detected symbol; neither is presented |
| `E_FORMAT_UNREADABLE` | 2.3, 2.7, 5.11 | Damaged | No copy of the format word decodes within 2e + s ≤ 7 to a valid word (in format version 0, W or H of 16, colour profile 2 or 3, colour profile 0 with chroma cell size 1, a transfer tile with a colour profile, a colour layer below 16 codewords, or W and H that disagree with the finders count as not decoded), or the one decoded copy has format version 1 to 3 with 2e + s > 4, or the sampling grid does not confirm W and H (5.11 step 4) |
| `E_FORMAT_CONFLICT` | 2.7 | Damaged | Both copies of the format word decode, to different words |
| `E_FORMAT_VERSION` | 2.3, 9.2 | Unsupported | The format version is 1, 2 or 3, from both copies or from one copy with 2e + s ≤ 4; a newer reader is needed |
| `E_SIZE_LIMIT` | 2.7, 5.11 | Unsupported | W × H of the chosen format word exceeds the largest area this reader accepts |
| `E_ECC_FAILED` | 4.9 | Damaged | A base-layer Reed-Solomon block could not be corrected |
| `E_COLOUR_AMBIGUOUS` | 7.9.3 | Malformed | A reader that samples colour found more than `colour_mismatch_share_max` of the data modules in a colour outside the palette of the colour profile |
| `E_LENGTH_FIELD` | 3.2.3, 3.7 | Damaged | The body length Lb is not a valid LEB128, or places the CRC-32C beyond the message capacity |
| `E_CRC_MISMATCH` | 3.7 | Damaged | The CRC-32C of the container does not match |
| `E_CONTAINER_VERSION` | 3.2.2 | Unsupported | The container version is 1, 2 or 3 |
| `E_TILE_RESERVED_BITS` | 3.3 | Malformed | A transfer tile's reserved lead-byte bits are not 0 |
| `E_TRANSFER_UNSUPPORTED` | 2.3, 3.3 | Unsupported | The symbol is a transfer tile and the reader does not implement transfer |
| `E_FORMAT_ECHO` | 3.2.2 | Malformed | The format echo byte of a static container differs from the byte built from the chosen format word, reserved bits included |
| `E_COLOUR_FLAG` | 3.2.2 | Malformed | The container says X = 1 while the format word's colour profile is 0 |
| `E_LEB128` | 1.4, 3.9 | Malformed | A LEB128 field in the body is not minimal or is 2^32 or more |
| `E_CODEC_ESCAPE` | 3.2.4 | Malformed | The codec ID escape holds a value below 15 |
| `E_UNSUPPORTED_CODEC` | 6.2 | Unsupported | The codec ID is not assigned by the reader's specification version: reserved, assigned later, or 16 or more |
| `E_UNKNOWN_DICTIONARY` | 6.11 | Unsupported | The dictionary ID is not 0 and the reader does not carry it, including IDs of the reserved and private-use ranges |
| `E_DICTIONARY_MISMATCH` | 6.2, 6.11 | Malformed | The registered kind of the dictionary does not match the codec |
| `E_HASH_ID_INVALID` | 3.6 | Malformed | The hash algorithm ID is 0 or 3 to 6 |
| `E_UNKNOWN_HASH` | 3.6 | Unsupported | The hash algorithm ID is otherwise unknown to the reader |
| `E_HEADER_OVERRUN` | 3.9 | Malformed | The header fields run past the end of the body |
| `E_TOO_LARGE` | 6.4 | Unsupported | The decoded length L exceeds the reader's own limit |
| `E_MALFORMED` | 6.4 to 6.10 | Malformed | The decoded length exceeds the cap of its container (`MAX_STATIC_CONTENT_LEN_V0` in a static symbol), the coded field breaks its codec's rules or expansion bound, or decoding gives a length other than L |
| `E_RECORD_LIST` | 3.2.5 | Malformed | Record count 0, a record that runs past the end, or bytes left over |
| `E_ACTION_RULE` | 3.4.4 | Malformed | More than one action record in the base-layer message, or an action record that is not its first record |
| `E_DIGEST_MISMATCH` | 3.5, 7.9.2 | Malformed | The digest over base and extension records differs from the base container's digest; no record is presented, base records included |
| `E_EXTENSION_UNREAD` | 3.5, 7.9.1, 7.9.2 | Presented, base only | The colour layer was not read, failed error correction, failed a container check, or broke rule 2 or 4 of 3.4.4 |
| `E_BOOTSTRAP_MISMATCH` | 8.6 | Presented, with error | A QR symbol beside the NMT Code symbol differs from every known bootstrap URL and does not have the shape of 8.2; its content is never presented, and the NMT Code result MAY still be presented with the error |

Records that are presented as unknown data (chapter 3, 3.4.3 and 3.9) are not errors and have no name here. A bootstrap QR of the shape of 8.2 that is not a known value is not an error either (chapter 8, 8.6).

## 9.9 Change log

Changes to parts listed in 9.4 raise the minor number while the major number is 0 (9.1, 9.2). No version before the first public release is interoperable with another.

### 0.2 (draft), 2026-09-29

Symbols made under 0.1 do not decode under 0.2.

- Format word (chapter 2): copy B has its own mask, MASK_B. The decoder corrects errors and erasures (2e + s ≤ 7, s ≤ 4) and confirms the result against the full generator. Two copies that decode to different words reject the symbol (`E_FORMAT_CONFLICT`). Format version 3 is reserved as the extended format version.
- Container (chapter 3): a format echo byte follows the lead byte (`E_FORMAT_ECHO`). The colour checks run in a fixed order, and an extension message with an action or attribute record is ignored rather than rejecting the symbol. URL, file-name and hidden-character presentation rules are normative.
- Payload (chapter 6): the content of a static symbol is capped at 1 MiB, every codec has an expansion bound, codec 3 has one canonical termination, and a brotli stream declares at most the window `W_max` of 6.10.
- Geometry (chapter 5): a new bottom-right finder. The reader checks the size against the finders and the sampling grid (`E_SIZE_LIMIT`), and rejects nested symbols (`E_NESTED_SYMBOL`).
- Colour (chapter 7): luma is the only luminance, a module colour check rejects off-palette symbols (`E_COLOUR_AMBIGUOUS`), the reference cells moved, and printed colour uses 2 × 2 cells.
- Profiles (chapters 1 and 8): the recommended size keeps the sides within a factor of 2, and the bootstrap QR is on by default only for `print`.
- Errors: `E_LAYER_TOO_SMALL`, `E_NO_BASE_RECORD` and `E_TILE_COLOUR` are folded into `E_FORMAT_UNREADABLE`, `E_RECORD_LIST` and `E_FORMAT_UNREADABLE`.

