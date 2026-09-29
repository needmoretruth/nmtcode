# NMT Code — 3. Container and records

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft).

This chapter defines the bytes that the error-corrected base layer carries, the extension message of a colour symbol, the records inside them, and the checks a reader applies.

## 3.1 Terms

| Term | Meaning |
|---|---|
| Message | The data bytes of one layer after error-correction decoding, in order. The base-layer message is defined by chapter 4; the colour-layer message by chapter 7 |
| Message capacity K | The number of bytes in a message: K of chapter 4 (4.6) for the base layer, K_c of chapter 7 (7.8.2) for the colour layer. It follows from W, H, the error-correction level and the colour profile |
| Container | The structure at the start of a message: lead byte, format echo byte (static symbols), body length, body, CRC-32C |
| Format echo byte | The byte after the lead byte of a static container that repeats fields of the format word (3.2.2) |
| Body | The header fields after the length field, followed by the content |
| Content | The codec output (chapter 6), Lc bytes, called the coded field in chapter 6. Decoding it gives the decoded content |
| Body length Lb | The number of bytes in the body (field 3) |
| Decoded length L | The number of bytes of the decoded content. Written in field 6 when the codec is not 0; equal to Lc when the codec is 0 |
| Record | One item of decoded content: a type ID and a value |
| Canonical record | `type ID (LEB128) ‖ value length (LEB128) ‖ value`, computed over the decoded value |

All multi-byte integers in this chapter are LEB128 as defined in chapter 1 (1.4), except the CRC-32C (3.7) and the transfer-tile fields (3.3), which are fixed-width, most significant byte first. Every comparison of a length or an offset against the bytes that remain is done without overflow (chapter 1, 1.4).

LEB128 byte order, for clarity: the first byte holds the least significant 7 bits; bit 7 (0x80) of a byte is set when another byte follows. Examples: 0 = `00`, 127 = `7F`, 128 = `80 01`, 300 = `AC 02`, 16384 = `80 80 01`, 2^32 − 1 = `FF FF FF FF 0F`.

## 3.2 Container of a static symbol

The format word's symbol class (chapter 2) selects the layout: class 0 (static) uses this section, class 1 (transfer tile) uses 3.3. The container has no bit of its own for this, so the two cannot disagree.

### 3.2.1 Field order

| # | Field | Size | Present when |
|---|---|---|---|
| 1 | Lead byte | 1 byte | always |
| 2 | Format echo byte | 1 byte | always |
| 3 | Body length Lb | LEB128 | always |
| 4 | Codec ID escape | LEB128 | codec field of the lead byte = 15 |
| 5 | Dictionary ID | LEB128 | the codec takes a dictionary: codecs 3 and 5 (chapter 6, 6.2) |
| 6 | Decoded length L | LEB128 | codec ID ≠ 0 |
| 7 | Hash algorithm ID | LEB128 | X = 1 |
| 8 | Digest | length given by the hash algorithm ID (3.6) | X = 1 |
| 9 | Content type ID | LEB128 | R = 0 |
| 10 | Record count | LEB128 | R = 1 |
| 11 | Content | the rest of the body | always (may be empty) |
| 12 | CRC-32C | 4 bytes | always |
| 13 | Padding | to the message capacity | when bytes remain |

Fields 4 to 11 form the body. Lb is the number of bytes in the body. The content length Lc = Lb minus the sizes of fields 4 to 10 that are present.

Layout rule for every container version, present and future: in a static symbol, byte 0 is the lead byte, byte 1 is the format echo byte, the next bytes are Lb as LEB128, and the 4 bytes after the body are the CRC-32C of 3.7. In a transfer tile (3.3), Lb follows the lead byte directly. A reader can therefore check the CRC of a container whose version it does not support.

### 3.2.2 Lead byte and format echo byte

Lead byte:

| Bits | Name | Width | Values |
|---|---|---|---|
| 7–6 | Container version | 2 | 0 = this chapter. 1–3 reserved |
| 5 | R, record form | 1 | 0 = single-record form (field 9 present). 1 = record-list form (field 10 present) |
| 4 | X, colour extension | 1 | 0 = all records are in this message. 1 = a colour extension message exists (3.5); fields 7 and 8 are present |
| 3–0 | Codec | 4 | 0–14 = codec ID (chapter 6). 15 = the codec ID is in field 4 |

