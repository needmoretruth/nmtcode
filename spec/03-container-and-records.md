# NMT Code — 3. Container and records

Specification version 0.1 (draft).

This chapter defines the bytes that the error-corrected base layer carries, the extension message of a colour symbol, the records inside them, and the checks a reader applies.

## 3.1 Terms

| Term | Meaning |
|---|---|
| Message | The data bytes of one layer after error-correction decoding, in order. The base-layer message is defined by chapter 4; the colour-layer message by chapter 7 |
| Message capacity K | The number of bytes in a message: K of chapter 4 (4.6) for the base layer, K_c of chapter 7 (7.8.2) for the colour layer. It follows from W, H, the error-correction level and the colour profile |
| Container | The structure at the start of a message: lead byte, body length, body, CRC-32C |
| Body | The header fields after the length field, followed by the content |
| Content | The codec output (chapter 6), Lc bytes, called the coded field in chapter 6. Decoding it gives the decoded content |
| Body length Lb | The number of bytes in the body (field 2) |
| Decoded length L | The number of bytes of the decoded content. Written in field 5 when the codec is not 0; equal to Lc when the codec is 0 |
| Record | One item of decoded content: a type ID and a value |
| Canonical record | `type ID (LEB128) ‖ value length (LEB128) ‖ value`, computed over the decoded value |

All multi-byte integers in this chapter are LEB128 as defined in 1.4, except the CRC-32C (3.7) and the transfer-tile fields (3.3), which are fixed-width, most significant byte first.

LEB128 byte order, for clarity: the first byte holds the least significant 7 bits; bit 7 (0x80) of a byte is set when another byte follows. Examples: 0 = `00`, 127 = `7F`, 128 = `80 01`, 300 = `AC 02`, 16384 = `80 80 01`, 2^32 − 1 = `FF FF FF FF 0F`.

## 3.2 Container of a static symbol

The format word's symbol class (chapter 2) selects the layout: class 0 (static) uses this section, class 1 (transfer tile) uses 3.3. The container has no bit of its own for this, so the two cannot disagree.

### 3.2.1 Field order

| # | Field | Size | Present when |
|---|---|---|---|
| 1 | Lead byte | 1 byte | always |
| 2 | Body length Lb | LEB128 | always |
| 3 | Codec ID escape | LEB128 | codec field of the lead byte = 15 |
| 4 | Dictionary ID | LEB128 | the codec takes a dictionary: codecs 3 and 5 (chapter 6, 6.2) |
| 5 | Decoded length L | LEB128 | codec ID ≠ 0 |
| 6 | Hash algorithm ID | LEB128 | C = 1 |
| 7 | Digest | length given by the hash algorithm ID (3.6) | C = 1 |
| 8 | Content type ID | LEB128 | R = 0 |
| 9 | Record count | LEB128 | R = 1 |
| 10 | Content | the rest of the body | always (may be empty) |
| 11 | CRC-32C | 4 bytes | always |
| 12 | Padding | to the message capacity | when bytes remain |

Fields 3 to 10 form the body. Lb is the number of bytes in the body. The content length Lc = Lb minus the sizes of fields 3 to 9 that are present.

Layout rule for every container version, present and future: byte 0 is the lead byte, the next bytes are Lb as LEB128, and the 4 bytes after the body are the CRC-32C of 3.7. A reader can therefore check the CRC of a container whose version it does not support.

### 3.2.2 Lead byte

| Bits | Name | Width | Values |
|---|---|---|---|
| 7–6 | Container version | 2 | 0 = this chapter. 1–3 reserved |
| 5 | R, record form | 1 | 0 = single-record form (field 8 present). 1 = record-list form (field 9 present) |
| 4 | C, colour extension | 1 | 0 = all records are in this message. 1 = a colour extension message exists (3.5); fields 6 and 7 are present |
| 3–0 | Codec | 4 | 0–14 = codec ID (chapter 6). 15 = the codec ID is in field 3 |

