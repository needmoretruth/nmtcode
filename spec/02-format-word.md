# NMT Code — 2. Format word

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft).

## 2.1 Scope

The format word tells a reader what it needs before it reads the data area: the format version, the symbol class, the size, the error-correction level, the colour profile and the chroma cell size. This chapter defines:

- its 28 data bits (2.2) and what a reader does with each value (2.3);
- the code that protects it (2.4) and the masks of its two copies (2.5);
- how its bits sit on the modules of its two copies (2.6);
- how a reader decodes it (2.7).

The positions of the two copies are defined in chapter 5 (5.5). Everything else a reader needs is in the error-corrected container (chapter 3), which repeats the level, the colour profile, the chroma cell size and the symbol class in its format echo byte (chapter 3, 3.2.2).

## 2.2 Fields

The format word's data part is 28 bits, d[27] … d[0]. d[27] is sent first. Multi-bit fields are most significant bit first.

| Bits | Field | Width | Values | Meaning |
|---|---|---|---|---|
| d[27..26] | Format version | 2 | 0–3 | 0 = this specification (0.x). 1 and 2 are reserved. 3 is reserved as the extended format version: a later version of this specification gives the actual format version in the container and defines its layout; this version defines none |
| d[25] | Symbol class | 1 | 0–1 | 0 = static single code. 1 = transfer tile (the container carries transfer fields; defined in a later 0.x version) |
| d[24..15] | Width code w | 10 | 1–1023 | W = 4 × (w + 4) modules, 20 ≤ W ≤ 4108. w = 0 (W = 16) is invalid |
| d[14..5] | Height code h | 10 | 1–1023 | H = 4 × (h + 4) modules, 20 ≤ H ≤ 4108. h = 0 (H = 16) is invalid |
| d[4..3] | Error-correction level | 2 | 0–3 | Recovery comparable to QR Code levels: 0 ≈ L, 1 ≈ M, 2 ≈ Q, 3 ≈ H. Parity is at least 15, 30, 50 and 60% of the codewords, which corrects about 7.5, 15, 25 and 30% of them as byte errors. Chapter 4 (4.5, 4.6) gives the exact parity for every size |
| d[2..1] | Colour profile | 2 | 0–1 | 0 = black and white. 1 = luminance base layer plus chroma cells, 4 colours (chapter 7). 2 and 3 are reserved |
| d[0] | Chroma cell size | 1 | 0–1 | 0 = a chroma cell is 1 × 1 module. 1 = a chroma cell is 2 × 2 modules. MUST be 0 when the colour profile is 0 |

As an integer:

    d = (version << 26) | (class << 25) | (w << 15) | (h << 5) | (level << 3) | (colour << 1) | cell

A later format version that keeps the four finder patterns (chapter 5, 5.3), their separators (5.4) and the positions of the two copies relative to the finders (5.5) also keeps d[27..26] as the format version field, the code (2.4) and the masks (2.5). A reader of this version can then read its version and report that a newer reader is needed. A later format version that changes the finders or the positions of the copies cannot be told apart from an unreadable symbol by a reader of this version.

## 2.3 Field values and reader behaviour

A generator MUST NOT write a reserved or invalid value (width or height code 0, colour profile 2 or 3) or an invalid combination: colour profile 0 with chroma cell size 1, a transfer tile with a colour profile other than 0 (chapter 3, 3.3), or colour profile 1 at a size whose colour layer has fewer than 16 codewords (chapter 7, 7.8.2).

After a copy has been decoded (2.7), in this order:

