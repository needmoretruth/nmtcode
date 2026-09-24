# NMT Code — 7. Colour profiles

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder).

## 7.1 Scope and status

This chapter defines the colour profile selected by the colour-profile field of the format word (chapter 2):

| Value | Profile | Short description |
|---|---|---|
| 0 | black and white | no colour layer; chapters 2 to 6 only |
| 1 | luma + chroma cells, 4 colours | every data module keeps its base-layer luminance; each chroma cell adds 1 bit by a colour swing that keeps the luminance class |
| 2, 3 | reserved | chapter 2 (a reader rejects them) |

It defines the palettes, the colour cells and their order, the colour reference cells, the colour layer's error correction and bit mapping, what a black-and-white reader and a colour reader do, and printing.

Status:

- Colour is experimental in this version. A generator MUST NOT choose a colour profile unless the user asks for it. Every named profile except `color` (chapter 1, 1.5) uses profile 0.
- A colour profile becomes a candidate default only if measurement on real and simulated captures shows a net payload per symbol area of at least 1.3 times that of profile 0 at an equal decode-failure rate, under the same capture conditions (parameter `colour_default_gain_min` = 1.3).
- Colour applies to static symbols only. A transfer tile uses profile 0 (chapter 3, 3.3).

A pure colour grid without a luminance base, with 2 bits per cell, was also evaluated. By desk arithmetic it carries less than profile 1 (1.13 against 1.59 times profile 0, as in 7.11), so it is not in this version.

Binding rule: a symbol has a base layer that a reader decodes from luminance alone, and that base layer holds the format word, the container header and the base records (chapter 3, 3.5). The colour layer carries only the extension message.

## 7.2 Terms

| Term | Meaning |
|---|---|
| Luma | Y' = 0.299 R' + 0.587 G' + 0.114 B', computed on sRGB-encoded values R', G', B' in [0, 1] (the weights of ITU-R BT.601). A luminance-only reader is assumed to threshold this value or one that orders the palette the same way |
| Luminance class | dark (base-layer value 1) or light (base-layer value 0) |
| Class margin | the smallest luma of the light colours minus the largest luma of the dark colours |
| c | chroma cell side in modules: 1 when the format word's chroma cell size bit is 0, 2 when it is 1 |
| Data module | a module that is not a function module (chapter 5, 5.2 and 5.7) |
| Cell | a c × c block of modules aligned to multiples of c (7.5) |
| Colour cell | a cell used by the colour layer (7.5) |
| Reference cell | a colour cell with a fixed value, used for calibration (7.7) |
| Data cell | a colour cell that is not a reference cell |
| Colour message | the bytes the colour layer carries after error correction (7.8.1) |

## 7.3 Format word fields used by this chapter

| Field (chapter 2) | Width | Use here |
|---|---|---|
| Colour profile | 2 bits | 0 or 1 as in 7.1. Values 2 and 3 are handled by chapter 2 |
| Chroma cell size | 1 bit | 0 → c = 1 (1 × 1 module); 1 → c = 2 (2 × 2 modules) |
| Symbol class | 1 bit | a colour profile other than 0 requires class 0 (static) |

- With colour profile 0 the chroma cell size bit MUST be 0 (chapter 2).
- Desk arithmetic (7.11) says c = 2 cannot reach the gain of 7.1 (1.15 times profile 0). It is kept for captures where the camera, not the module size, limits resolution; measurement decides.
- A reader MUST treat a transfer tile with a colour profile other than 0 as malformed (chapter 3, 3.3).

## 7.4 Palettes

All colours are given as 8-bit sRGB (IEC 61966-2-1). CIELAB values are for the D65 white point, computed from the sRGB values; they describe the intended colour, not a tolerance.

### 7.4.1 Profile 1: luma + chroma

Each module shows the colour for its luminance class d (from the base layer) and the chroma bit b of its cell:

| d (base) | b (chroma) | Name | sRGB | Hex | Relative luminance Y | Luma Y' | L* | a* | b* |
|---|---|---|---|---|---|---|---|---|---|
| 1 (dark) | 0 | black | 0, 0, 0 | `#000000` | 0.0000 | 0.000 | 0.00 | 0.00 | 0.00 |
| 1 (dark) | 1 | blue | 0, 0, 255 | `#0000FF` | 0.0722 | 0.114 | 32.30 | 79.20 | −107.86 |
| 0 (light) | 0 | yellow | 255, 255, 0 | `#FFFF00` | 0.9278 | 0.886 | 97.14 | −21.56 | 94.48 |
| 0 (light) | 1 | white | 255, 255, 255 | `#FFFFFF` | 1.0000 | 1.000 | 100.00 | 0.00 | 0.00 |

Rule in one line: **chroma bit 1 selects the bluer of the two colours of the module's luminance class.**

Why this palette:

- Class margin. Dark colours have luma at most 0.114 and light colours at least 0.886, so the class margin is 0.772 in luma (0.856 in relative luminance, 64.8 in L*). The requirement is a margin of at least 0.5. Blue and yellow are the only saturated sRGB corners that keep the luma of their class that far from the threshold; red, magenta, green and cyan fall between 0.29 and 0.71.
- Direction. For both classes, bit 1 moves the colour toward blue and bit 0 toward yellow on the blue–yellow axis (black→blue adds blue; yellow→white adds blue). All modules of one cell therefore move in the same direction, whatever their luminance, so colour blur inside a 2 × 2 cell (camera chroma subsampling, demosaicing) does not cancel the signal. A palette with "bit 1 = chromatic in both classes" (blue for dark, yellow for light) would push the two classes in opposite directions and cancel inside mixed cells.
- Print. The four colours are exactly black ink, cyan and magenta overprinted, bare paper and yellow ink (7.10).
- Cost. Blue against black in dim light and yellow against white in strong light are the pairs that published colour-code tests found hardest. That is the price of keeping the luminance base; it is why the colour layer has its own strong error correction (7.8.2) and reference cells (7.7).

### 7.4.2 Other modules

- Function modules (finders, reference marks, format word; chapter 5) and the quiet zone are black `#000000` and white `#FFFFFF` only, in every profile.
- A data module outside every colour cell is black when dark and white when light.

### 7.4.3 Rendering

- A generator MUST output the palette values exactly: 8-bit sRGB, no dithering, no anti-aliasing at module edges, no colour-management conversion. A PNG output SHOULD carry the sRGB chunk.
- For printing, 7.10 replaces the sRGB values by inks.

## 7.5 Cells and their order

The cell grid divides the symbol into cells (i, j) for 0 ≤ i < W / c and 0 ≤ j < H / c. Cell (i, j) covers the modules with c·i ≤ x < c·i + c and c·j ≤ y < c·j + c. W and H are multiples of 4 (chapter 2), so the grid covers the symbol exactly.

A cell is a colour cell when all its c² modules are data modules.

Colour-cell order: colour cells sorted by j, then by i (row by row, left to right). Let N_cells be their number and e(0), e(1), …, e(N_cells − 1) the cells in this order.

Reference and data cells, with R_ref = 32 / c² (32 cells when c = 1, 8 cells when c = 2; parameter `reference_modules_per_copy` = 32):

| Cells | Role |
|---|---|
| e(0) … e(R_ref − 1) | reference copy A |
| e(N_cells − R_ref) … e(N_cells − 1) | reference copy B |
| all others, in colour-cell order | data cells d(0), d(1), …, d(N_data − 1), N_data = N_cells − 2·R_ref |

Copy A lies near the top of the symbol and copy B near the bottom, so one smudge rarely covers both.

## 7.6 The base layer

The base layer of profile 1 is exactly the base layer of profile 0: chapters 3 to 5 apply to all data modules, unchanged, including the whitening of chapter 5 (5.10). The colour of a module never changes its luminance class, so a luminance-only reader decodes the base layer of a profile 1 symbol with the procedure it uses for profile 0.

## 7.7 Reference cells

Reference cells carry fixed values instead of data. They are not whitened and consume no whitening bits (7.8.4).