X = 1 is allowed only when the format word's colour profile is not 0.

A colour symbol with X = 0 carries all its records in the base layer. Chapter 7 defines what its colour layer then carries.

Format echo byte:

| Bits | Field | Value |
|---|---|---|
| 7–6 | Error-correction level | d[4..3] of the format word (chapter 2, 2.2) |
| 5–4 | Colour profile | d[2..1] |
| 3 | Chroma cell size | d[0] |
| 2 | Symbol class | d[25]; 0 in every static container |
| 1–0 | Reserved | 0 |

For example, the echo byte is `00` for a black-and-white symbol at level 0, `40` at level 1, and `10` for colour profile 1 at level 0 with 1 × 1 cells.

- A generator writes the byte from the format word it writes. The extension message of a colour symbol (3.5) carries the same byte as the base-layer message.
- A reader builds the byte from the chosen format word (chapter 2, 2.7) and compares the whole byte, reserved bits included, after the CRC-32C has passed and before it reads any field after Lb. A difference makes the container malformed (`E_FORMAT_ECHO`).

Reason: chapter 4 splits the base layer into the same blocks at every level (4.6), so the first bytes of the message are the same codeword bytes whichever level a reader assumes. A symbol whose two format copies claimed different levels could otherwise be built so that its base layer passes every check under both levels and gives two contents; a reader that sees only one copy could not tell. The echo byte sits in those shared bytes, so a reading under any level other than the one the symbol was made with fails. The colour profile, the chroma cell size and the symbol class are bound the same way. W and H are not repeated. A wrong size moves every codeword, so an honest symbol read at a wrong size fails its CRC-32C. A symbol built on purpose to pass at two sizes is caught only by the exact size check of chapter 5 (5.11, step 4), which compares W and H with the sampling grid.

### 3.2.3 Body length Lb

- Range: 0 to 2^32 − 1. It MUST satisfy `2 + size(Lb) + Lb + 4 ≤ K`, where size(Lb) is the number of bytes of the LEB128 encoding of Lb. The reader evaluates this without overflow (chapter 1, 1.4), so Lb = 2^32 − 1 is rejected by a 32-bit reader as well.
- The reader uses Lb to find the CRC-32C. The reader MUST NOT interpret fields 4 to 11 before the CRC-32C has passed.

### 3.2.4 Codec, dictionary and decoded length

- Codec ID: from the lead byte when it is 0–14. When the lead byte says 15, field 4 holds the codec ID, and its value MUST be 15 or more. A smaller value makes the container malformed. The value 15 itself is reserved (chapter 6, 6.2), so it is unsupported like every value the reader's version does not assign.
- A reader that does not know the codec ID stops here with the outcome Unsupported (3.9): it cannot know whether fields 5 and 6 are present.
- Dictionary ID: present if and only if chapter 6 (6.2) marks the codec as taking a dictionary. In this version codecs 3 (short-text model) and 5 (brotli) do. Value 0 means the codec's built-in model or dictionary. Other values name an entry of the dictionary registry (chapter 6, 6.11).
- Decoded length L: present if and only if the codec ID is not 0. For codec 0, L = Lc. Before allocating, the reader MUST check L ≤ `MAX_STATIC_CONTENT_LEN_V0` (1 MiB, chapter 6, 6.4) and L ≤ its own limit, and the codec's own bounds of chapter 6 (6.4). For codec 0 the same checks apply to L = Lc. Decoding MUST produce exactly L bytes; any other length makes the container malformed.
- The codec decodes the whole content into the decoded content. No codec of this version reads the content type ID. In record-list form the decoded content is a list of canonical records.

### 3.2.5 Record form

- Single-record form (R = 0): field 9 is the content type ID of the one record. The decoded content is that record's value. A value of length 0 is allowed.
- Record-list form (R = 1): field 10 is the record count, 1 or more. The decoded content is exactly that many canonical records, one after another, with no bytes left over. A count of 0, a record that runs past the end, or bytes left over make the container malformed.
- A generator uses single-record form when the message holds exactly one record and record-list form when it holds two or more, so that the same records always give the same bytes. A reader accepts both forms for one record.

### 3.2.6 Header size

The header is the lead byte, the format echo byte, Lb and fields 4 to 10. For single-record form, codec 0, a content type ID below 128 and no colour extension, the header is 4 bytes when the content is at most 126 bytes, and 5 bytes when it is 127 to 16 382 bytes.