| Condition | Reader behaviour |
|---|---|
| Format version 0 and any of: width code 0 or height code 0 (W or H = 16); colour profile 2 or 3; colour profile 0 with chroma cell size 1; symbol class 1 with a colour profile other than 0; colour profile 1 with fewer than 16 colour codewords for its W, H and chroma cell size (chapter 7, 7.8.2) | The copy counts as not decoded (2.7, step 3). Every reader computes the colour-layer size from W, H and the chroma cell size, including a reader that supports only black and white |
| Format version 0 and W or H that disagree with the finders the reader found (chapter 5, 5.11) | The copy counts as not decoded (2.7, step 3) |
| Format version 1, 2 or 3 in the chosen word | Reject the symbol. Report that a newer reader is needed (`E_FORMAT_VERSION`, chapter 9, 9.2) only when both copies decoded to this word, or when its copy has 2e + s ≤ 4 (e errors and s erasures, 2.7). Otherwise report `E_FORMAT_UNREADABLE` |
| W × H above the largest area the reader accepts | Reject the symbol with `E_SIZE_LIMIT` (Unsupported). The largest area is the reader's choice |
| Symbol class 1 | A reader that does not implement transfer tiles MUST NOT present content, and SHOULD tell the user that the code is one frame of a transfer (`E_TRANSFER_UNSUPPORTED`, reported after the CRC-32C has passed, chapter 3, 3.9) |
| Colour profile 1 | Continue as chapter 7 says, including for a reader that supports only black and white (chapter 1, 1.6) |

The report rule for format versions 1 to 3 exists because a reflectance-reversed copy decodes, with e = 3, to format version 2 (2.5). Both copies of a fully reversed symbol decode to the same such word; the polarity rule of chapter 1 (1.4) keeps a reader from reading a reversed symbol as upright.

Every other value of the width code, the height code and the error-correction level is defined. Every allowed size, 20 × 20 and up, has at least 20 codewords, which meets the minimum of chapter 4 (4.6) at every level. Chapter 7 (7.8.2) gives the smallest sizes that leave 16 colour codewords.

## 2.4 Code

### 2.4.1 Parameters

| Parameter | Value |
|---|---|
| Code | Binary cyclic code of length 63, shortened to 47 |
| Word length | 47 bits: 28 data bits and 19 parity bits |
| Minimum distance | 8 |
| Correction | up to 3 bit errors, or e bit errors and s erasures with 2e + s ≤ 7 and s ≤ 4 (2.7). Every pattern of 4 bit errors is detected |
| Generator polynomial | g(x) = x^19 + x^15 + x^10 + x^9 + x^8 + x^6 + x^4 + 1 |
| Generator as an integer | 0x88751 (bit i is the coefficient of x^i) |

### 2.4.2 Construction

Let α be a root of x^6 + x + 1 in GF(64). The minimal polynomials of α, α^3 and α^5 over GF(2) are:

    m1(x) = x^6 + x + 1
    m3(x) = x^6 + x^4 + x^2 + x + 1
    m5(x) = x^6 + x^5 + x^2 + x + 1

m1 · m3 · m5 (octal 1701317) generates the triple-error-correcting BCH code of length 63 with 45 data bits. It was published by Bose and Ray-Chaudhuri in 1960 and Hocquenghem in 1959. The format word's generator adds the factor x + 1:

    g(x) = (x + 1) · m1(x) · m3(x) · m5(x)

The roots of g include α^0, α^1, …, α^6, which are seven consecutive powers of α. By the BCH bound the cyclic code generated by g, of length 63 with 44 data bits, has minimum distance at least 8. Setting the 16 highest data positions to zero and not sending them gives the (47, 28) code used here. Shortening keeps the minimum distance.

The weight distribution of the (47, 28) code, computed from its dual code by the MacWilliams identity, is: 1 word of weight 0; 1841 of weight 8; 17 063 of weight 10; 203 290 of weight 12; and so on up to 1 of weight 44. There are no words of odd weight. The minimum distance is exactly 8. The word of weight 44, c44, has zeros at the bit indices 1, 13 and 31 of 2.4.3; its data part is 0xBFFBFFF.

The minimum weights of the 2^19 cosets of the code, which are the distances from a received word to the nearest codeword, are distributed as follows:

| Coset weight | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Cosets | 1 | 47 | 1081 | 16 215 | 126 685 | 244 332 | 134 377 | 1550 |

The covering radius is 7: every 47-bit word is within 7 bits of a codeword.

### 2.4.3 Systematic encoding

With the data polynomial d(x) = d[27]·x^27 + … + d[0]:

    p(x) = (d(x) · x^19) mod g(x)          # degree ≤ 18
    u(x) = d(x) · x^19 + p(x)              # degree ≤ 46

