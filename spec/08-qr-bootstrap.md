# NMT Code — 8. QR bootstrap

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder).

## 8.1 Purpose and scope

A phone camera app that reads only QR Code cannot read an NMT Code symbol. The **bootstrap QR** is a separate, small, standard QR Code symbol placed beside the NMT Code symbol. It holds one fixed https URL, so a QR reader gets a link to a page that explains NMT Code and where to get a reader.

This chapter defines the bootstrap QR's content (8.2), its QR parameters (8.3), its placement and size (8.4), when a generator adds it (8.5), what an NMT Code reader does with it (8.6), and the trademark notice (8.7).

The bootstrap QR is not part of the NMT Code symbol. It carries no NMT Code data, and an NMT Code reader never presents its content.

## 8.2 Content: the URL constant

The bootstrap QR holds exactly one value from the registry below, as its ASCII bytes, and nothing else.

| Version | Value | Length | Status |
|---|---|---|---|
| 0 | `https://github.com/needmoretruth/nmtcode` | 40 bytes | current |

Rules:

- The registry is append-only (chapter 9, 9.3). A value, once published, stays a known value for every later reader.
- A new version is added when the page address changes, for example when the repository is renamed. The new version becomes current; earlier versions stay known.
- A generator MUST use the current version of the specification version it implements.
- Every value is an `https://` URL of the NMT Code repository or of one of its release pages, in ASCII, with no query and no fragment. Because every value starts with `https://`, the content never starts with `UR:` or `B$`, the prefixes of the animated-QR formats that wallet applications read. A value MUST NOT be a web address that stops working when the repository is renamed; the repository's own address is kept redirecting after a rename, provided the old name is never reused.

## 8.3 QR parameters

| Parameter | Value |
|---|---|
| Symbology | QR Code Model 2, as specified in ISO/IEC 18004:2024. Not Micro QR, not rMQR, not Model 1 |
| Error-correction level | M by default. Q or H MAY be chosen, for example for print where damage is expected. L is not used: it gives no smaller symbol for this content (8.3.1) |
| Version | the smallest version that holds the content at the chosen level: 3 at M, 4 at Q, 5 at H (for the 40-byte value of version 0) |
| Mode | one byte-mode segment holding the whole value |
| ECI | none. The value is ASCII, which the default byte-mode character set of ISO/IEC 18004 covers |
| Structured Append, FNC1 | not used |
| Mask | chosen by the encoder by the mask-selection rule of ISO/IEC 18004. Readers do not depend on it |
| Colours | dark modules black `#000000` (K ink in print), light modules and quiet zone white `#FFFFFF` (paper). No colour, no reflectance reversal (light modules on a dark background), no mirror image |
| Quiet zone | 4 QR modules on all sides, as ISO/IEC 18004 requires |

An encoder MUST produce a symbol that conforms to ISO/IEC 18004:2024 with these parameters. Any conforming encoder may be used; a generator MAY instead embed the finished module matrix of the value, since the value is fixed. This specification requires no particular QR library.

### 8.3.1 Version choice

For the 40-byte value of version 0, at level M:

| Mode | Bits needed | Smallest version at M (data codewords, bits) |
|---|---|---|
| Byte, one segment | 4 (mode) + 8 (count) + 40 × 8 = 332 | 3 (44 codewords, 352 bits) |
| Alphanumeric, value in upper case | 4 + 9 + 20 × 11 = 233 | 3 (version 2 has 28 codewords, 224 bits, too few) |

Alphanumeric mode would need the value in upper case and gives no smaller symbol, so byte mode is used and the value keeps its exact bytes. Version 3 is 29 × 29 modules, 37 × 37 with its quiet zone.

At level L the byte-mode value also needs version 3 (version 2 at L holds 32 bytes), so L buys no area and only loses robustness.

## 8.4 Placement and size

### 8.4.1 Module size

Let s be the NMT Code module size and s_min the minimum bootstrap module size:

| Output | s_min |
|---|---|
| physical size known (print, or a screen rendered at a known pixel density) | 0.4 mm |
| screen with unknown pixel density | 4 device pixels |

(parameter `qr_min_module`, tunable in 0.x, fixed before 1.0).

