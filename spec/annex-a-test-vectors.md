# NMT Code — Annex A. Test vectors

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder).

This annex is informative. It indexes the worked examples of chapters 1 to 8 and adds one complete symbol computed from the text of this specification.

## A.1 Index of worked examples

| Chapter | Section | What it tests | Exact values |
|---|---|---|---|
| 1 | 1.4 | CRC-32C parameters | check value of ASCII `123456789` = 0xE3069283; byte order in 3.7 |
| 2 | 2.8 | Format word fields, BCH(47, 28) parity, mask, bit positions of both copies, correction of 3 bit errors | d, P, C, MASK, F in hex and binary; 20 × 20 module map; corrupted word 0x59F7480C3ACD and syndrome 0x1135D |
| 2 | 2.5 | Mask derivation | MASK = 0x51F3694EAFAA from SHA-256 of `NMT Code format mask` |
| 3 | 3.1 | LEB128 byte order | 0, 127, 128, 300, 16384, 2^32 − 1 |
| 3 | 3.10 a | Static container, codec 0, padding to the capacity of a 28 × 28 symbol at level 0 | full 56-byte message |
| 3 | 3.10 b | Container with UTF-8 text, codec 0 | CRC-32C 0xFFF6057C |
| 3 | 3.10 c | Transfer-tile container (provisional layout) | CRC-32C 0xAC39AE69 |
| 3 | 3.10 d | Record-list form: action, file name, file | CRC-32C 0x3855D91B |
| 3 | 3.10 e | Colour symbol: digest, base and extension containers | SHA-256 digest; CRC-32C 0x0506D2A1 and 0xA2ECCDA1 |
| 3 | 3.10 f | Container with codec 3, dictionary ID and decoded length | 28 bytes; CRC-32C 0x3356F16E |
| 4 | 4.6 | Smallest symbol at each level | N, P, K for 20 × 20 |
| 4 | 4.7 | Block split for sample N at every level | B, block lengths, P, K |
| 4 | 4.11.1 | Generator polynomial g_16, parity of one block, zero syndromes | coefficients and parity in hex |
| 4 | 4.11.2 | Codeword stream order with unequal blocks | stream index to (block, byte) |
| 5 | 5.6.1 | Reference-mark lines | lines for sides 48, 64, 100 |
| 5 | 5.7 | Data-module count D, codewords N, remainder R | table from 20 × 20 to 4108 × 4108 |
| 5 | 5.10.1 | Whitening sequence | seed 0x15D9C3FC; w[0 … 63] = 0x2BB387F8EC5F707F |
| 5 | 5.12 | Function-module map of 20 × 20, placement order, whitening of the first 32 data modules, remainder modules | table of P[k], b[k], w[k] |
| 6 | 6.8.2 | Token table transcription | SHA-256 of the serialised table |
| 6 | 6.12.1 | Codec 1 | `03 15 9A 9A 40` |
| 6 | 6.12.2 | Codec 2 | 16 bytes |
| 6 | 6.12.3 | Codec 3: greedy parse, model 0, range coder | 19 bytes |
| 6 | 6.12.4 | Codec 4: H and A modes | 18 bytes |
| 6 | 6.12.5 | Encoder selection by whole-container size | coded and container sizes per codec |
| 7 | 7.8.4 | Chroma whitening sequence | seed 0x644E9D0D; u[0 … 63] = 0xC89D3A1B18E9D587 |
| 7 | 7.13 | Reference cells, data cells and module colours of profile 1 with c = 2; classification of two modules | cell tables; normalised values, t_m, e_m |
| 8 | 8.8.1 | Bootstrap QR data and error-correction codewords at 3-M | 44 + 26 codewords in hex |
| 8 | 8.8.2 | Bootstrap placement, gap and canvas | coordinates for three cases |
| A | A.2 | Complete static symbol from content to module matrix | every intermediate value below |

## A.2 End-to-end symbol: a URL at level 0

Input and choices:

| Item | Value |
|---|---|
| Content | `https://github.com/needmoretruth/nmtcode` (40 bytes, ASCII), content type 2 (URL), single-record form |
| Symbol class, format version | 0 (static), 0 |
| Colour profile, chroma cell size | 0 (black and white), 0 |
| Error-correction level | 0 |
| QR bootstrap | off (chapter 8, 8.5) |
| Quiet zone | 2 modules, light; not part of the matrix below |

### A.2.1 Codec

By the selection rule of chapter 6 (6.3) and the sizes of 6.12.5, the whole container is 47 bytes with codec 0, 28 with codec 3, 44 with codec 4 and 49 with codec 5 (one brotli implementation at quality 11). Codecs 1 and 2 do not apply (lower-case letters). The encoder chooses codec 3 with dictionary 0.

### A.2.2 Container