As integers: U = (d << 19) | P, where P is the 19-bit remainder. U is called the codeword. It is sent most significant bit first. Bit index i, 0 ≤ i ≤ 46, is the coefficient of x^(46 − i). Bits 0 to 27 are therefore d[27] … d[0], and bits 28 to 46 are the parity.

Division by a shift register:

    rem = 0                                # 19 bits
    for i in 27 down to 0:
        fb  = d[i] XOR (bit 18 of rem)
        rem = (rem << 1) AND 0x7FFFF
        if fb = 1: rem = rem XOR 0x08751   # g without its x^19 term
    P = rem

## 2.5 Masks

Each copy has its own mask. Copy A is sent as F_A = U XOR MASK_A and copy B as F_B = U XOR MASK_B, with

    MASK_A = 0x51F3694EAFAA
           = 101 0001 1111 0011 0110 1001 0100 1110 1010 1111 1010 1010  (47 bits)
    MASK_B = 0x3FCA34BE1F26
           = 011 1111 1100 1010 0011 0100 1011 1110 0001 1111 0010 0110  (47 bits)

The derivations are published to show that the constants were not chosen for hidden properties:

- MASK_A is the most significant 47 bits of SHA-256 over the ASCII string `NMT Code format mask`.
- MASK_B is taken from SHA-256 over the ASCII string `NMT Code format mask B`, which is `482e1ccb7271d40f9cb6bf29fe51a5f0f936a39635267be20a648a4499f47c43`. Its 256 bits are cut into 47-bit pieces from the most significant end: bits 0 to 46, 47 to 93, 94 to 140, and so on. MASK_B is the first piece that meets both conditions below.
  1. MASK_A XOR MASK_B lies in a coset of minimum weight 6 or more (2.4.2).
  2. The all-light and all-dark words, read as copy B, are more than 3 bit errors from every codeword.

  The first piece, 0x24170E65B938, and the second, 0x7503E72DAFCA, give MASK_A XOR MASK_B a coset of weight 4 and fail condition 1. The third piece, bits 94 to 140, meets both.

The following properties were checked:

- Neither mask is a codeword, so no data value gives an all-light copy.
- Blank and solid areas: the all-light word is 5 bits from the nearest codeword under MASK_A and 5 under MASK_B; the all-dark word is 6 bits away under MASK_A and 4 under MASK_B. A blank or solid area without erasures therefore never decodes as a format copy, because step 2 of 2.7 corrects at most 3 errors.
- A turned symbol: a reader that takes a symbol turned by 180° for upright samples the modules of copy B where it expects copy A, and the other way round. With its expected mask removed, such a copy reads U XOR MASK_A XOR MASK_B. MASK_A XOR MASK_B = 0x6E395DF0B08C lies in a coset of weight 6, so the copy is more than 3 errors from every codeword. It also stays outside the bound 2e + s ≤ 7 for every choice of up to 4 erasures, since 2 · (6 − s) + s ≥ 8. Neither copy decodes, and the reader tries another orientation. Of the 2^19 cosets, 135 927 (25.9%) have weight 6 or more.
- A reflectance-reversed copy (every module inverted) reads NOT U under either mask. NOT U equals U XOR c44 with 3 more bits flipped (2.4.2), so it decodes, with e = 3, to U XOR c44. When U has format version 0, U XOR c44 has format version 2. The masks cannot change this, because NOT U does not depend on them. Chapter 1 (1.4) therefore makes polarity a matter of the finders, and 2.3 limits when a newer format version is reported.

## 2.6 Bits on modules

Bit i of F_A (2.4.3) is the value of module A[i] of copy A, and bit i of F_B the value of module B[i] of copy B, with A and B as defined in chapter 5 (5.5). A[i] lies next to the TL finder, and B[i] is its image turned by 180° about the symbol's centre, next to the BR finder. 1 is dark, 0 is light. Format modules are not whitened (chapter 5, 5.10).

Both copies carry the same codeword U under different masks, so their modules differ. A generator MUST write both.

## 2.7 Decoding

A reader decodes the format word as follows.