The bootstrap QR module is n NMT Code modules wide, where n is the smallest integer ≥ 1 with n · s ≥ s_min. With the default module sizes of the named profiles (chapter 1, 1.5), n = 1: the bootstrap QR uses the NMT Code module size. n > 1 only when the user chose modules smaller than s_min. The generator MAY let the user choose a larger n. Because n is an integer, QR modules stay on the NMT Code pixel or printer-dot grid.

Let v be the QR version (8.3) and S = (17 + 4v) · n the QR side in NMT Code modules (29n for version 3).

### 8.4.2 Gap

Let Q ≥ 2 be the NMT Code quiet zone in NMT Code modules (chapter 5). The gap G between the facing edges of the two symbols is

    G = n · max(4, ceil(Q / n))      NMT Code modules  =  max(4, ceil(Q / n))  QR modules

The gap is light. It contains both the NMT Code quiet zone (Q modules) and the QR quiet zone (4 QR modules), which overlap. With n = 1 and Q = 2, G = 4 modules.

### 8.4.3 Position

The bootstrap QR sits above the NMT Code symbol or to its left, in the NMT Code coordinates of chapter 1, 1.4 (quiet zone excluded):

| Side | QR symbol occupies (quiet zone excluded) |
|---|---|
| left | −G − S ≤ x < −G, 0 ≤ y < S (top edges aligned) |
| above | 0 ≤ x < S, −G − S ≤ y < −G (left edges aligned) |

- Default side: left when W ≥ H, above when H > W. The QR then sits on the shorter side of the NMT Code symbol, which adds the least area. The generator MAY let the user choose the other side.
- The whole canvas (both symbols with their quiet zones) spans:

  | Side | x from | x to (exclusive) | y from | y to (exclusive) |
  |---|---|---|---|---|
  | left | −(G + S + 4n) | W + Q | −max(Q, 4n) | max(H + Q, S + 4n) |
  | above | −max(Q, 4n) | max(W + Q, S + 4n) | −(G + S + 4n) | H + Q |

- Nothing else is drawn in the canvas. Every module of the canvas outside the two symbols is light.
- Other QR symbols: a generator MUST NOT place any QR symbol other than the bootstrap QR closer to an NMT Code symbol than that QR symbol's own side length, and a page layout SHOULD keep the same distance, so that readers do not take it for a bootstrap QR (8.6).

## 8.5 When the bootstrap QR is added

| Symbol | Default |
|---|---|
| static symbol (class 0), any named profile, screen or print | on |
| transfer tile (class 1) | off; a generator SHOULD NOT place a bootstrap QR beside a transfer tile. The transfer chapter defines where a sender shows it |

A generator MUST let the user turn the bootstrap QR off. The format word and the container do not record whether a bootstrap QR is present; a reader never needs it.

## 8.6 Reader rules

An NMT Code reader need not decode QR symbols. A reader that decodes QR symbols SHOULD check the bootstrap QR, because a bootstrap QR replaced by a sticker would send QR-reader users elsewhere while NMT Code readers see the NMT Code content.

**Association.** A decoded QR symbol belongs to an NMT Code symbol in the same image or video frame when, with the QR symbol's corners mapped into the NMT Code symbol's coordinates, the smallest distance between the QR symbol (quiet zone excluded) and the rectangle 0 ≤ x ≤ W, 0 ≤ y ≤ H is at most the QR symbol's own side length in those coordinates. An overlapping QR symbol has distance 0. The side does not matter: a QR symbol below or to the right is checked too.

For an associated QR symbol, the reader compares its decoded byte string with every value of 8.2 that it knows, byte for byte (no case folding, no URL normalisation):

- **Equal to a known value.** The reader MUST NOT present it as content of the NMT Code symbol and MUST NOT open it. It MAY ignore it silently.
- **Different from every known value.** The reader MUST NOT present that content as a result in any form (no display as a result, no link, no copy, no open) and MUST report an error saying that the QR code beside this NMT Code does not match the NMT Code bootstrap (`E_BOOTSTRAP_MISMATCH`, chapter 9, 9.8). It SHOULD mark the QR symbol's position in the image.
  - The reader MAY still present the NMT Code symbol's own result, together with that error. It SHOULD NOT offer the NMT Code symbol's action record (chapter 3, 3.4) until the user has seen the error.
  - One mismatching decode is enough; the reader does not wait for a second frame.