C = 1 is allowed only when the format word's colour profile is not 0.

A colour symbol with C = 0 carries all its records in the base layer. Chapter 7 defines what its colour layer then carries.

### 3.2.3 Body length Lb

- Range: 0 to 2^32 − 1. It MUST satisfy `1 + size(Lb) + Lb + 4 ≤ K`, where size(Lb) is the number of bytes of the LEB128 encoding of Lb.
- The reader uses Lb to find the CRC-32C. The reader MUST NOT interpret fields 3 to 10 before the CRC-32C has passed.

### 3.2.4 Codec, dictionary and decoded length

- Codec ID: from the lead byte when it is 0–14. When the lead byte says 15, field 3 holds the codec ID, and its value MUST be 15 or more. A smaller value makes the container malformed.
- A reader that does not know the codec ID stops here with the outcome Unsupported (3.9): it cannot know whether fields 4 and 5 are present.
- Dictionary ID: present if and only if chapter 6 (6.2) marks the codec as taking a dictionary. In this version codecs 3 (short-text model) and 5 (brotli) do. Value 0 means the codec's built-in model or dictionary. Other values name an entry of the dictionary registry (chapter 6, 6.11).
- Decoded length L: present if and only if the codec ID is not 0. For codec 0, L = Lc. Before allocating, the reader MUST check L ≤ `MAX_CONTENT_LEN_V0` and L ≤ its own limit (chapter 6, 6.4). Decoding MUST produce exactly L bytes; any other length makes the container malformed.
- The codec decodes the whole content into the decoded content. No codec of this version reads the content type ID. In record-list form the decoded content is a list of canonical records.

### 3.2.5 Record form

- Single-record form (R = 0): field 8 is the content type ID of the one record. The decoded content is that record's value. A value of length 0 is allowed.
- Record-list form (R = 1): field 9 is the record count, 1 or more. The decoded content is exactly that many canonical records, one after another, with no bytes left over. A count of 0, a record that runs past the end, or bytes left over make the container malformed.

### 3.2.6 Header size

The header is the lead byte, Lb and fields 3 to 9. For single-record form, codec 0, a content type ID below 128 and no colour extension, the header is 3 bytes when the content is at most 126 bytes, and 4 bytes when it is 127 to 16 382 bytes.

| Case | Header bytes |
|---|---|
| 40-byte URL, codec 0, single-record form (example 3.10 a) | 3 (`00 29 02`) |
| Same URL, codec 3, dictionary 0 (example 3.10 f) | 5 (`03 16 00 28 02`) |
| 200-byte text, codec 0 | 4 |
| Codec 1, 2 or 4 | + size of L (1 byte below 128, 2 bytes below 16 384) |
| Codec 3 or 5 | + size of L + size of the dictionary ID (1 byte below 128) |
| Codec ID 15 or more | +1 or more |
| Colour extension (hash ID 1) | +33 |
| Record-list form | as single-record form in the header; each record adds its type ID and value length (2 bytes for type IDs below 128 and values below 128 bytes) |

The CRC-32C adds 4 bytes to every container.

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

Fields 3 to 7 take 11 bytes, so a tile header is 13 or 14 bytes before the CRC-32C. The codec, dictionary, content type, file name and whole-file digest of a transfer are carried by the transfer manifest of the transfer chapter, not by each tile. In this version a transfer tile MUST have colour profile 0. A reader that does not implement transfer MUST report a transfer tile as unsupported.

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

"Reserved" for IDs 7 and 8: the ID is assigned now, and signature checking or decryption by the reader is left to a later version. In this version a reader presents them as files and MUST NOT describe a type 7 record as verified.

"Never compress": a generator MUST use codec 0 for a container that holds such a record, because ciphertext and signed bytes do not compress. Readers do not check this rule.

### 3.4.3 Presentation by type