1. **Sample.** For each copy whose finder is visible, sample its 47 modules, relative to that copy's finder (chapter 5, 5.5), into a word F'. The reader MAY mark up to 4 of the 47 bits as erasures when it cannot tell their value, for example under glare. Set R = F' XOR MASK_A for copy A and R = F' XOR MASK_B for copy B.
2. **Correct.** Find the codeword U with e bit errors outside the erased positions and s erased positions such that 2e + s ≤ 7 and s ≤ 4. Without erasures this is e ≤ 3. If there is none, the copy is not decoded. Because the minimum distance is 8, such a U is unique when it exists. A reader MUST NOT accept a codeword beyond this bound, and MUST NOT accept a result that is not a codeword of the full generator g(x), including its factor x + 1.
3. **Check fields.** A copy of format version 0 is not decoded when a value or a combination of 2.3 is invalid: the width code or the height code is 0; the colour profile is 2 or 3; the colour profile is 0 with chroma cell size 1; the symbol class is 1 with a colour profile other than 0; the colour profile is 1 and the colour layer of W, H and the chroma cell size has fewer than 16 codewords (chapter 7, 7.8.2). It is also not decoded when W and H disagree with the finders the reader found; chapter 5 (5.11) defines that check and its tolerance. A copy with format version 1, 2 or 3 is decoded; only its version field is read.
4. **Choose.**
   - If no copy is decoded, the reader MUST reject the symbol (`E_FORMAT_UNREADABLE`) and MUST NOT present any content from it.
   - If exactly one copy is decoded, use it.
   - If both copies decode to the same word, use it.
   - If both copies decode, to different words, the reader MUST reject the symbol (`E_FORMAT_CONFLICT`). It MUST NOT present content, and it MUST NOT read the base layer under either word. Reason: the Reed-Solomon codes of the four levels are nested (chapter 4), so a symbol can be built whose base layer passes every check under two levels and gives two contents; which one a reader shows would depend on which copy it trusts.
5. **Apply 2.3** to the chosen word: a format version 1, 2 or 3 rejects the symbol, and an area W × H above the reader's largest area rejects it with `E_SIZE_LIMIT`.

Content is presented only if the base layer, read with the chosen format word, passes every check of chapters 3 to 6, including the format echo byte of chapter 3 (3.2.2).

Permitted variations:

- A reader MAY decode with any method that meets step 2. Three methods:
  - A table of the 17 344 syndromes of the error patterns of weight 0 to 3 (19-bit syndrome R mod g(x)). Every result is then a codeword of g(x).
  - With erasures, the same table used twice: fill the erased positions with 0 and correct the result within 3 errors, then fill them with 1 and do the same, and keep a result that meets 2e + s ≤ 7. One filling is within e + ⌊s/2⌋ ≤ 3 bits of the codeword.
  - A Berlekamp–Massey decoder for the length-63 code with the 16 unsent positions set to zero. A result that places an error on an unsent position is a failure. That decoder uses the syndromes of m1 · m3 · m5 only and ignores the factor x + 1, so the reader MUST then confirm the result, for example by recomputing U mod g(x) = 0. Without this confirmation, about 8% of the patterns of 4 bit errors turn into a wrong word instead of a failure.
- A reader MAY combine the soft values of the two copies, each unmasked with its own mask, before deciding each bit. The combined word counts as one decoded copy: step 2 applies to it. A reader that also decodes the copies separately applies step 4 to them.

Integrity note: a random received word passes step 2 with the probability below, which depends on the number s of erased positions. With s erasures the other 47 − s positions carry 2^28 distinct codewords at least 8 − s apart, and each accepts the words within ⌊(7 − s)/2⌋ bits.

| Erasures s | Errors e allowed | Received words accepted | Share of random words |
|---:|---:|---|---:|
| 0 | 3 | 17 344 of every 2^19 | 3.31% |
| 1 | 3 | 16 262 of every 2^18 | 6.20% |
| 2 | 2 | 1036 of every 2^17 | 0.79% |
| 3 | 2 | 991 of every 2^16 | 1.51% |
| 4 | 1 | 44 of every 2^15 | 0.13% |

Step 3, agreement between copies, the format echo byte and the checks of chapters 3 and 4 then apply. The format word gives no protection against deliberate forgery.

## 2.8 Worked example