| Case | Header bytes |
|---|---|
| 40-byte URL, codec 0, single-record form (example 3.10 a) | 4 (`00 00 29 02`) |
| Same URL, codec 3, dictionary 0 (example 3.10 f) | 6 (`03 00 16 00 28 02`) |
| 200-byte text, codec 0 | 5 |
| Codec 1, 2 or 4 | + size of L (1 byte below 128, 2 bytes below 16 384) |
| Codec 3 or 5 | + size of L + size of the dictionary ID (1 byte below 128) |
| Codec ID 15 or more | +1 or more |
| Colour extension (hash ID 1) | +33 |
| Record-list form | as single-record form in the header; each record adds its type ID and value length (2 bytes for type IDs below 128 and values below 128 bytes) |

The CRC-32C adds 4 bytes to every container. The smallest container, codec 0 with one empty record, is 8 bytes.

## 3.3 Container of a transfer tile

Provisional, defined by the transfer chapter. This section fixes the default layout so that chapter 4 and chapter 5 can size a tile. The transfer chapter, added in a later 0.x version, may change every field of this section except the layout rule of 3.2.1.

| # | Field | Size | Meaning |
|---|---|---|---|
| 1 | Lead byte | 1 byte | bits 7–6: container version (0). Bits 5–0: reserved, MUST be 0 |
| 2 | Body length Lb | LEB128 | bytes from field 3 to the end of field 8 |
| 3 | Session ID | 32 bits | chosen by the sender for one transfer; tells concurrent transfers apart |
| 4 | Frame counter | 16 bits | display-frame number modulo 2^16; a changed value marks a new frame |
| 5 | Tile index | 8 bits | position of this tile in the frame, 0–255 |
| 6 | Block number | 16 bits | source block of the erasure code |
| 7 | Erasure-symbol index | 16 bits | index of the transport symbol within the block |
| 8 | Payload | the rest of the body | one transport symbol |
| 9 | CRC-32C | 4 bytes | as 3.7 |
| 10 | Padding | to the message capacity | as 3.8 |

Fields 3 to 7 take 11 bytes, so a tile header is 12 + size(Lb) bytes before the CRC-32C: 13 bytes when Lb is below 128. This provisional layout has no format echo byte; the transfer chapter decides whether a tile carries one. The codec, dictionary, content type, file name and whole-file digest of a transfer are carried by the transfer manifest of the transfer chapter, not by each tile.

In format version 0 a transfer tile has colour profile 0: the format word of a tile with another colour profile is not decoded (chapter 2, 2.3), so every reader treats such a symbol the same way. A reader checks a tile in this order: Lb and the CRC-32C (Damaged), the container version (Unsupported), the reserved lead-byte bits (Malformed). A reader that does not implement transfer then reports the tile as unsupported (`E_TRANSFER_UNSUPPORTED`).

## 3.4 Records

### 3.4.1 Encoding

In record-list form, and for the digest of 3.5, a record is its canonical record: type ID (LEB128), value length (LEB128), value. The type ID is a content type ID from 3.4.2. Record type IDs and content type IDs are one registry.

### 3.4.2 Content type registry

Append-only (chapter 9). Role "data" is shown to the user; role "action" may also be offered as an action; role "attribute" describes the next record.

| ID | Name | Media type | Role | Value | Never compress |
|---|---|---|---|---|---|
| 0 | Unspecified bytes | — | data | any bytes | no |
| 1 | Text | `text/plain; charset=utf-8` | data | UTF-8 text (RFC 3629) | no |
| 2 | URL | — | action | a URI (RFC 3986) or IRI (RFC 3987), UTF-8 | no |
| 3 | JSON | `application/json` | data | UTF-8 JSON text (RFC 8259) | no |
| 4 | File | `application/octet-stream` | data (file) | the bytes of a file | no |
| 5 | File name | — | attribute | UTF-8 name of the file in the next record | no |
| 6 | PSBT | — | data (file) | a partially signed Bitcoin transaction (BIP-174), starting with `70 73 62 74 FF` | no |
| 7 | COSE_Sign1 envelope (reserved) | `application/cose; cose-type="cose-sign1"` | data (file) | a COSE_Sign1 structure (RFC 9052) | yes |
| 8 | age file (reserved) | — | data (file) | an age v1 file (C2SP age specification) | yes |
| 9 and up | unassigned | | | | |