A QR symbol that is not associated with any NMT Code symbol is outside this specification. A reader MAY report it separately; it MUST NOT merge it into an NMT Code result.

Informative: NMT Code finder patterns avoid the 1:1:3:1:1 run-length ratio of QR finder patterns (chapter 5), so a QR reader finds only the bootstrap QR in the canvas.

## 8.7 Trademark notice

"QR Code" is a registered trademark of DENSO WAVE INCORPORATED. DENSO WAVE's FAQ (https://www.qrcode.com/en/faq.html) says, in plain terms:

- The trademark covers the word "QR Code", not the QR pattern itself.
- A publication or web site that uses the word should state that QR Code is a registered trademark of DENSO WAVE INCORPORATED.
- Where only a QR image appears, without the word, no statement is needed.
- On a web site the statement can be anywhere; in print it is best on the same page as the word.

Rules for implementations of this specification:

- A document, user interface text, package description or web page that uses the words "QR Code" SHOULD carry the sentence: **"QR Code is a registered trademark of DENSO WAVE INCORPORATED."**
- The bootstrap QR content and NMT Code symbols never contain the words "QR Code".
- An implementation MUST NOT describe NMT Code as a kind of QR Code or as endorsed by DENSO WAVE.

DENSO WAVE also states that anyone may use QR Code freely when following the QR Code standards of JIS or ISO (https://www.qrcode.com/en/patent.html). The bootstrap QR is therefore a plain conforming symbol: nothing is embedded in it and nothing overlaps its quiet zone.

## 8.8 Worked example

Values computed by a script. Its Reed-Solomon encoder reproduces the published parity of the QR version 1-M example `HELLO WORLD` (`C4 23 27 77 EB D7 E7 E2 5D 17`).

### 8.8.1 Codewords of the version 0 value at 3-M

Value bytes (40):

```
68 74 74 70 73 3A 2F 2F 67 69 74 68 75 62 2E 63 6F 6D 2F 6E
65 65 64 6D 6F 72 65 74 72 75 74 68 2F 6E 6D 74 63 6F 64 65
```

Bit stream: mode indicator `0100` (byte), character count `00101000` (40), the 320 value bits, terminator `0000`. That is 336 bits = 42 bytes, already on a byte boundary. Two pad codewords `EC 11` fill the 44 data codewords of version 3-M (one block):

```
42 86 87 47 47 07 33 A2 F2 F6 76 97 46 87 56 22 E6 36 F6 D2 F6 E6
56 56 46 D6 F7 26 57 47 27 57 46 82 F6 E6 D7 46 36 F6 46 50 EC 11
```

Error-correction codewords (26), Reed-Solomon over GF(2^8) with field polynomial 0x11D and generator roots α^0 … α^25:

```
3D 7E 96 5A A8 1D 01 67 EA 68 9C D9 12 4C D4 F3 33 BC E8 1A DD 81 C4 14 2B 16
```

The 70 codewords placed in the symbol are the 44 data codewords followed by these 26. The module matrix then depends on the mask the encoder selects (8.3).

### 8.8.2 Placement

| Case | NMT Code symbol | s | n | Q | G | Side | QR occupies (NMT Code modules) | Canvas (NMT Code modules) |
|---|---|---|---|---|---|---|---|---|
| Screen | 64 × 64 | 4 px | 1 | 2 | 4 | left (W ≥ H) | −33 ≤ x < −4, 0 ≤ y < 29 | x −37 … 66, y −4 … 66: 103 × 70 (412 × 280 px) |
| Screen, tall | 48 × 96 | 4 px | 1 | 2 | 4 | above (H > W) | 0 ≤ x < 29, −33 ≤ y < −4 | x −4 … 50, y −37 … 98: 54 × 135 |
| Print, small modules | 80 × 80 | 0.25 mm | 2 | 2 | 8 | left | −66 ≤ x < −8, 0 ≤ y < 58 | x −74 … 82, y −8 … 82: 156 × 90 (39.0 × 22.5 mm) |

In the print case the QR module is 0.5 mm, the QR symbol is 14.5 mm wide, and the gap is 2 mm: 8 NMT Code modules, or 4 QR modules.

QR Code is a registered trademark of DENSO WAVE INCORPORATED.