W = 20, H = 20 (the smallest symbol), static class, error-correction level 1, colour profile 0, chroma cell size 0, format version 0. Values computed by a script that implements 2.2 to 2.6.

Fields:

| Field | Value | Bits |
|---|---|---|
| Format version | 0 | `00` |
| Symbol class | 0 | `0` |
| Width code | 20/4 − 4 = 1 | `0000000001` |
| Height code | 1 | `0000000001` |
| Error-correction level | 1 | `01` |
| Colour profile | 0 | `00` |
| Chroma cell size | 0 | `0` |

| Quantity | Hex | Binary (most significant first) |
|---|---|---|
| Data d (28 bits) | 0x0008028 | `0000000000001000000000101000` |
| Parity P (19 bits) | 0x39567 | `0111001010101100111` |
| Codeword U (47 bits) | 0x000401439567 | `00000000000010000000001010000111001010101100111` |
| MASK_A | 0x51F3694EAFAA | `10100011111001101101001010011101010111110101010` |
| Copy A, F_A = U XOR MASK_A | 0x51F7680D3ACD | `10100011111011101101000000011010011101011001101` |
| MASK_B | 0x3FCA34BE1F26 | `01111111100101000110100101111100001111100100110` |
| Copy B, F_B = U XOR MASK_B | 0x3FCE35FD8A41 | `01111111100111000110101111111011000101001000001` |

Module values of both copies in the 20 × 20 symbol. `#` dark, `o` light, `F` finder module (chapter 5, 5.3), `-` separator, `.` data (left unfilled). Compare with the map in chapter 5, 5.12:

            1111111111
        01234567890123456789
     0  FFFFF-#o#o....-FFFFF
     1  FFFFF-oo##....-FFFFF
     2  FFFFF-###o....-FFFFF
     3  FFFFF-###o....-FFFFF
     4  FFFFF-##o#....-FFFFF
     5  ------ooo.....------
     6  o#o###..............
     7  o#oo##..............
     8  oo##oo..............
     9  o##oo#..............
    10  ..............#ooo##
    11  ..............oo#oo#
    12  ..............o#oo##
    13  ..............oo####
    14  ------.....#o#------
    15  FFFFF-....o##o-FFFFF
    16  FFFFF-....oo##-FFFFF
    17  FFFFF-....#oo#-FFFFF
    18  FFFFF-....####-FFFFF
    19  FFFFF-....###o-FFFFF

For example, bit 0 of F_A is 1, so module A[0] = (6, 0) is dark, and bit 0 of F_B is 0, so module B[0] = (13, 19) is light. Bit 46 is 1 in both: A[46] = (5, 9) and B[46] = (14, 10) are dark.

Decoding with errors: suppose copy A is read with bits 3, 17 and 30 flipped, as F' = 0x59F7480C3ACD. Then R = F' XOR MASK_A has syndrome R mod g(x) = 0x1135D. The syndrome table gives the error pattern at bits 3, 17 and 30, and the corrected data is 0x0008028, with e = 3. If copy B is read without error (e = 0), both copies give the same word and step 4 uses it.

Decoding with erasures: suppose copy A is read as 0x53F7480C3ACD, with bits 5, 17 and 30 flipped, and the reader marks bits 3, 17, 30 and 44 as erasures (mask 0x080020010004). The values of the erased bits are ignored. One error remains outside them, at bit 5: e = 1, s = 4 and 2e + s = 6 ≤ 7, so the copy decodes to 0x0008028. With a second error, at bit 6, 2e + s would be 8 and the copy would not be decoded.

A turned reading: a reader that takes the symbol turned by 180° for upright reads F_B where it expects copy A. R = F_B XOR MASK_A has syndrome 0x65BC8, which lies in a coset of weight 6; F_A read as copy B gives the same syndrome. Neither copy decodes.

A reversed reading: with every module inverted, copy A reads NOT F_A = 0x2E0897F2C532 and copy B reads NOT F_B = 0x4031CA0275BE. Each decodes with e = 3 to the data 0xBFF3FD7 (U XOR c44), whose format version field is `10` (2). One such copy alone gives `E_FORMAT_UNREADABLE` (2e = 6 > 4); both together give `E_FORMAT_VERSION` (2.3).