| ID | Reader presents |
|---|---|
| 0 | as bytes (for example hexadecimal), available to copy or save |
| 1, 3 | as text. If the value is not valid UTF-8, as type 0 with a notice |
| 2 | the complete URL text. The reader MAY offer to open it. It MUST NOT open it without a user action, and MUST show the complete text before opening. It chooses which URI schemes it offers to open. If the value is not valid UTF-8, as type 0 with a notice |
| 4, 6, 7, 8 | as a file offered for saving (type 6 MAY also be offered to a wallet application). The reader MUST NOT open or run it |
| 5 | as the name of the file that follows. The reader MUST NOT use it as a path: it removes directory parts, control characters and `/` `\`. If the value is not valid UTF-8, as type 0 |
| unknown | as unknown data: the type ID, the length, and the bytes available to copy or save. Never acted on |

PSBT recognition: a reader MAY present a type 0 or type 4 value that starts with `70 73 62 74 FF` as a PSBT. A type 6 value that does not start with these bytes is presented as type 4 with a notice. A generator SHOULD use type 6 for a PSBT.

Unknown data is always presented and never silently dropped. This applies to unknown type IDs, to values that fail their type's check, and to attribute records without a target.

### 3.4.4 Ordering rules

1. A symbol holds at most one action record (type 2), counting base and extension records together. More than one makes the symbol malformed. Reason: with two actions, different readers could offer different ones.
2. An action record, if present, MUST be the first record of the base-layer message. An action record in any other position, including anywhere in the extension message, makes the symbol malformed.
3. A file name record (type 5) MUST be followed directly by a record of type 0, 4, 6, 7 or 8, and a file record has at most one file name. A file name record that breaks this rule is presented as unknown data; the symbol is not rejected.
4. The reader presents records in order: base records first, then extension records.

## 3.5 Colour symbols

When C = 1, the content is split over two messages.

- Base-layer message: the container of 3.2 with fields 6 and 7. Its records are the base records. It MUST hold at least one record. Every reader MUST present the base records, whether or not it reads colour.
- Colour-layer message (the extension message): the container of 3.2 with C = 0 and its own CRC-32C and padding. Its records are the extension records. Chapter 7 defines how the colour layer carries it and its message capacity.
- Digest: `SHA-256(canonical base records ‖ canonical extension records)`, over the decoded values, in record order. It does not depend on the codec or the record form used.

Reader rules:

1. The base container MUST pass its own checks (3.7, 3.9) before anything is presented.
2. The reader presents the base records only, and MUST state that the symbol holds further content it could not read, when any of these happens: it does not read colour; colour-layer error correction fails; the extension CRC-32C fails; the extension container is unsupported or malformed.
3. Otherwise it computes the digest. If the digest differs from field 7, the reader MUST reject the whole symbol and present no record, base records included. Reason: the colour layer then decoded to content that the symbol did not commit to, through a miscorrection that the CRC missed or through a symbol built so that readers disagree.
4. If the digest matches, the reader presents the base records, then the extension records.

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

- Algorithm: CRC-32C as in 1.4.
- Covered bytes: from byte 0 of the message (the lead byte) to the last byte of the body, that is, `1 + size(Lb) + Lb` bytes. Padding is not covered.
- Position: the 4 bytes directly after the body.
- Byte order: most significant byte first. The CRC-32C of ASCII "123456789" is written `E3 06 92 83`.
- Reader: compute the CRC-32C over the same bytes and compare. On a mismatch, or when Lb places the CRC beyond the message capacity, the container is damaged: the reader MUST present nothing from it. A reader reading video continues with the next frame.

Every message has its own CRC-32C: the base-layer message, the extension message and each transfer tile. The CRC-32C catches Reed-Solomon miscorrections; it does not protect against a symbol made on purpose.

## 3.8 Padding

- Bytes after the CRC-32C up to the message capacity are padding.
- Pattern: alternating `EC` and `11`, starting with `EC` at the first byte after the CRC-32C. Pad byte i (i = 0, 1, 2, …) is `EC` when i is even and `11` when i is odd.
- A generator MUST write exactly this pattern, so that the same input always gives the same symbol.
- A reader finds the end of the body from Lb and MUST ignore the padding bytes.

## 3.9 Reader outcomes

The reader decodes the base-layer message (chapter 4), reads the lead byte and Lb, checks the CRC-32C, and only then interprets the rest. Each check ends in one outcome. Chapter 9 (9.8) gives each condition below a stable name.

| Outcome | Reader presents |
|---|---|
| Damaged | nothing; in video, keeps reading frames |
| Unsupported | nothing from the content; an error naming the unsupported item. It MAY offer the body bytes for saving, labelled as undecoded |
| Malformed | nothing; an error saying the symbol breaks this specification |
| Presented | the records, per 3.4 and 3.5 |
| Presented, base only | the base records and a statement that further content could not be read (3.5 rule 2) |

Per field:

| Field or condition | Outcome |
|---|---|
| Lb not a valid LEB128 (1.4), or the CRC would lie beyond the message capacity | Damaged |
| CRC-32C mismatch | Damaged |
| Container version 1–3 | Unsupported ("newer reader needed") |
| Transfer tile, reader without transfer support | Unsupported |
| Transfer tile with reserved lead-byte bits not 0 | Malformed |
| C = 1 while the format word's colour profile is 0 | Malformed |
| Any LEB128 field in the body not minimal or ≥ 2^32 | Malformed |
| Codec ID escape below 15 | Malformed |
| Codec ID not known to the reader | Unsupported |
| Dictionary ID not known to the reader | Unsupported |
| Dictionary ID of a kind that does not match the codec (chapter 6, 6.11) | Malformed |
| Hash algorithm ID 0 or 3–6 | Malformed |
| Hash algorithm ID otherwise unknown | Unsupported |
| Header fields run past the end of the body | Malformed |
| Decoded length L > `MAX_CONTENT_LEN_V0` | Malformed |
| Decoded length L above the reader's own limit (chapter 6, 6.4) | Unsupported ("too large for this reader") |
| Codec fails, or produces a length other than L | Malformed |
| Record count 0, records overrun, bytes left over | Malformed |
| C = 1 and no base record | Malformed |
| Ordering rule 1 or 2 broken (3.4.4) | Malformed |
| Content type ID unknown | Presented, as unknown data |
| Value fails its type's check (UTF-8, PSBT magic) | Presented, as 3.4.3 says |
| File name without a target (rule 3) | Presented, as unknown data |
| Extension message: any failure of this table | Presented, base only |
| Digest mismatch | Malformed; no record presented, base records included |

## 3.10 Worked examples

Computed by a script. The CRC-32C implementation reproduces the check value 0xE3069283. Bytes in hexadecimal.

### a) Static black-and-white symbol with a URL, codec 0, no dictionary

Content: `https://github.com/needmoretruth/nmtcode` (40 bytes). Single-record form, content type 2. Lb = 1 (type ID) + 40 = 41. Codec 0 has no decoded-length field.

