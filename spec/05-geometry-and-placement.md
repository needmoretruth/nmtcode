# NMT Code — 5. Geometry and placement

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder).

## 5.1 Scope

This chapter defines, for the base (black-and-white) layer:

- the symbol size and the quiet zone (5.2);
- the four finder patterns and their separators (5.3, 5.4);
- the two areas that hold the format word (5.5);
- the reference marks (5.6);
- the number of data modules of any symbol (5.7);
- the order in which data modules are visited, how codeword bits are put on them, and the modules left after the last codeword (5.8, 5.9);
- whitening (5.10).

The format word's content and code are in chapter 2. The block structure and the codeword stream c[0], …, c[N−1] are in chapter 4 (4.6, 4.8). The colour layer is in chapter 7.

Coordinates follow chapter 1 (1.4): (x, y) in modules, origin at the top-left module of the symbol, quiet zone excluded.

## 5.2 Symbol size and quiet zone

| Parameter | Value |
|---|---|
| Width W | a multiple of 4, 16 ≤ W ≤ 4108 (format word, chapter 2) |
| Height H | a multiple of 4, 16 ≤ H ≤ 4108. W and H are independent |
| Quiet zone | at least 2 modules on each of the four sides, all light |

- A generator MUST surround the symbol with a light quiet zone of at least 2 modules on every side. The default is 2 modules.
- Nothing dark may be drawn inside the quiet zone. A QR bootstrap code (chapter 8) is placed outside it.

A module belongs to exactly one of these classes: finder (5.3), separator (5.4), format (5.5), reference mark (5.6), data (5.7). Finder, separator, format and reference-mark modules are **function modules**.

## 5.3 Finder patterns

### 5.3.1 Position and shape

Each corner of the symbol holds a 5 × 5 finder pattern whose outer corner is the symbol's corner:

| Finder | Modules |
|---|---|
| TL | 0 ≤ x ≤ 4, 0 ≤ y ≤ 4 |
| TR | W−5 ≤ x ≤ W−1, 0 ≤ y ≤ 4 |
| BL | 0 ≤ x ≤ 4, H−5 ≤ y ≤ H−1 |
| BR | W−5 ≤ x ≤ W−1, H−5 ≤ y ≤ H−1 |