"Reserved" for IDs 7 and 8: the ID is assigned now, and signature checking or decryption by the reader is left to a later version. In this version a reader presents them as files.

"Never compress": a generator MUST use codec 0 for a container that holds such a record, because ciphertext and signed bytes do not compress. This rule comes before the codec selection rule of chapter 6 (6.3). Readers do not check it.

### 3.4.3 Presentation by type

Rules for every record:

- **Hidden characters.** A reader shows these characters of a text value as visible escapes (for example `\u{202E}`), never as themselves: the C0 and C1 control characters (U+0000 to U+001F, U+007F to U+009F), except line feed and tab in types 1 and 3; the characters that change the direction of text (U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069); and the invisible characters U+00AD, U+180E, U+200B to U+200D, U+2060 to U+2064 and U+FEFF.
- **No claim of safety.** A reader MUST NOT describe any content as verified, authentic, trusted or safe on the basis of the CRC-32C or the digest (3.5, 3.7). Neither is a signature.
- **Size.** A reader SHOULD cap the amount of text it shows at once and offer the whole value as a file. It never drops part of a value silently.

| ID | Reader presents |
|---|---|
| 0 | as bytes (for example hexadecimal), available to copy or save |
| 1, 3 | as text, with the hidden characters escaped. The reader MUST NOT turn any part of the text into a link or an action: no automatic links for URLs, phone numbers or e-mail addresses, and no acting on payload patterns such as `WIFI:`, `otpauth://`, `BEGIN:VCARD` or `bitcoin:` inside the text. Only a type 2 record is an action (3.4.4). JSON is shown as text, or parsed for display with a limit on its nesting depth. If the value is not valid UTF-8, as type 0 with a notice |
| 2 | by the URL rules below. If the value is not valid UTF-8, as type 0 with a notice, and never offered to open |
| 4, 6, 7, 8 | as a file offered for saving. The reader MUST NOT open or run it. Type 6 MAY also be handed to a wallet application, but only when the user asks for it after the file has been shown; the hand-off is not an action record |
| 5 | as the name of the file that follows, by the file-name rules below. If the value is not valid UTF-8, as type 0 |
| unknown | as unknown data: the type ID, the length, and the bytes available to copy or save. Never acted on |

URL rules (type 2). The reader:

1. Parses the value with the URL parser of the WHATWG URL Standard (https://url.spec.whatwg.org/). If parsing fails, it presents the value as text with a notice and does not offer to open it.
2. Shows the complete text, with the hidden characters escaped (line feed and tab included). A long URL is wrapped, never shortened in the middle; the reader MAY show its start and let the user expand the rest, provided the host stays visible.
3. Shows the host on its own line. When mixed-script or confusable detection by Unicode Technical Standard #39 flags the host, the reader also shows its A-label (the punycode form of RFC 5891) next to it. A reader MAY always show both.
4. Shows a warning when the URL has userinfo (a user name or password before `@` in the authority).
5. MAY offer to open it, only after a user action and only after the complete text has been shown. It offers `https`; `http` with a notice that the connection is not encrypted; and `mailto`, `tel`, `sms` and `geo` with a confirmation that names the action. It MUST NOT offer `javascript`, `data`, `file`, `blob`, `vbscript`, `intent` or `content`. Any other scheme MAY be offered only with a confirmation that names the scheme and the application that would open it.

File-name rules (type 5). The reader never uses the value as a path. It builds the name it offers by these steps, in this order:

1. Keep the part after the last `/` or `\`.
2. Remove the hidden characters listed above, every control character included.
3. Replace each of `:` `*` `?` `"` `<` `>` `|` by `_`.
4. Remove leading dots, then trailing dots and spaces.
5. Put `_` before a name whose part before the first `.` is a Windows device name: `CON`, `PRN`, `AUX`, `NUL`, `COM0` to `COM9` or `LPT0` to `LPT9`, in any mix of case.
6. Cut the name to at most 255 bytes of UTF-8, at a character boundary.

An empty result means the reader chooses a name of its own. The reader SHOULD show the final extension of the name separately before saving, and MUST NOT choose an application by the extension. For example, the value `invoice` U+202E `fdp.exe`, which a screen shows as `invoiceexe.pdf`, becomes `invoicefdp.exe`, and `../.bashrc` becomes `bashrc`.

PSBT recognition: a reader MAY present a type 0 or type 4 value that starts with `70 73 62 74 FF` as a PSBT. A type 6 value that does not start with these bytes is presented as type 4 with a notice. A generator SHOULD use type 6 for a PSBT.

Unknown data is always presented and never silently dropped. This applies to unknown type IDs, to values that fail their type's check, and to attribute records without a target.

### 3.4.4 Ordering rules

The rules count records by their type ID, whether or not the value passes its type's check.

1. The base-layer message holds at most one action record (type 2), and only as its first record. More than one action record in the base-layer message, or one in any other position of it, makes the symbol malformed (`E_ACTION_RULE`). Reason: with two actions, different readers could offer different ones.
2. The extension message (3.5) holds no action record and no attribute record (type 5). Either record anywhere in it makes the extension invalid: every reader then presents the base records only (3.5, rule 4). Reason: the extension is content that only colour readers show, so it must not name or act on anything.
3. A file name record (type 5) MUST be followed directly, in the same message, by a record of type 0, 4, 6, 7 or 8, and a file record has at most one file name. A file name record that is the last record of its message has no target. A file name record that breaks this rule is presented as unknown data; the symbol is not rejected.
4. When the symbol has an extension message, the last base record MUST NOT be an attribute record: its target would be in the other message. If it is, the extension is invalid (3.5, rule 4), and the record is presented as unknown data by rule 3.
5. The reader presents records in order: base records first, then extension records. It MUST show the extension records apart from the base records and label them as content in colour that other readers may not show.

## 3.5 Colour symbols

When X = 1, the content is split over two messages.

- Base-layer message: the container of 3.2 with fields 7 and 8. Its records are the base records; it holds at least one (3.2.5). Every reader MUST present the base records, whether or not it reads colour.
- Colour-layer message (the extension message): the container of 3.2 with X = 0, the same format echo byte as the base-layer message, and its own CRC-32C and padding. Its records are the extension records. Chapter 7 defines how the colour layer carries it and its message capacity.
- Digest: SHA-256 over `d ‖ LEB128(number of base records) ‖ canonical base records ‖ LEB128(number of extension records) ‖ canonical extension records`, where d is the 28-bit data word of the format word (chapter 2, 2.2) written as 4 bytes, most significant first. The records are taken over their decoded values, in record order. The digest does not depend on the codec or the record form used. The two counts fix where the base records end, so a record cannot move between the messages under the same digest, and d binds the colour profile and the chroma cell size.

Reader rules, applied in this order:

1. **Base message.** The base container MUST pass every check of 3.9, and the base records rules 1 and 3 of 3.4.4, before anything is presented. A failure has the outcome of its check, and nothing from the symbol is presented.
2. **Extension container.** The reader presents the base records only, and MUST state that the symbol holds further content it could not read (`E_EXTENSION_UNREAD`), when any of these happens: it does not read colour; colour-layer error correction fails; the extension container fails a check of 3.9, its format echo byte included.
3. **Digest.** Otherwise it computes the digest. If the digest differs from field 8, the reader MUST reject the whole symbol and present no record, base records included (`E_DIGEST_MISMATCH`). Reason: the colour layer then decoded to content that the symbol did not commit to, through a miscorrection that the CRC missed or through a symbol built so that readers disagree.
4. **Ordering over both messages.** If the extension message holds an action record or an attribute record (3.4.4 rule 2), or the last base record is an attribute record (3.4.4 rule 4), the extension is invalid: the reader presents the base records only, as in rule 2. An invalid extension never makes the whole symbol invalid, so a colour reader presents what a black-and-white reader presents.
5. **Present.** Otherwise the reader presents the base records, then the extension records, as 3.4.4 rule 5 says.

A reader reading video SHOULD keep trying further frames for a while before it settles on rule 2. The length of that wait is the reader's choice.

## 3.6 Hash algorithm IDs

Append-only registry. Each ID fixes both the algorithm and the digest length. IDs 1 to 6 have the numbers and lengths of the initial entries of the Named Information Hash Algorithm Registry (RFC 6920).

| ID | Algorithm | Digest length | Status |
|---|---|---|---|
| 0 | — | — | invalid; makes the container malformed |
| 1 | SHA-256 | 32 bytes (256 bits) | defined |
| 2 | SHA-256 truncated to 128 bits | 16 bytes | not defined in this version |
| 3–6 | SHA-256 truncated to 120, 96, 64, 32 bits | — | never assignable; make the container malformed |
| 7 and up | unassigned | | |

Truncation below 128 bits is forbidden: no ID with a digest shorter than 16 bytes will be assigned. A later ID SHOULD reuse the RFC 6920 registry number of the same algorithm and length when one exists. The transfer chapter uses this registry for the whole-file digest.

The digest gives integrity against accidents and against inconsistency between layers. It does not give authenticity: whoever makes the symbol chooses both the content and the digest.

## 3.7 Integrity: CRC-32C

- Algorithm: CRC-32C as in chapter 1 (1.4).
- Covered bytes: from byte 0 of the message (the lead byte) to the last byte of the body, that is, `2 + size(Lb) + Lb` bytes in a static container and `1 + size(Lb) + Lb` bytes in a transfer tile. Padding is not covered.
- Position: the 4 bytes directly after the body.
- Byte order: most significant byte first. The CRC-32C of ASCII "123456789" is written `E3 06 92 83`.
- Reader: compute the CRC-32C over the same bytes and compare. On a mismatch, or when Lb places the CRC beyond the message capacity, the container is damaged: the reader MUST present nothing from it. A reader reading video continues with the next frame.

Every message has its own CRC-32C: the base-layer message, the extension message and each transfer tile. The CRC-32C catches Reed-Solomon miscorrections; it does not protect against a symbol made on purpose.

## 3.8 Padding

- Bytes after the CRC-32C up to the message capacity are padding.
- Pattern: alternating `EC` and `11`, starting with `EC` at the first byte after the CRC-32C. Pad byte i (i = 0, 1, 2, …) is `EC` when i is even and `11` when i is odd.
- A generator MUST write exactly this pattern. With the record form of 3.2.5 and the default mode of chapter 6 (6.3), the same records then give the same message at the same size and level, with the exceptions that 6.3 states. The size itself is a generator choice (chapter 1, 1.5).
- A reader finds the end of the body from Lb and MUST ignore the padding bytes.

## 3.9 Reader outcomes

The reader decodes the base-layer message (chapter 4), reads the lead byte, the format echo byte and Lb, checks the CRC-32C, and only then interprets the rest. Each check ends in one outcome. Chapter 9 (9.8) collects the names.

| Outcome | Reader presents |
|---|---|
| Damaged | nothing; in video, keeps reading frames |
| Unsupported | nothing from the content; an error naming the unsupported item. It MAY offer the body bytes for saving, labelled as undecoded |
| Malformed | nothing; an error saying the symbol breaks this specification |
| Presented | the records, per 3.4 and 3.5 |
| Presented, base only | the base records and a statement that further content could not be read (3.5 rule 2) |
| Presented, with error | the records together with an error; the action record is not offered until the user has seen the error (chapter 8, 8.6) |

The checks run in the order of the table below, and the reader reports the first that fails; a test vector names that error. The header fields 4 to 10 are read in field order, and each field's checks apply when it is read: the first failure among them ends the reading. For codec 0, L = Lc is checked after field 10.

| # | Field or condition | Outcome | Name |
|---|---|---|---|
| 1 | Lb not a valid LEB128 (1.4), or the CRC would lie beyond the message capacity | Damaged | `E_LENGTH_FIELD` |
| 2 | CRC-32C mismatch | Damaged | `E_CRC_MISMATCH` |
| 3 | More than `colour_mismatch_share_max` of the data modules off the palette, for a reader that samples colour (chapter 7, 7.9.3) | Malformed | `E_COLOUR_AMBIGUOUS` |
| 4 | Container version 1–3 | Unsupported ("newer reader needed") | `E_CONTAINER_VERSION` |
| 5 | Transfer tile with reserved lead-byte bits not 0 | Malformed | `E_TILE_RESERVED_BITS` |
| 6 | Transfer tile, reader without transfer support | Unsupported | `E_TRANSFER_UNSUPPORTED` |
| 7 | Format echo byte other than the byte of the chosen format word | Malformed | `E_FORMAT_ECHO` |
| 8 | X = 1 while the format word's colour profile is 0 | Malformed | `E_COLOUR_FLAG` |
| 9 | A LEB128 field of the header not minimal or ≥ 2^32 | Malformed | `E_LEB128` |
| 9 | A header field that runs past the end of the body | Malformed | `E_HEADER_OVERRUN` |
| 9 | Codec ID escape below 15 (field 4) | Malformed | `E_CODEC_ESCAPE` |
| 9 | Codec ID not known to the reader | Unsupported | `E_UNSUPPORTED_CODEC` |
| 9 | Dictionary ID not carried by the reader (field 5) | Unsupported | `E_UNKNOWN_DICTIONARY` |
| 9 | Dictionary ID of a kind that does not match the codec (chapter 6, 6.11) | Malformed | `E_DICTIONARY_MISMATCH` |
| 9 | Decoded length L > `MAX_STATIC_CONTENT_LEN_V0` (field 6) | Malformed | `E_MALFORMED` |
| 9 | Decoded length L above the reader's own limit (chapter 6, 6.4) | Unsupported ("too large for this reader") | `E_TOO_LARGE` |
| 9 | Hash algorithm ID 0 or 3–6 (field 7) | Malformed | `E_HASH_ID_INVALID` |
| 9 | Hash algorithm ID otherwise unknown | Unsupported | `E_UNKNOWN_HASH` |
| 9 | Record count 0 (field 10) | Malformed | `E_RECORD_LIST` |
| 10 | Codec fails, breaks its bounds (chapter 6, 6.4), or produces a length other than L | Malformed | `E_MALFORMED` |
| 11 | Records overrun, or bytes left over | Malformed | `E_RECORD_LIST` |
| 12 | Ordering rule 1 broken (3.4.4) | Malformed | `E_ACTION_RULE` |
| 13 | Content type ID unknown | Presented, as unknown data | — |
| 14 | Value fails its type's check (UTF-8, PSBT magic) | Presented, as 3.4.3 says | — |
| 15 | File name without a target (3.4.4 rule 3) | Presented, as unknown data | — |
| 16 | Extension message not read, or failing any check of rows 1, 2 and 4 to 11 | Presented, base only | `E_EXTENSION_UNREAD` |
| 17 | Digest mismatch | Malformed; no record presented, base records included | `E_DIGEST_MISMATCH` |
| 18 | Extension message breaking 3.4.4 rule 2 or 4 | Presented, base only | `E_EXTENSION_UNREAD` |

## 3.10 Worked examples

Computed by a script. The CRC-32C implementation reproduces the check value 0xE3069283. Bytes in hexadecimal. The static examples are for black-and-white symbols at level 0 (format echo byte `00`) unless stated otherwise.

### a) Static black-and-white symbol with a URL, codec 0, no dictionary

Content: `https://github.com/needmoretruth/nmtcode` (40 bytes). Single-record form, content type 2. Lb = 1 (type ID) + 40 = 41. Codec 0 has no decoded-length field.

| Part | Bytes |
|---|---|
| Lead byte (version 0, R 0, X 0, codec 0) | `00` |
| Format echo byte (level 0, colour profile 0, cell size 0, class 0) | `00` |
| Lb = 41 | `29` |
| Content type ID 2 | `02` |
| Content (40) | `68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E 65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65` |
| CRC-32C over the 44 bytes above = 0x0578960C | `05 78 96 0C` |

Header 4 bytes, total 48 bytes. A 28 × 28 symbol at level 0 has N = 68 codewords and K = 56 (chapter 5, 5.7; chapter 4, 4.6), so the padding is 8 bytes, `EC 11 EC 11 EC 11 EC 11`, and the full message is:

```
00 00 29 02 68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E
65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65 05 78 96 0C
EC 11 EC 11 EC 11 EC 11
```

The encoder selection rule of chapter 6 (6.3) would not choose codec 0 for this content; example f shows the container it chooses.

### b) UTF-8 text, codec 0, level 1

Content: `안녕하세요 NMT Code` (24 bytes of UTF-8). Single-record form, content type 1, codec 0, in a symbol at error-correction level 1. Lb = 25.

| Part | Bytes |
|---|---|
| Lead byte | `00` |
| Format echo byte (level 1) | `40` |
| Lb = 25 | `19` |
| Content type ID 1 | `01` |
| Content (24) | `EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 20 4E 4D 54 20 43 6F 64 65` |
| CRC-32C over 28 bytes = 0x267D9BA5 | `26 7D 9B A5` |

Header 4 bytes, total 32 bytes before padding.

### c) Transfer-tile container (provisional layout of 3.3)

Session ID 0x00001234, frame counter 3, tile index 7, block 0, erasure-symbol index 31, payload the 16 bytes 0x00 to 0x0F. Lb = 11 + 16 = 27. A tile has no format echo byte.

| Part | Bytes |
|---|---|
| Lead byte | `00` |
| Lb = 27 | `1B` |
| Session ID | `00 00 12 34` |
| Frame counter | `00 03` |
| Tile index | `07` |
| Block number | `00 00` |
| Erasure-symbol index | `00 1F` |
| Payload (16) | `00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F` |
| CRC-32C over 29 bytes = 0xAC39AE69 | `AC 39 AE 69` |

Header 13 bytes, total 33 bytes before padding.

### d) Record-list form: action, file name, file

Records: URL of example a (type 2), file name `hello.txt` (type 5), file `hello` followed by a line feed (type 4). Codec 0. Content = 61 bytes; Lb = 1 (record count) + 61 = 62.

| Part | Bytes |
|---|---|
| Lead byte (R 1) | `20` |
| Format echo byte | `00` |
| Lb = 62 | `3E` |
| Record count 3 | `03` |
| Record 1: type 2, length 40, value | `02 28` + the 40 content bytes of example a |
| Record 2: type 5, length 9, `hello.txt` | `05 09 68 65 6C 6C 6F 2E 74 78 74` |
| Record 3: type 4, length 6 | `04 06 68 65 6C 6C 6F 0A` |
| CRC-32C over 65 bytes = 0x21C2FFFD | `21 C2 FF FD` |

Total 69 bytes before padding.

### e) Colour symbol: base record, extension record, digest

A 32 × 32 symbol at level 0 with colour profile 1 and 1 × 1 chroma cells: d = 0x0020082 (chapter 2, 2.2) and the format echo byte is `10`. Base record: text `NMT Code` (type 1). Extension record: text `안녕하세요` (type 1, 15 bytes). Codec 0 in both messages.

Digest input (d, one base record, the canonical base record, one extension record, the canonical extension record; 33 bytes):

```
00 02 00 82 01 01 08 4E 4D 54 20 43 6F 64 65 01 01 0F EC 95 88 EB 85 95
ED 95 98 EC 84 B8 EC 9A 94
```

SHA-256 of it:

```
88 42 74 7E 65 E1 E1 BB CF 64 6D 36 FC 53 53 49 0E B4 4D 5E EC 5A FF C7
BD 1F 41 03 BE F9 EA D5
```

Base-layer message (Lb = 1 + 32 + 1 + 8 = 42):

| Part | Bytes |
|---|---|
| Lead byte (X 1) | `10` |
| Format echo byte (level 0, colour profile 1, cell size 0) | `10` |
| Lb = 42 | `2A` |
| Hash algorithm ID 1 | `01` |
| Digest (32) | as above |
| Content type ID 1 | `01` |
| Content (8) | `4E 4D 54 20 43 6F 64 65` |
| CRC-32C over 45 bytes = 0x03DB0AA4 | `03 DB 0A A4` |

Extension message (Lb = 16):

| Part | Bytes |
|---|---|
| Lead byte | `00` |
| Format echo byte | `10` |
| Lb = 16 | `10` |
| Content type ID 1 | `01` |
| Content (15) | `EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94` |
| CRC-32C over 19 bytes = 0x28CA4D68 | `28 CA 4D 68` |

A black-and-white reader presents `NMT Code` and states that more content exists. A colour reader that decodes the extension checks the digest and presents `NMT Code`, then `안녕하세요` apart from it, labelled as colour content.

### f) The URL of example a with the codec that the selection rule chooses

Codec 3 (chapter 6, 6.8) with dictionary 0 (model 0) codes the 40 bytes into 19 bytes (chapter 6, 6.12.3). Single-record form, content type 2. Lb = 1 (dictionary ID) + 1 (L) + 1 (type ID) + 19 = 22.

| Part | Bytes |
|---|---|
| Lead byte (version 0, R 0, X 0, codec 3) | `03` |
| Format echo byte | `00` |
| Lb = 22 | `16` |
| Dictionary ID 0 | `00` |
| Decoded length L = 40 | `28` |
| Content type ID 2 | `02` |
| Content (19) | `DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91` |
| CRC-32C over the 25 bytes above = 0x137DCFC4 | `13 7D CF C4` |

Total 29 bytes before padding, against 48 for example a. Annex A (A.2) places this container in a complete symbol.
