# NMT Code — Annex A. Test vectors

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft).

This annex is informative. It indexes the worked examples of chapters 1 to 8 (A.1) and adds three test vectors computed from the text of this specification: a complete symbol from content to module matrix (A.2), a symbol drawn inside another (A.3), and length fields at the limit of their range (A.4).

## A.1 Index of worked examples

| Chapter | Section | What it tests | Exact values |
|---|---|---|---|
| 1 | 1.4 | CRC-32C parameters | check value of ASCII `123456789` = 0xE3069283; byte order in 3.7 |
| 2 | 2.4.2 | Weight distribution and coset weights of the (47, 28) code | 1841 words of weight 8, 17 063 of weight 10, 203 290 of weight 12; the word of weight 44 has data 0xBFFBFFF; coset counts 1, 47, 1081, 16 215, 126 685, 244 332, 134 377, 1550 for weights 0 to 7 |
| 2 | 2.5 | Derivation and properties of the two masks | MASK_A = 0x51F3694EAFAA from SHA-256 of `NMT Code format mask`; MASK_B = 0x3FCA34BE1F26, the third 47-bit piece of SHA-256 of `NMT Code format mask B`, after 0x24170E65B938 and 0x7503E72DAFCA (cosets of weight 4); MASK_A XOR MASK_B = 0x6E395DF0B08C, coset weight 6; the all-light and all-dark words are 5 and 6 bits from a codeword under MASK_A, 5 and 4 under MASK_B |
| 2 | 2.7 | Received words accepted with s erasures | 17 344 of 2^19, 16 262 of 2^18, 1036 of 2^17, 991 of 2^16, 44 of 2^15 for s = 0 to 4 |
| 2 | 2.8 | Format word of a 20 × 20 symbol at level 1: fields, parity, both copies, their modules; correction with errors and with erasures; a turned and a reversed reading | d = 0x0008028, P = 0x39567, U = 0x000401439567, F_A = 0x51F7680D3ACD, F_B = 0x3FCE35FD8A41; 20 × 20 map of both copies; 3 errors: 0x59F7480C3ACD, syndrome 0x1135D; 1 error and 4 erasures: 0x53F7480C3ACD, erasure mask 0x080020010004; turned: syndrome 0x65BC8, coset weight 6, no copy decodes; reversed: 0x2E0897F2C532 and 0x4031CA0275BE decode to data 0xBFF3FD7 |
| 3 | 3.1 | LEB128 byte order | 0, 127, 128, 300, 16384, 2^32 − 1 |
| 3 | 3.2.2 | Format echo byte | `00` (level 0), `40` (level 1), `10` (colour profile 1, level 0, 1 × 1 cells) |
| 3 | 3.2.6 | Header sizes | 4 bytes `00 00 29 02` (codec 0), 6 bytes `03 00 16 00 28 02` (codec 3), 5 bytes for 200 bytes of text; smallest container 8 bytes |
| 3 | 3.10 a | Static container, codec 0, padding to the capacity of a 28 × 28 symbol at level 0 | 48 bytes, CRC-32C 0x0578960C; full 56-byte message |
| 3 | 3.10 b | UTF-8 text, codec 0, level 1 | echo byte `40`; CRC-32C 0x267D9BA5; 32 bytes |
| 3 | 3.10 c | Transfer-tile container (provisional layout) | CRC-32C 0xAC39AE69; 33 bytes |
| 3 | 3.10 d | Record-list form: action, file name, file | CRC-32C 0x21C2FFFD; 69 bytes |
| 3 | 3.10 e | Colour symbol: digest, base and extension containers | 33-byte digest input; SHA-256 `88 42 74 7E … BE F9 EA D5`; CRC-32C 0x03DB0AA4 (base, 49 bytes) and 0x28CA4D68 (extension, 23 bytes) |
| 3 | 3.10 f | Codec 3 with dictionary ID and decoded length | 29 bytes; CRC-32C 0x137DCFC4 |
| 4 | 4.5 | Module-error rate at which a symbol fails with probability 1% | 20 × 20 to 324 × 324 at levels 0 to 3, from 0.29% (20 × 20, level 0) to 3.25% (64 × 64, level 3) |
| 4 | 4.6 | Smallest symbol at each level | 20 × 20: N = 20; P = 4, 6, 10, 12; K = 16, 14, 10, 8; largest content of one record with codec 0: 8, 6, 2, 0 bytes |
| 4 | 4.7 | Block split for sample N at every level | B, block lengths, P, K and parity share for N = 32, 50, 64, 128, 512, 1311, 13000 |
| 4 | 4.11.1 | Generator polynomial g_16, parity of one block, zero syndromes | g_16 = `01 3B 0D 68 … 24 3B`; parity `36 5A E8 16 6F 40 AC F0 72 10 3F 0F BB 20 DD F4` |
| 4 | 4.11.2 | Codeword stream order with unequal blocks | N = 512 at level 1: n = 171, 171, 170, P = 52, K = 356; stream index to (block, byte) |
| 5 | 5.6.1 | Reference-mark lines | 48: 2, 24, 45; 64: 2, 22, 41, 61; 100: 2, 26, 50, 73, 97 |
| 5 | 5.7 | Data-module count D, codewords N, remainder R | table from 20 × 20 to 4108 × 4108 |
| 5 | 5.10.1 | Whitening sequence | seed 0x15D9C3FC; w[0 … 63] = 0x2BB387F8EC5F707F |
| 5 | 5.12 | Function-module map of 20 × 20, placement order, whitening of the first 32 data modules, remainder modules | map; P[k], b[k], w[k] for k = 0 to 31 with the stream `4E 4D 54 20 …`; P[160] = (18, 6), P[161] = (19, 6) |
| 6 | 6.8.2 | Token table transcription | serialisation of 1,794 bytes, SHA-256 `a8e0f1be…3e3717` |
| 6 | 6.8.5 | Canonical termination of codec 3 | the 19 bytes of 6.12.3 followed by `01` are rejected (`Lc` = 20, `p` = 22); `a` under model 0 codes to `56`, and `55 55 55 55` is rejected |
| 6 | 6.12.1 | Codec 1 | `03 15 9A 9A 40` |
| 6 | 6.12.2 | Codec 2 | `63 54 CA 8C 7B A5 2E 76 23 D2 A0 46 90 24 E6 98` (16 bytes) |
| 6 | 6.12.3 | Codec 3: greedy parse, model 0, range coder | 24 symbols, `M` = 22; `DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91` (19 bytes) |
| 6 | 6.12.4 | Codec 4: H and A modes | `32 90 2A AD 2B 0A 9C 59 2F FF 54 E9 B5 10 43 DF 93 28` (18 bytes) |
| 6 | 6.12.5 | Encoder selection by whole-container size | `Lc` and container size per codec for four contents; chosen codecs 1, 2, 3, 4 |
| 7 | 7.8.2 | Smallest valid sizes of colour profile 1 | c = 1: 20 × 24 and 24 × 20 (N_c = 22), square 24 × 24 (34), 20 × 20 too small (12); c = 2: 24 × 36 and 36 × 24 (17), square 32 × 32 (22), 28 × 28 (15) and 24 × 32 (14) too small |
| 7 | 7.8.4 | Chroma whitening sequence | seed 0x644E9D0D; u[0 … 63] = 0xC89D3A1B18E9D587 |
| 7 | 7.13 | The colour symbol of 3.10 e: 32 × 32, level 0, colour profile 1, c = 1; reference groups, colour message, first 16 data cells, classification of two modules | d = 0x0020082, F_A = 0x51E36D589747, F_B = 0x3FDA30A827CB; N_cells = 786, N_data = 722, N_c = 90, one block with P = 46, K_c = 44; the four 4 × 4 reference groups; 46 parity bytes `3E F4 A3 … F9 43`; colours of d(0) … d(15); t_m = 1.1164 and 0.4511, e_m = 0.6164 and −0.0489 |
| 8 | 8.5 | Canvas with the bootstrap QR against a plain QR Code of the same content | 10 sizes, from 20 × 32 (canvas 2627, plain QR 1369) to 128 × 128 (22 378 and 21 025) |
| 8 | 8.8.1 | Bootstrap QR data and error-correction codewords at 3-M | 44 + 26 codewords in hex |
| 8 | 8.8.2 | Bootstrap placement, gap and canvas | coordinates for four cases |
| A | A.2 | Complete static symbol from content to module matrix | every intermediate value below |
| A | A.3 | A symbol drawn inside another | the construction below; `E_NESTED_SYMBOL` |
| A | A.4 | Body length and record value length of 2^32 − 1 | the messages below; `E_LENGTH_FIELD` and `E_RECORD_LIST` |