Each finder is an outer ring one module wide, all dark, around an inner 3 × 3 that differs per corner. The four patterns, as they appear in the upright symbol (`#` dark, `o` light; left column is the finder's smallest x, top row its smallest y):

    TL          TR          BL          BR
    # # # # #   # # # # #   # # # # #   # # # # #
    # o o # #   # # o o #   # # # # #   # o o # #
    # o o # #   # # # # #   # # # # #   # o # o #
    # # # # #   # # o o #   # # # # #   # # o o #
    # # # # #   # # # # #   # # # # #   # # # # #

In words, the inner 3 × 3 is:

- TL: a 2 × 2 light hole in the corner of the inner area nearest the symbol's corner (inner rows `oo#`, `oo#`, `###`).
- TR: two 2 × 1 light slots against the ring's outer side, with the rest of the inner area dark (inner rows `#oo`, `###`, `#oo`).
- BL: all dark (the finder is a solid 5 × 5 square).
- BR: two light triangles on either side of a dark anti-diagonal (inner rows `oo#`, `o#o`, `#oo`).

### 5.3.2 Properties

The following were checked by exhaustive enumeration.

1. **Identity under rotation and mirroring.** Let T be any of the eight rotations and mirror images of a 3 × 3 array. For any two different finders F and G and any T, the inner 3 × 3 of F and T applied to the inner 3 × 3 of G differ in at least 4 of the 9 modules. A reader that finds three finders therefore knows which corner each one is, and the three corners fix the rotation and whether the image is mirrored.
2. **No QR finder signature.** In every row and every column that crosses a finder, for any values of the data and format modules beyond the separator:
   - no sequence of five runs, dark first, with lengths u, u, 3u, u, u (u ≥ 1) has its middle run on a finder module (the dark runs that contain finder modules have lengths 1, 2 or 5);
   - no sequence of five runs in the ratio 1:1:3:1:1, of either colour order, consists only of finder, separator and quiet-zone modules.

   Whitened data (5.10) contains 1:1:3:1:1 runs at the rate of random bits, including runs whose first or last run is a ring module. No function pattern forms the nested squares of a QR finder.
3. **Blur.** Model blur as a Gaussian point-spread function of standard deviation σ modules, integrated over each module, with a light quiet zone and separator and the data modules beyond set all dark or all light. Sample each module centre and threshold at half contrast. Then:
   - classifying the inner 3 × 3 by the nearest finder (the smallest Hamming distance over the eight transforms) returns the right finder for all four patterns and all eight transforms up to σ = 0.57 module;
   - every module of the finder itself reads correctly up to σ = 0.91 (BL), 0.74 (TL), 0.57 (TR) and 0.52 (BR).

   Blur of one module (full width at half maximum = 1 module, σ ≈ 0.42) is inside every one of these margins. The smallest light feature of every inner pattern is at least one module wide, like the light ring of a QR finder.

## 5.4 Separators

Each finder is bordered, on the two sides that face the inside of the symbol, by light modules one module wide:

| Finder | Separator modules (all light) |
|---|---|
| TL | x = 5 for 0 ≤ y ≤ 5; y = 5 for 0 ≤ x ≤ 5 |
| TR | x = W−6 for 0 ≤ y ≤ 5; y = 5 for W−6 ≤ x ≤ W−1 |
| BL | x = 5 for H−6 ≤ y ≤ H−1; y = H−6 for 0 ≤ x ≤ 5 |
| BR | x = W−6 for H−6 ≤ y ≤ H−1; y = H−6 for W−6 ≤ x ≤ W−1 |

That is 11 modules per corner, 44 in total. With the quiet zone on the two outer sides, every finder is a dark 5 × 5 square with a light border all round.

Why the separator is needed:

- The detector family this format is designed for (adaptive threshold, then contour or quadrilateral fitting, then a check of the inner pattern) finds a finder as a dark connected region with a closed light border. Without a separator, dark data modules touching the ring join the finder to the rest of the symbol, and the finder's outline is lost on its two inner sides.
- Properties 2 and 3 of 5.3.2 depend on a light border of known width. Without it, a dark data module next to the ring lengthens the ring's runs and moves the thresholded edge of the ring under blur.

The cost is 44 modules per symbol. `SEPARATOR_WIDTH` = 1 module; tunable in 0.x, fixed before 1.0, by the finder measurement on real captures. The data-module count in 5.7 assumes 1.

## 5.5 Format word areas

The format word (chapter 2) is 47 bits and is written twice: copy A next to the TL finder, copy B next to the BR finder.

Copy A occupies 47 modules, listed here in bit order (bit index i = 0 first):

    A[0..22]  = the modules (x, y) with 0 ≤ y ≤ 5, 6 ≤ x ≤ 9,
                in rows y = 0, 1, …, 5, each row x = 6, 7, 8, 9,
                except (9, 5)
    A[23..46] = the modules (x, y) with 0 ≤ x ≤ 5, 6 ≤ y ≤ 9,
                in columns x = 0, 1, …, 5, each column y = 6, 7, 8, 9

Copy B is copy A turned by 180° about the symbol's centre:

    B[i] = (W − 1 − x, H − 1 − y)   where A[i] = (x, y)

Explicit list of copy A:

| i | A[i] | i | A[i] | i | A[i] | i | A[i] |
|---|---|---|---|---|---|---|---|
| 0 | (6, 0) | 12 | (6, 3) | 24 | (0, 7) | 36 | (3, 7) |
| 1 | (7, 0) | 13 | (7, 3) | 25 | (0, 8) | 37 | (3, 8) |
| 2 | (8, 0) | 14 | (8, 3) | 26 | (0, 9) | 38 | (3, 9) |
| 3 | (9, 0) | 15 | (9, 3) | 27 | (1, 6) | 39 | (4, 6) |
| 4 | (6, 1) | 16 | (6, 4) | 28 | (1, 7) | 40 | (4, 7) |
| 5 | (7, 1) | 17 | (7, 4) | 29 | (1, 8) | 41 | (4, 8) |
| 6 | (8, 1) | 18 | (8, 4) | 30 | (1, 9) | 42 | (4, 9) |
| 7 | (9, 1) | 19 | (9, 4) | 31 | (2, 6) | 43 | (5, 6) |
| 8 | (6, 2) | 20 | (6, 5) | 32 | (2, 7) | 44 | (5, 7) |
| 9 | (7, 2) | 21 | (7, 5) | 33 | (2, 8) | 45 | (5, 8) |
| 10 | (8, 2) | 22 | (8, 5) | 34 | (2, 9) | 46 | (5, 9) |
| 11 | (9, 2) | 23 | (0, 6) | 35 | (3, 6) | | |

The module (9, 5) and its turned image (W − 10, H − 6) are data modules.

Design notes:

- Each copy lies within 10 modules of its finder's corner, in the two strips along the symbol's edges. The two copies sit at opposite corners, so covering any one corner, or any one edge, leaves at least one copy and three finders visible.
- A copy's positions are fixed relative to its own corner. A reader can read copy B from the BR finder alone, before it knows W and H.
- For W = 16 or H = 16 the two strips of a copy end exactly at the separator of the neighbouring finder. No format module overlaps another function module for any allowed W and H.

## 5.6 Reference marks

A reference mark is a 3 × 3 pattern: eight dark modules around a light centre.

    # # #
    # o #
    # # #

### 5.6.1 Positions

For a side of N modules (N = W for x, N = H for y), define the line count n and the line coordinates:

    S = N − 5
    n = 1                    if N < 48
    n = ceil_div(S, 24)      if N ≥ 48        # ceil_div(a, d) = floor((a + d − 1) / d)
    line[i] = 2 + floor((2·i·S + n) / (2·n))   for i = 0 .. n

line[i] is 2 + round(i·S/n), with halves rounded up. line[0] = 2 and line[n] = N − 3 are the centre lines of the finders. The lines divide the span between the finder centres into n equal parts, rounded to whole modules, so the last spacing is never a short remainder.

A reference mark is centred on every point (xline[i], yline[j]), 0 ≤ i ≤ n_W, 0 ≤ j ≤ n_H, except the four points (2, 2), (W−3, 2), (2, H−3), (W−3, H−3), which are finder centres. The mark occupies the 3 × 3 modules centred there.

The number of marks is

    M = (n_W + 1) · (n_H + 1) − 4

Consequences:

- Both sides below 48: no marks. One side of 48 or more: marks lie only on the two edge lines of that side, in line with the finder centres, for example along the top and bottom edges of a wide, short symbol.
- For N ≥ 48 consecutive lines are between 17 and 24 modules apart, checked for every N from 48 to 4108.
- Nothing is skipped other than the four finder centres. Every mark is at least 18 modules from both left and right edges, or from both top and bottom edges, because on a side of 48 or more line[1] ≥ 19 and line[n−1] ≤ N − 20. A finder, its separator and its format area lie within 10 modules of both edges at their corner. So no mark overlaps them, and marks, at least 17 modules apart, do not overlap each other.

Examples: N = 48 gives lines 2, 24, 45; N = 64 gives 2, 22, 41, 61; N = 100 gives 2, 26, 50, 73, 97.

Reader note (informative): a reader predicts each mark's position from the finders and the marks already found, then searches a small window by correlating the 3 × 3 template. The light centre gives a position that does not depend on the data modules outside the ring.

## 5.7 Data modules

Every module that is not a function module is a data module. Their number is

    D(W, H) = W·H − 100 − 44 − 94 − 9·M

(finders 4 × 25, separators 44, format copies 2 × 47, marks 9 each, M from 5.6.1). The codeword count used by chapter 4 is N = floor(D / 8). The remainder R = D mod 8 is defined in 5.9.

| W × H | Marks M | Data modules D | Codewords N | Remainder R |
|---|---|---|---|---|
| 16 × 16 | 0 | 18 | 2 | 2 |
| 20 × 20 | 0 | 162 | 20 | 2 |
| 24 × 24 | 0 | 338 | 42 | 2 |
| 32 × 32 | 0 | 786 | 98 | 2 |
| 48 × 48 | 5 | 2021 | 252 | 5 |
| 64 × 64 | 12 | 3750 | 468 | 6 |
| 128 × 128 | 45 | 15741 | 1967 | 5 |
| 324 × 324 | 221 | 102749 | 12843 | 5 |
| 1000 × 600 | 1114 | 589736 | 73717 | 0 |
| 4108 × 4108 | 29580 | 16609206 | 2076150 | 6 |

The formula was checked against a module-by-module count for every W and H from 16 to 296 in steps of 4, and for 1000 × 600.

A 16 × 16 symbol has N = 2, below the minimum of 3 codewords in chapter 4 (4.6), so it cannot be generated. The smallest usable sizes are 16 × 20 and 20 × 16 (D = 82, N = 10).

## 5.8 Placement order

The data modules are visited in one fixed order, the **placement order**. It walks the symbol in column pairs, alternately down and up, and skips every function module:

    k = 0
    for p in 0 .. W/2 − 1:                 # column pair p covers x = 2p and x = 2p + 1
        if p is even: ys = 0, 1, …, H − 1  # downward
        else:         ys = H − 1, …, 1, 0  # upward
        for y in ys:
            for x in (2p, 2p + 1):
                if (x, y) is a data module:
                    P[k] = (x, y); k = k + 1

At the end, k = D. P[k] is the k-th data module.

The eight bits of one codeword fall on neighbouring modules, normally a 2 × 4 block, so a small local defect damages few codewords. Because chapter 4 (4.8.2) interleaves the blocks in the codeword stream, neighbouring codewords belong to different blocks.

## 5.9 Codeword bits on modules

The codeword stream c[0], …, c[N−1] is the one defined in chapter 4 (4.8.2); it is already interleaved across blocks. Its bits are put on the data modules in placement order, each codeword most significant bit first:

    for i in 0 .. N − 1:
        for j in 0 .. 7:
            b[8i + j] = bit (7 − j) of c[i]      # bit 7 is the most significant

**Remainder bits.** The last R = D − 8N data modules, P[8N] … P[D−1], carry no codeword. Their bits are 0:

    b[k] = 0   for 8N ≤ k < D

They are whitened like every other data module (5.10). A reader MUST ignore them.

A reader reverses the mapping: it reads b[k] from module P[k], removes the whitening, and packs the bits into codewords. A reader that marks a module as unreliable SHOULD mark the codeword floor(k / 8) that contains it as an erasure (chapter 4, 4.9).

## 5.10 Whitening

Every data module, remainder modules included, is XORed with a fixed binary sequence w[0], w[1], …:

    module value at P[k] = b[k] XOR w[k]      # 1 = dark, 0 = light

Function modules are not whitened. The format word has its own mask (chapter 2).

### 5.10.1 Sequence

| Parameter | Value |
|---|---|
| Recurrence | w[k] = w[k−28] XOR w[k−31] for k ≥ 31 (feedback taps 28 and 31) |
| Characteristic polynomial | x^31 + x^3 + 1 (its reciprocal, x^31 + x^28 + 1, is the usual name of the same register) |
| Seed | w[0], …, w[30] = the 31 bits of 0x15D9C3FC, most significant first: `0010101110110011100001111111100` |
| Period | 2^31 − 1 |

Register form, equivalent to the recurrence: keep a 31-bit register r, initially 0x15D9C3FC. For each k:

    w[k] = bit 30 of r
    f    = (bit 30 of r) XOR (bit 27 of r)
    r    = ((r << 1) AND 0x7FFFFFFF) OR f

The first 64 bits, w[0] … w[63], are 0x2BB387F8EC5F707F.

### 5.10.2 Why this sequence

- The polynomial is primitive. Its degree 31 is prime and 2^31 − 1 is a prime, so irreducibility is enough. Irreducibility was checked: x^(2^31) ≡ x modulo the polynomial, and the polynomial has no root in GF(2). The sequence therefore has maximal period 2^31 − 1. The taps are those of PRBS-31, a common test pattern for serial links; the seed and output convention here are this specification's own.
- The period exceeds the largest data area, about 1.7 × 10^7 modules at 4108 × 4108, so the sequence never repeats inside a symbol. A shorter register, such as degree 16 with period 65 535, would repeat many times in a large symbol. Repeats line up with the column-pair walk and could build periodic structures.
- Two taps give one XOR per bit. Because w[k] depends only on bits at least 28 back, an implementation can compute 28 bits with one word-wide XOR.
- The seed is the most significant 31 bits of SHA-256 over the ASCII string `NMT Code whitening`. A published derivation shows the constant was not chosen for hidden properties. Starting from these seed bits, the longest run of equal bits is 8 in the first 1000 bits and 17 in the first 200 000 bits, as expected for random bits.
- A fixed sequence replaces the choice among masks that QR Code makes. The generator saves the mask search and the format word needs no mask field. The finders cannot be mistaken for whitened data because of their separators and shape. The cost is that a rare data pattern cannot be masked away. Such patterns are counted separately in the finder measurement.

## 5.11 Reader orientation (informative)

1. Find the finders, which are dark 5 × 5 squares with a light border, and classify each by its inner 3 × 3 against the four patterns under all eight transforms (5.3.2).
2. From any three identified finders, fix the rotation and mirroring. A reader MUST decode symbols at any rotation and SHOULD decode mirror images, for example from a front camera or through a transparent sheet. After undoing the mirror, all coordinates in this specification apply unchanged.
3. Read copy A relative to the TL finder and copy B relative to the BR finder (5.5), and decode the format word (chapter 2).
4. With W and H known, compute the function-module map and the placement order, and refine the sampling grid using the reference marks (5.6).

## 5.12 Worked example: 20 × 20

Values below were computed by a script that implements 5.3 to 5.10.

W = H = 20: n_W = n_H = 1, so there are no reference marks. D = 400 − 238 = 162, N = 20 and R = 2.

Module map. `#` dark function module, `o` light module inside a finder, `-` separator, `a` format copy A, `b` format copy B, `.` data module. Row numbers are y. The first character of each row is x = 0.

            1111111111
        01234567890123456789
     0  #####-aaaa....-#####
     1  #oo##-aaaa....-##oo#
     2  #oo##-aaaa....-#####
     3  #####-aaaa....-##oo#
     4  #####-aaaa....-#####
     5  ------aaa.....------
     6  aaaaaa..............
     7  aaaaaa..............
     8  aaaaaa..............
     9  aaaaaa..............
    10  ..............bbbbbb
    11  ..............bbbbbb
    12  ..............bbbbbb
    13  ..............bbbbbb
    14  ------.....bbb------
    15  #####-....bbbb-#####
    16  #####-....bbbb-#oo##
    17  #####-....bbbb-#o#o#
    18  #####-....bbbb-##oo#
    19  #####-....bbbb-#####

The first 32 placement positions and their module values, for a codeword stream that begins `4E 4D 54 20`:

| k | P[k] | Codeword | b[k] | w[k] | Module |
|---|---|---|---|---|---|
| 0 | (0, 10) | c[0] = 4E | 0 | 0 | 0 |
| 1 | (1, 10) | c[0] | 1 | 0 | 1 |
| 2 | (0, 11) | c[0] | 0 | 1 | 1 |
| 3 | (1, 11) | c[0] | 0 | 0 | 0 |
| 4 | (0, 12) | c[0] | 1 | 1 | 0 |
| 5 | (1, 12) | c[0] | 1 | 0 | 1 |
| 6 | (0, 13) | c[0] | 1 | 1 | 0 |
| 7 | (1, 13) | c[0] | 0 | 1 | 1 |
| 8 | (2, 13) | c[1] = 4D | 0 | 1 | 1 |
| 9 | (3, 13) | c[1] | 1 | 0 | 1 |
| 10 | (2, 12) | c[1] | 0 | 1 | 1 |
| 11 | (3, 12) | c[1] | 0 | 1 | 1 |
| 12 | (2, 11) | c[1] | 1 | 0 | 1 |
| 13 | (3, 11) | c[1] | 1 | 0 | 1 |
| 14 | (2, 10) | c[1] | 0 | 1 | 1 |
| 15 | (3, 10) | c[1] | 1 | 1 | 0 |
| 16 | (4, 10) | c[2] = 54 | 0 | 1 | 1 |
| 17 | (5, 10) | c[2] | 1 | 0 | 1 |
| 18 | (4, 11) | c[2] | 0 | 0 | 0 |
| 19 | (5, 11) | c[2] | 1 | 0 | 1 |
| 20 | (4, 12) | c[2] | 0 | 0 | 0 |
| 21 | (5, 12) | c[2] | 1 | 1 | 0 |
| 22 | (4, 13) | c[2] | 0 | 1 | 1 |
| 23 | (5, 13) | c[2] | 0 | 1 | 1 |
| 24 | (6, 19) | c[3] = 20 | 0 | 1 | 1 |
| 25 | (7, 19) | c[3] | 0 | 1 | 1 |
| 26 | (6, 18) | c[3] | 1 | 1 | 0 |
| 27 | (7, 18) | c[3] | 0 | 1 | 1 |
| 28 | (6, 17) | c[3] | 0 | 1 | 1 |
| 29 | (7, 17) | c[3] | 0 | 0 | 0 |
| 30 | (6, 16) | c[3] | 0 | 0 | 0 |
| 31 | (7, 16) | c[3] | 0 | 0 | 0 |

Column pairs 0, 1 and 2 hold 8 data modules each (rows 10 to 13). Pair 3 walks upward from y = 19. The last codeword c[19] ends at k = 159. The remainder modules are P[160] = (18, 6) and P[161] = (19, 6), with b = 0 and w[160] = 1, w[161] = 0, so they are dark and light.