In each copy, in colour-cell order, the first R_ref/2 cells have chroma bit 0 and the last R_ref/2 cells chroma bit 1. Values come in runs so that the inner samples of each run are free of colour bleeding from a neighbouring value.

Each reference module still carries its base-layer luminance. Once the base layer is decoded, the reader knows the class of every module (7.9.2 step 3), so each reference module has a known expected colour: black or yellow in the bit-0 run, blue or white in the bit-1 run. With whitened base data about half the modules of each run are dark; with 16 modules per run and copy, the chance that a run holds no module of one class is 2^−15.

Black and white anchors come from the function modules: the dark and light modules of each finder pattern (chapter 5, 5.3), its separator (5.4) and the quiet zone beside it.

## 7.8 Colour layer

### 7.8.1 Colour message

- When the base container has C = 1 (chapter 3, 3.2.2), the colour message is the extension message of chapter 3, 3.5: a container with C = 0, its own CRC-32C and the padding of chapter 3, 3.8, filled to the colour message capacity K_c. Its records are the extension records.
- When the base container of a colour symbol has C = 0, all records are in the base layer. The colour message is then K_c padding bytes: `EC 11 EC 11 …` starting with `EC` at byte 0. A reader MUST NOT present anything from the colour layer of such a symbol and need not decode it. A generator SHOULD NOT make such a symbol: it pays for colour and carries nothing in it.

### 7.8.2 Capacity and error correction

| Quantity | Value |
|---|---|
| Bits per data cell | 1 |
| Colour bits | N_data |
| Colour codewords N_c | floor(N_data / 8) |
| Fill bits | N_data − 8 · N_c (0 to 7), value 0 before whitening |
| Error-correction level lvl_c | `chroma_ecc_level` = 2, on the scale of chapter 4, 4.5 (parity at least 50%, close to QR level Q) |
| Block split | `split(N_c, lvl_c)` of chapter 4, 4.6 |
| Colour message capacity K_c | K returned by `split` |

- The colour layer is a Reed-Solomon code exactly as chapter 4 defines it: field and code (4.2), generator (4.3), systematic encoding (4.4), message assignment and codeword order (4.8), decoder requirements (4.9). Only the input differs: N_c and lvl_c instead of N and the format word's level.
- A symbol with a colour profile MUST have N_c ≥ 16 (parameter `colour_min_codewords`) and N_data ≥ 1. A reader MUST treat a smaller colour layer as malformed. A generator MUST NOT choose a colour profile for such a size.
- lvl_c is fixed at 2 in this version (parameter `chroma_ecc_level`, tunable in 0.x, fixed before 1.0). It is not in the format word or the container, so the reader knows it from the format version alone.

Why level 2. A byte of the colour stream spans 8 data cells, so a cell error rate p gives a byte error rate of about 8p. Published real-capture tests of coloured modules report per-module error rates of several percent, worst for the blue/black and yellow/white pairs that profile 1 uses. At p = 3% about 22% of bytes are wrong; level 2 corrects about 25%, level 1 about 15%. Level 3 would leave the colour layer too little room to reach the 1.3× gate of 7.1 (7.11). The base layer keeps its own level from the format word; the `color` profile default for the base layer is level 0.

### 7.8.3 Codeword stream

The colour message M_c[0 … K_c − 1] is split into blocks and encoded by chapter 4, 4.8.1 and 4.4. The colour codeword stream c_c[0 … N_c − 1] is taken from the blocks in the order of chapter 4, 4.8.2.

### 7.8.4 Bit stream and whitening

The colour bit stream s[0 … N_data − 1] is the codewords c_c[0], c_c[1], … each most significant bit first, followed by the fill bits (0).

Each bit is XORed with the whitening sequence u[0], u[1], …: t[k] = s[k] XOR u[k]. The sequence is produced by the generator of chapter 5 (5.10.1), unchanged — the recurrence u[k] = u[k−28] XOR u[k−31] — with its own seed:

| Parameter | Value |
|---|---|
| Seed | u[0], …, u[30] = the 31 bits of 0x644E9D0D, most significant first: `1100100010011101001110100001101` |
| Seed derivation | the most significant 31 bits of SHA-256 over the ASCII string `NMT Code chroma whitening` |
| First 64 bits | 0xC89D3A1B18E9D587 (bytes `C8 9D 3A 1B 18 E9 D5 87`) |

Register form, as in chapter 5: r = 0x644E9D0D, then for each k: u[k] = bit 30 of r; f = (bit 30 of r) XOR (bit 27 of r); r = ((r << 1) AND 0x7FFFFFFF) OR f.

Reference cells consume no bits of u; k counts only the bits of data cells. Whitening keeps the colours in equal shares on average, including over long padding, so a reader's colour statistics do not depend on the content. The seed differs from the base layer's, and the colour-cell order differs from the placement order, so chroma bits and luminance classes are uncorrelated. The period, 2^31 − 1, exceeds the colour bits of the largest symbol, so the sequence never repeats inside a symbol.

### 7.8.5 Mapping to cells

Data cell d(k) takes bit t[k].

### 7.8.6 Module colours

Every module of data cell d(k) with luminance class d gets the colour of (d, t[k]) from 7.4.1. Every module of a reference cell gets the colour of (d, fixed bit).

## 7.9 Reading

### 7.9.1 Reader that reads luminance only

1. Read the format word and the base layer from luminance, exactly as for profile 0.
2. Present the base records and state that the symbol holds further content in colour that this reader did not read (chapter 3, 3.5 rule 2). The base records are the same records that every reader presents, so a black-and-white reader and a colour reader never present different content; they differ only in whether the extension records appear.

This behaviour is provisional in 0.1.

### 7.9.2 Colour reader

A colour reader follows these steps. Steps 1 to 3 and 6 to 8 are normative. Steps 4 and 5 are RECOMMENDED; a reader MAY use another classifier, and every result is still subject to the checks of steps 7 and 8.

1. Decode the base layer as in 7.9.1 step 1, including the CRC-32C of chapter 3, 3.7. If it fails, present nothing (chapter 3, 3.9).
2. Find the colour cells, reference cells and data cells from W, H, c, the profile and the function-module map (7.5).
3. Re-encode the corrected base codewords and re-apply chapter 5's placement and whitening. This gives the exact luminance class of every data module, including modules the camera misread. Use these classes, not the thresholded pixels, from here on.
4. Calibration.
   - For each finder found, take the mean camera RGB of its dark modules (anchor K) and of its light modules and the nearby quiet zone (anchor Wt). Interpolate K and Wt bilinearly across the symbol from the corners found (with three finders, fit a plane).
   - Normalise each sampled module value per channel: v' = (v − K) / (Wt − K), clamped to [−0.25, 1.25]. This corrects white balance, exposure and slow lighting gradients.
   - Measure the palette in the normalised space from the reference cells of both copies: P[d][b] = mean v' of the reference modules with class d in the run of chroma bit b.

     A reader MAY interpolate between copy A and copy B by position. If a mean has fewer than 4 modules, use the nominal value (the sRGB value of 7.4 divided by 255).