The robustness figures of chapter 5 (5.3.2, property 3) come from a random simulation and are not test vectors.

## A.2 End-to-end symbol: a URL at level 0

Input and choices. They are the defaults of the `screen` profile (chapter 1, 1.5).

| Item | Value |
|---|---|
| Content | `https://github.com/needmoretruth/nmtcode` (40 bytes, ASCII), content type 2 (URL), single-record form |
| Symbol class, format version | 0 (static), 0 |
| Colour profile, chroma cell size | 0 (black and white), 0 |
| Error-correction level | 0 |
| Codec | the selection rule of chapter 6 (6.3) in its default mode |
| Size | the RECOMMENDED size rule of chapter 1 (1.5) |
| QR bootstrap | off (chapter 8, 8.5) |
| Quiet zone | 2 modules, light; not part of the matrix below |

### A.2.1 Codec

By the selection rule of chapter 6 (6.3) and the sizes of 6.12.5, the whole container is 48 bytes with codec 0, 29 with codec 3, 45 with codec 4 and 50 with codec 5 (one brotli implementation at quality 11). Codecs 1 and 2 do not apply (lower-case letters). The encoder chooses codec 3 with dictionary 0 and the reference parse of 6.8.6.

### A.2.2 Container

The container is example f of chapter 3 (3.10), 29 bytes:

```
03 00 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B
EF DF 35 76 47 AE BA F7 91 13 7D CF C4
```

### A.2.3 Symbol size and block split

The size rule of chapter 1 (1.5) takes the sizes whose sides are within a factor of 2 of each other and whose message capacity K at level 0 holds the 29-byte container, and picks the smallest area:

| Size | Area | N | K at level 0 | Holds the container |
|---|---:|---:|---:|---|
| 20 × 20 | 400 | 20 | 16 | no |
| 20 × 24, 24 × 20 | 480 | 30 | 24 | no |
| 20 × 28, 28 × 20 | 560 | 40 | 34 | yes |
| 24 × 24 | 576 | 42 | 34 | yes |

20 × 28 and 28 × 20 have the same area and are equally far from square, so the rule picks the one with the larger H: 20 × 28. The smallest square that holds the container is 24 × 24.

| Quantity | Value | Rule |
|---|---|---|
| D | 322 | chapter 5, 5.7 (no reference marks: both sides are below 48) |
| N = floor(D / 8), R = D mod 8 | 40, 2 | chapter 5, 5.7 |
| B, n, P | 1, 40, 6 | chapter 4, 4.6: P = 2 · ceil_div(40 · 15, 200) |
| K | 34 | N − B · P |

### A.2.4 Message and codeword stream

Message: the container and 5 padding bytes (chapter 3, 3.8), 34 bytes:

```
03 00 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B
EF DF 35 76 47 AE BA F7 91 13 7D CF C4 EC 11 EC
11 EC
```

Parity p[0 … 5] of the single block (chapter 4, 4.4):

```
83 8C FB 28 C9 ED
```

With one block the codeword stream c[0 … 39] is the message followed by the parity (chapter 4, 4.8.2):

```
03 00 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B
EF DF 35 76 47 AE BA F7 91 13 7D CF C4 EC 11 EC
11 EC 83 8C FB 28 C9 ED
```

### A.2.5 Format word

Width code 20 / 4 − 4 = 1, height code 28 / 4 − 4 = 3, level 0, colour profile 0, chroma cell size 0.

| Quantity | Hex |
|---|---|
| Data d (28 bits) | 0x0008060 |
| Parity P (19 bits) | 0x466EB |
| Codeword U (47 bits) | 0x0004030466EB |
| Copy A, F_A = U XOR MASK_A | 0x51F76A4AC941 |
| Copy B, F_B = U XOR MASK_B | 0x3FCE37BA79CD |

Bit i of F_A is the value of module A[i] = (x, y) of chapter 5 (5.5), and bit i of F_B the value of module B[i] = (19 − x, 27 − y).

### A.2.6 Placement and whitening

Codeword bits b[0 … 319] go on the data modules in placement order (chapter 5, 5.8 and 5.9), most significant bit first. The 2 remainder modules P[320] = (18, 6) and P[321] = (19, 6) carry b = 0. Every data module is XORed with w[k] (chapter 5, 5.10); w[320] = 1 and w[321] = 0, so the two remainder modules are dark and light.

### A.2.7 Module matrix

Row y, then the 20 modules from x = 0 to x = 19 (1 = dark), then the same row as 20 bits in hexadecimal, x = 0 as the most significant bit:

```
 0  11111010100010011111  FA89F
 1  10011000110011011001  98CD9
 2  10011011101010011111  9BA9F
 3  11111011101000011001  FBA19
 4  11111011010011011111  FB4DF
 5  00000001001000000000  01200
 6  01110001001011000010  712C2
 7  10101001011111001011  A97CB
 8  01000011001100011111  4331F
 9  00010101010010110011  154B3
10  00011010010101000111  1A547
11  10111010010010101101  BA4AD
12  10011100100010111100  9C8BC
13  00011000101011001001  18AC9
14  10001100011001011101  8C65D
15  11010100101001111001  D4A79
16  00001011000110110100  0B1B4
17  11110000111011110001  F0EF1
18  10000101110000101101  85C2D
19  01100111111000000111  67E07
20  00111101010011110100  3D4F4
21  01110100001100111011  7433B
22  00000011111111000000  03FC0
23  11111001010110011111  F959F
24  11111011010011011001  FB4D9
25  11111000001001010001  F8251
26  11111011111111010001  FBFD1
27  11111011111110011111  FBF9F
```