| Part | Bytes |
|---|---|
| Lead byte (version 0, R 0, C 0, codec 0) | `00` |
| Lb = 41 | `29` |
| Content type ID 2 | `02` |
| Content (40) | `68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E 65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65` |
| CRC-32C over the 43 bytes above = 0xFCCF4D0A | `FC CF 4D 0A` |

Header 3 bytes, total 47 bytes. A 28 × 28 symbol at level 0 has N = 68 codewords and K = 56 (chapter 5, 5.7; chapter 4, 4.6), so the padding is 9 bytes, `EC 11 EC 11 EC 11 EC 11 EC`, and the full message is:

```
00 29 02 68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E 65
65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65 FC CF 4D 0A EC
11 EC 11 EC 11 EC 11 EC
```

The encoder selection rule of chapter 6 (6.3) would not choose codec 0 for this content; example f shows the container it chooses.

### b) UTF-8 text, codec 0

Content: `안녕하세요 NMT Code` (24 bytes of UTF-8). Single-record form, content type 1, codec 0. Lb = 25.

| Part | Bytes |
|---|---|
| Lead byte | `00` |
| Lb = 25 | `19` |
| Content type ID 1 | `01` |
| Content (24) | `EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 20 4E 4D 54 20 43 6F 64 65` |
| CRC-32C over 27 bytes = 0xFFF6057C | `FF F6 05 7C` |