5. Classification.
   - Each module m of a data cell, with class d:

         t_m = ((v'_m − P[d][0]) · (P[d][1] − P[d][0])) / |P[d][1] − P[d][0]|²
         e_m = clamp(t_m − 0.5, −1, 1)

     Cell score S = sum of e_m over the cell's modules. Chroma bit = 1 when S > 0, else 0.
   - Erasures: a reader SHOULD mark a colour codeword as an erasure (chapter 4, 4.9) when a cell in it has low confidence (|S| < 0.15 · c²), or when a module in it had its base value corrected in step 3. These thresholds are reader choices.
6. Remove the whitening (7.8.4), rebuild the colour codewords, and decode them with `split(N_c, lvl_c)` and chapter 4, 4.9.
7. Check the colour message as chapter 3 requires: its CRC-32C (3.7) and its container rules (3.9).
8. Apply chapter 3, 3.5:
   - any failure in step 6 or 7: present the base records and state that the colour content could not be read (chapter 3, 3.5 rule 2);
   - otherwise compute the digest over the base and extension records. On a mismatch reject the whole symbol and present nothing; on a match present the base records, then the extension records.

A reader reading video SHOULD try further frames before settling on a colour failure (chapter 3, 3.5).

## 7.10 Print

Profile 1 has an optional 4-ink form. A generator that writes a CMYK output (for example PDF with DeviceCMYK) for a profile 1 symbol SHOULD use these inks instead of converting the sRGB values through a colour profile:

| Colour | Inks |
|---|---|
| black | K 100% only (no rich black) |
| blue | C 100% + M 100% |
| white | no ink (paper) |
| yellow | Y 100% |

- Luminance classes survive print. Cyan plus magenta absorbs the red and green parts of the spectrum, so the overprint prints dark; yellow ink absorbs only blue, so it prints light. A luminance-only reader therefore reads the base layer of a printed profile 1 symbol as it reads a black-and-white one.
- A print setup is suitable for profile 1 when, measured on the printed result (D50, 2° observer): K and C+M have L* ≤ 40, paper and Y have L* ≥ 80, and within each class the b* values differ by at least 30 (parameters `print_dark_L_max` = 40, `print_light_L_min` = 80, `print_class_db_min` = 30; tunable in 0.x, fixed before 1.0). The person printing SHOULD check this on a test print; a generator cannot measure it.
- Cell size: in print a generator SHOULD use c = 2. Every module is a whole number of printer dots, as for the `print` profile (chapter 1, 1.5).
- Known risks: some printers add faint yellow tracking dots, which disturb yellow and white cells; some printer drivers merge nearby hues. The reference cells correct colour shifts, but they cannot restore two colours that the printer made equal.

## 7.11 Capacity arithmetic (informative)

Raw bits per module against profile 0, at the same module size, before any measurement:

| Case | Base layer (bits per module × data share) | Colour layer | Ratio to profile 0 |
|---|---|---|---|
| profile 0, base level 0 (parity about 15%) | 1 × 0.85 | — | 1.00 |
| profile 1, c = 2, lvl_c = 2 | 1 × 0.85 | 1/4 × 0.50 | 1.15 |
| profile 1, c = 1, lvl_c = 2 | 1 × 0.85 | 1 × 0.50 | 1.59 |
| pure colour grid, not in this version: 1/4 of rows black and white, 2 bits per colour module | 1/4 × 0.85 | 3/4 × 2 × 0.50 | 1.13 |

- Reference cells, the digest field (33 bytes) and cells lost next to function patterns come on top and matter most in small symbols.
- With c = 2 profile 1 cannot reach the 1.3× gate at these levels. Camera chroma subsampling halves colour resolution, so c = 1 needs about 4 or more camera pixels per module (parameter `chroma_k_min_c1` = 4); below that a generator SHOULD use c = 2.
- The pure colour grid carries less than profile 1 with c = 1 because profile 1 gets each module's luminance class free from the corrected base layer.

## 7.12 Parameters

All are tunable in 0.x and fixed before 1.0.

| Name | Value | Section |
|---|---|---|
| `colour_default_gain_min` | 1.3 | 7.1 |
| `reference_modules_per_copy` | 32 (R_ref = 32 / c² cells) | 7.5 |
| `colour_min_codewords` | N_c ≥ 16 | 7.8.2 |
| `chroma_ecc_level` | 2 | 7.8.2 |
| `chroma_whitening_seed` | 0x644E9D0D, generator of chapter 5 (5.10.1) | 7.8.4 |
| `chroma_k_min_c1` | 4 camera pixels per module | 7.11 |
| `print_dark_L_max`, `print_light_L_min`, `print_class_db_min` | 40, 80, 30 | 7.10 |
| `profile1_palette` | the four colours of 7.4.1 | 7.4.1 |

## 7.13 Worked example

Values computed by a script. The positions of the cells in the symbol follow from chapter 5's function-module map; the example uses their indices in colour-cell order. Base-layer module classes marked "assumed" stand for the whitened base data at those positions (taken from the bits of SHA-256 of the ASCII string `NMT Code colour example`).

Symbol: profile 1, c = 2 (chroma cell size bit 1), so R_ref = 8 cells per reference copy. Modules of a cell are listed top-left, top-right, bottom-left, bottom-right; 1 = dark. Colours: K black, B blue, Y yellow, W white.

### 7.13.1 Reference copy A: cells e(0) … e(7)

| Cell | Fixed chroma bit | Module classes (assumed) | Module colours |
|---|---|---|---|
| e(0) | 0 | 1 0 1 0 | K Y K Y |
| e(1) | 0 | 0 1 1 1 | Y K K K |
| e(2) | 0 | 1 0 0 0 | K Y Y Y |
| e(3) | 0 | 0 1 0 1 | Y K Y K |
| e(4) | 1 | 0 0 1 0 | W W B W |
| e(5) | 1 | 0 0 1 0 | W W B W |
| e(6) | 1 | 1 1 0 0 | B B W W |
| e(7) | 1 | 0 1 1 1 | W B B B |

The reader gets 8 samples of black, 8 of yellow, 8 of blue and 8 of white from this copy.

### 7.13.2 Colour message and the first 16 data cells

The colour message is the extension message of chapter 3, 3.10 e (22 bytes, then padding):

```
00 10 01 EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 A2 EC CD A1
```

With a single Reed-Solomon block (N_c ≤ 255) the codeword stream starts with the message bytes, so c_c[0] = `00` and c_c[1] = `10`. Whitening bits u[0 … 15] = `11001000 10011101` (`C8 9D`).

| Data cell | s[k] | u[k] | t[k] | Module classes (assumed) | Module colours |
|---|---|---|---|---|---|
| d(0) | 0 | 1 | 1 | 0 1 0 1 | W B W B |
| d(1) | 0 | 1 | 1 | 0 1 1 0 | W B B W |
| d(2) | 0 | 0 | 0 | 0 0 1 0 | Y Y K Y |
| d(3) | 0 | 0 | 0 | 1 1 0 1 | K K Y K |
| d(4) | 0 | 1 | 1 | 0 1 0 0 | W B W W |
| d(5) | 0 | 0 | 0 | 1 0 1 0 | K Y K Y |
| d(6) | 0 | 0 | 0 | 0 0 1 1 | Y Y K K |
| d(7) | 0 | 0 | 0 | 1 1 1 1 | K K K K |
| d(8) | 0 | 1 | 1 | 0 1 0 1 | W B W B |
| d(9) | 0 | 0 | 0 | 1 1 0 1 | K K Y K |
| d(10) | 0 | 0 | 0 | 0 1 1 0 | Y K K Y |
| d(11) | 1 | 1 | 0 | 1 0 1 0 | K Y K Y |
| d(12) | 0 | 1 | 1 | 1 0 0 1 | B W W B |
| d(13) | 0 | 1 | 1 | 1 1 1 1 | B B B B |
| d(14) | 0 | 0 | 0 | 1 1 1 0 | K K K Y |
| d(15) | 0 | 1 | 1 | 0 0 0 1 | W W W B |

Cells d(0) … d(7) carry c_c[0]; cells d(8) … d(15) carry c_c[1].


### 7.13.3 Classification of two modules (step 5 of 7.9.2)

Anchors: K = (18, 20, 30), Wt = (230, 225, 210). Reference means, already normalised: P[dark][0] = (0.02, 0.03, 0.05), P[dark][1] = (0.10, 0.12, 0.80), P[light][0] = (0.97, 0.95, 0.12), P[light][1] = (0.98, 0.97, 0.96).

| Module | Class | Camera RGB | Normalised v' | t_m | e_m |
|---|---|---|---|---|---|
| 1 | dark | (40, 45, 190) | (0.1038, 0.1220, 0.8889) | 1.1164 | 0.6164 |
| 2 | light | (215, 212, 120) | (0.9292, 0.9366, 0.5000) | 0.4511 | −0.0489 |

Module 1 is clearly blue (bit 1). Module 2 lies between yellow and white. If both are in one cell, they add 0.6164 − 0.0489 = 0.5675 to that cell's score, which is positive, so they vote for bit 1.