The same matrix drawn with `#` for dark and `.` for light:

```
#####.#.#...#..#####
#..##...##..##.##..#
#..##.###.#.#..#####
#####.###.#....##..#
#####.##.#..##.#####
.......#..#.........
.###...#..#.##....#.
#.#.#..#.#####..#.##
.#....##..##...#####
...#.#.#.#..#.##..##
...##.#..#.#.#...###
#.###.#..#..#.#.##.#
#..###..#...#.####..
...##...#.#.##..#..#
#...##...##..#.###.#
##.#.#..#.#..####..#
....#.##...##.##.#..
####....###.####...#
#....#.###....#.##.#
.##..######......###
..####.#.#..####.#..
.###.#....##..###.##
......########......
#####..#.#.##..#####
#####.##.#..##.##..#
#####.....#..#.#...#
#####.########.#...#
#####.#######..#####
```

Reading this matrix back by chapters 2, 5, 4, 3 and 6 in that order gives copy A equal to F_A and copy B equal to F_B, both decoding to U with no error; a codeword stream with zero syndromes; a CRC-32C that matches; a format echo byte `00` that matches the format word; and the 40 content bytes.

## A.3 A symbol inside another

The reader rule of chapter 5 (5.11, step 6) for a symbol drawn inside the data area of another.

| Item | Value |
|---|---|
| Outer symbol | the text `outer content` (13 bytes, content type 1) at level 0 in a 96 × 96 symbol, codec by 6.3. Container: `03 00 0D 00 0D 01 A0 2E 26 0D 71 12 C9 D4 BE C9 F1 AC 44 FC` |
| Inner symbol | the text `inner content` at level 0 in the size of the rule of 1.5, 20 × 24, codec by 6.3. Container: `03 00 0D 00 0D 01 80 FE D4 15 91 12 C9 D4 BE C9 9D 0B 07 FC` |
| Construction | In the module matrix of the outer symbol, set the modules with 36 ≤ x < 60 and 36 ≤ y < 64 light: the inner symbol and a quiet zone of 2 modules around it. Then copy the matrix of the inner symbol so that its module (0, 0) is the outer module (38, 38). 349 modules of the outer symbol change, among them the reference mark centred at (48, 48) |
| Image | the resulting 96 × 96 matrix with a light quiet zone of 2 modules |

A reader that finds both symbols presents neither and reports `E_NESTED_SYMBOL` (Malformed). Read on its own, the inner symbol decodes to `inner content`, and the base layer of the changed outer symbol fails error correction (`E_ECC_FAILED`, chapter 4, 4.9, with a decoder that corrects within 2e ≤ P).

## A.4 Length fields at the limit

The overflow rule of chapter 1 (1.4) for the body length (chapter 3, 3.2.3) and a record's value length (3.2.5). Both vectors are 20 × 20 symbols at level 0: format data d = 0x0008020, U = 0x00040107AFEF, F_A = 0x51F768490045, F_B = 0x3FCE35B9B0C9; N = 20, one block, P = 4, K = 16. Each symbol is drawn from its message by chapters 4 and 5.

a) Body length Lb = 2^32 − 1. Message: lead byte `00`, format echo byte `00`, Lb as `FF FF FF FF 0F`, then padding:

```
00 00 FF FF FF FF 0F EC 11 EC 11 EC 11 EC 11 EC
```

Parity `5C 90 A8 87`. The reader rejects the symbol with `E_LENGTH_FIELD` (Damaged): 2 + 5 + Lb + 4 exceeds K = 16. Computed in 32 bits, the sum wraps to 10, which would pass.

b) Record value length 2^32 − 1. Container: lead byte `20` (record-list form, codec 0), format echo byte `00`, Lb = 8, record count 1, then one record with type ID 1, value length `FF FF FF FF 0F` and the value byte `41`; CRC-32C over these 11 bytes = 0x74007865:

```
20 00 08 01 01 FF FF FF FF 0F 41 74 00 78 65 EC
```

Parity `05 7D 80 1B`. The CRC-32C passes, and the reader rejects the symbol with `E_RECORD_LIST` (Malformed): the record's value runs past the 7 bytes of the decoded content. Computed in 32 bits, the end offset 6 + (2^32 − 1) wraps to 5, which would pass.