Header 3 bytes, total 31 bytes before padding.

### c) Transfer-tile container (provisional layout of 3.3)

Session ID 0x00001234, frame counter 3, tile index 7, block 0, erasure-symbol index 31, payload the 16 bytes 0x00 to 0x0F. Lb = 11 + 16 = 27.

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
| Lb = 62 | `3E` |
| Record count 3 | `03` |
| Record 1: type 2, length 40, value | `02 28` + the 40 content bytes of example a |
| Record 2: type 5, length 9, `hello.txt` | `05 09 68 65 6C 6C 6F 2E 74 78 74` |
| Record 3: type 4, length 6 | `04 06 68 65 6C 6C 6F 0A` |
| CRC-32C over 64 bytes = 0x3855D91B | `38 55 D9 1B` |

Total 68 bytes before padding.

### e) Colour symbol: base record, extension record, digest

Base record: text `NMT Code` (type 1). Extension record: text `안녕하세요` (type 1, 15 bytes). Codec 0 in both messages.

Digest input (canonical base records ‖ canonical extension records, 27 bytes):

```
01 08 4E 4D 54 20 43 6F 64 65 01 0F EC 95 88 EB 85 95 ED 95 98 EC 84 B8
EC 9A 94
```

SHA-256 of it:

```
53 14 09 A7 3A CC A0 2F A0 78 07 F5 45 BA B6 B8 F0 40 3D 7F 96 CA 2A A1
F8 39 A6 73 FD DB 53 31
```

Base-layer message (Lb = 1 + 32 + 1 + 8 = 42):

| Part | Bytes |
|---|---|
| Lead byte (C 1) | `10` |
| Lb = 42 | `2A` |
| Hash algorithm ID 1 | `01` |
| Digest (32) | as above |
| Content type ID 1 | `01` |
| Content (8) | `4E 4D 54 20 43 6F 64 65` |
| CRC-32C over 44 bytes = 0x0506D2A1 | `05 06 D2 A1` |

Extension message (Lb = 16):

| Part | Bytes |
|---|---|
| Lead byte | `00` |
| Lb = 16 | `10` |
| Content type ID 1 | `01` |
| Content (15) | `EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94` |
| CRC-32C over 18 bytes = 0xA2ECCDA1 | `A2 EC CD A1` |

A black-and-white reader presents `NMT Code` and states that more content exists. A colour reader that decodes the extension checks the digest and presents `NMT Code`, then `안녕하세요`.

### f) The URL of example a with the codec that the selection rule chooses

Codec 3 (chapter 6, 6.8) with dictionary 0 (model 0) codes the 40 bytes into 19 bytes (chapter 6, 6.12.3). Single-record form, content type 2. Lb = 1 (dictionary ID) + 1 (L) + 1 (type ID) + 19 = 22.

| Part | Bytes |
|---|---|
| Lead byte (version 0, R 0, C 0, codec 3) | `03` |
| Lb = 22 | `16` |
| Dictionary ID 0 | `00` |
| Decoded length L = 40 | `28` |
| Content type ID 2 | `02` |
| Content (19) | `DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91` |
| CRC-32C over the 24 bytes above = 0x3356F16E | `33 56 F1 6E` |

Total 28 bytes before padding, against 47 for example a. Annex A (A.2) places this container in a 24 × 24 symbol.