The container is example f of chapter 3 (3.10):

```
03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF
DF 35 76 47 AE BA F7 91 33 56 F1 6E
```

### A.2.3 Symbol size and block split

The size is the smallest square symbol whose message capacity at level 0 holds the 28-byte container. 20 × 20 gives K = 16; 24 × 24 gives:

| Quantity | Value | Rule |
|---|---|---|
| D | 338 | chapter 5, 5.7 (no reference marks below 48) |
| N = floor(D / 8), R = D mod 8 | 42, 2 | chapter 5, 5.7 |
| B, n, P | 1, 42, 8 | chapter 4, 4.6: P = 2 · ceil_div(42 · 15, 200) |
| K | 34 | N − B · P |

A generator may choose a size freely. The rectangles 20 × 28 and 28 × 20 (560 modules, K = 34) also hold this container and are smaller in area than 24 × 24 (576 modules).

### A.2.4 Message and codeword stream

Message: the container and 6 padding bytes (chapter 3, 3.8), 34 bytes:

```
03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF
DF 35 76 47 AE BA F7 91 33 56 F1 6E EC 11 EC 11
EC 11
```

Parity p[0 … 7] of the single block (chapter 4, 4.4):

```
5F BF C3 A7 FC 87 2F A6
```

With one block the codeword stream c[0 … 41] is the message followed by the parity (chapter 4, 4.8.2):

```
03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF
DF 35 76 47 AE BA F7 91 33 56 F1 6E EC 11 EC 11
EC 11 5F BF C3 A7 FC 87 2F A6
```

### A.2.5 Format word

Width code and height code 24 / 4 − 4 = 2, level 0, colour profile 0, chroma cell size 0.

| Quantity | Hex |
|---|---|
| Data d (28 bits) | 0x0010040 |
| Parity P (19 bits) | 0x7D88F |
| Codeword C (47 bits) | 0x00080207D88F |
| Sent word F = C XOR MASK | 0x51FB6B497725 |

Bit i of F is the value of module A[i] = (x, y) of chapter 5 (5.5) and of module B[i] = (23 − x, 23 − y).

### A.2.6 Placement and whitening

Codeword bits b[0 … 335] go on the data modules in placement order (chapter 5, 5.8 and 5.9), most significant bit first. The 2 remainder modules P[336] = (22, 6) and P[337] = (23, 6) carry b = 0. Every data module is XORed with w[k] (chapter 5, 5.10); w[336] = 1 and w[337] = 0.

### A.2.7 Module matrix

Row y, then the 24 modules from x = 0 to x = 23 (1 = dark), then the same row as 24 bits in hexadecimal, x = 0 as the most significant bit:

```
 0  111110101010010010011111  FAA49F
 1  100110001101100010011001  98D899
 2  100110111111011111011111  9BF7DF
 3  111110011000011000011001  F98619
 4  111110110100111111011111  FB4FDF
 5  000000011111011111000000  01F7C0
 6  010000110110100000011010  43681A
 7  101101001100101110100100  B4CBA4
 8  001110111111011001110011  3BF673
 9  011101110010011011011000  7726D8
10  000011010000000001100101  0D0065
11  100010111011101001010100  8BBA54
12  100111111011000011000110  9FB0C6
13  001110010010001100001000  392308
14  101110101010001010101110  BAA2AE
15  100100100111010111011100  9275DC
16  010000111111101100101101  43FB2D
17  011001110001000110000010  671182
18  000000110100110110000000  034D80
19  111110001111101011011111  F8FADF
20  111110111001110110010011  FB9D93
21  111110011011011111010101  F9B7D5
22  111110011001111100011001  F99F19
23  111110111000110101011111  FB8D5F
```

The same matrix drawn with `#` for dark and `.` for light:

```
#####.#.#.#..#..#..#####
#..##...##.##...#..##..#
#..##.######.#####.#####
#####..##....##....##..#
#####.##.#..######.#####
.......#####.#####......
.#....##.##.#......##.#.
#.##.#..##..#.###.#..#..
..###.######.##..###..##
.###.###..#..##.##.##...
....##.#.........##..#.#
#...#.###.###.#..#.#.#..
#..######.##....##...##.
..###..#..#...##....#...
#.###.#.#.#...#.#.#.###.
#..#..#..###.#.###.###..
.#....#######.##..#.##.#
.##..###...#...##.....#.
......##.#..##.##.......
#####...#####.#.##.#####
#####.###..###.##..#..##
#####..##.##.#####.#.#.#
#####..##..#####...##..#
#####.###...##.#.#.#####
```

Reading this matrix back by chapters 2, 5, 4, 3 and 6 in that order gives both format copies equal to F, a codeword stream with zero syndromes, a CRC-32C that matches, and the 40 content bytes.
