# NMT Code — 7. Colour profiles

© 2026 needmoretruth. Licensed under CC BY 4.0 (see LICENSE).

Specification version 0.2 (draft).

## 7.1 Scope and status

This chapter defines the colour profile selected by the colour-profile field of the format word (chapter 2):

| Value | Profile | Short description |
|---|---|---|
| 0 | black and white | no colour layer; chapters 2 to 6 only |
| 1 | luma + chroma cells, 4 colours | every data module keeps its base-layer luminance; each chroma cell adds 1 bit by a colour swing that keeps the luminance class |
| 2, 3 | reserved | chapter 2 (a reader rejects them) |

It defines the palettes, the colour cells and their order, the colour reference cells, the colour layer's error correction and bit mapping, what a black-and-white reader and a colour reader do, the module colour check that every reader that samples colour applies, and printing.

Status:

- Colour is experimental in this version. A generator MUST NOT choose a colour profile unless the user asks for it. Every named profile except `color` (chapter 1, 1.5) uses profile 0.
- A colour profile becomes a candidate default only if measurement on real and simulated captures shows a net payload per symbol area of at least 1.3 times that of profile 0 at an equal decode-failure rate, under the same capture conditions (parameter `colour_default_gain_min` = 1.3).
- Colour applies to static symbols only. A transfer tile uses profile 0 (chapter 3, 3.3).

A pure colour grid without a luminance base, with 2 bits per cell, was also evaluated. By desk arithmetic it carries less than profile 1 (1.13 against 1.59 times profile 0, as in 7.11), so it is not in this version.

Binding rule: a symbol has a base layer that a reader decodes from luminance alone, and that base layer holds the format word, the container header and the base records (chapter 3, 3.5). The colour layer carries only the extension message.

## 7.2 Terms

| Term | Meaning |
|---|---|
| Luma | Y' = 0.299 R' + 0.587 G' + 0.114 B', computed on sRGB-encoded values R', G', B' in [0, 1] (the weights of ITU-R BT.601). Chapter 1 (1.4) makes it the luminance by which every reader classifies the modules of the base layer |
| Luminance class | dark (base-layer value 1) or light (base-layer value 0) |
| Class margin | the smallest luma of the light colours minus the largest luma of the dark colours |
| c | chroma cell side in modules: 1 when the format word's chroma cell size bit is 0, 2 when it is 1 |
| Data module | a module that is not a function module (chapter 5, 5.2 and 5.7) |
| Cell | a c × c block of modules aligned to multiples of c (7.5) |
| Colour cell | a cell used by the colour layer (7.5) |
| Reference cell | a colour cell with a fixed value, used for calibration (7.5, 7.7) |
| Data cell | a colour cell that is not a reference cell |
| Colour message | the bytes the colour layer carries after error correction (7.8.1) |

## 7.3 Format word fields used by this chapter

| Field (chapter 2) | Width | Use here |
|---|---|---|
| Colour profile | 2 bits | 0 or 1 as in 7.1. Values 2 and 3 are handled by chapter 2 |
| Chroma cell size | 1 bit | 0 → c = 1 (1 × 1 module); 1 → c = 2 (2 × 2 modules) |
| Symbol class | 1 bit | a colour profile other than 0 requires class 0 (static) |

- With colour profile 0 the chroma cell size bit MUST be 0 (chapter 2).
- Desk arithmetic (7.11) says c = 2 cannot reach the gain of 7.1 (1.15 times profile 0). It is kept for captures where the camera, not the module size, limits resolution, and for print (7.10); measurement decides.
- Two combinations are invalid in the format word itself, so every reader, a black-and-white reader included, treats the copy that carries them as not decoded (chapter 2, 2.3 and 2.7 step 3): a transfer tile with a colour profile other than 0 (chapter 3, 3.3), and colour profile 1 at a size whose colour layer has fewer than 16 codewords (7.8.2).

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
- Known risks of the blue–yellow axis. The colour signal lies almost entirely in the camera's blue channel, which a Bayer sensor samples least and which warm light (2700 to 3000 K) weakens most. The daylight locus runs roughly along the same axis, so mixed lighting, such as a window and a lamp, makes colour gradients along it. On screens, night modes and blue-light filters cut the blue emission and pull white toward yellow. The palette is still the only one that keeps a luma margin of 0.5 (above), so measurement and calibration (7.9.2) answer these risks, not another palette.

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

Reference and data cells. R_ref = 32 / c² cells (32 when c = 1, 8 when c = 2; parameter `reference_modules_per_copy` = 32). The reference cells are two copies of R_ref cells. Each copy is made of two groups of R_ref / 2 cells at opposite corners of the symbol:

| Group | Anchor (x, y) | Copy |
|---|---|---|
| TL | (10, 10) | A |
| TR | (W − 10, 10) | B |
| BL | (10, H − 10) | B |
| BR | (W − 10, H − 10) | A |

- An anchor is a point in module coordinates, a corner shared by four modules. The centre of cell (i, j) is (c·i + c/2, c·j + c/2).
- The groups are chosen in the order TL, TR, BL, BR. Each group takes the R_ref / 2 colour cells, not taken by an earlier group, whose centres are nearest to its anchor by Euclidean distance; on equal distances the cell earlier in colour-cell order comes first. In integers, the squared distance of cell (i, j) from anchor (a_x, a_y), doubled on each axis, is (2c·i + c − 2a_x)² + (2c·j + c − 2a_y)².
- Within a group the cells keep colour-cell order (7.7 uses it).
- All other colour cells are the data cells d(0), d(1), …, d(N_data − 1), in colour-cell order, N_data = N_cells − 2 · R_ref.

Why there: the anchors lie 10 modules in from the edges, beyond the finders and the format copies. The outermost rows touch the quiet zone, and blur, flare and camera chroma subsampling bias their colour toward white. Four groups near the four finders also show colour shading across the symbol, which is often radial (lens shading, the viewing angle of a screen), not only vertical. Each copy spans two opposite corners, so one smudge rarely covers both copies. In a small symbol the anchors meet near the middle and the groups lie in rings around them; at 20 × 24, the smallest colour size, the anchors are (10, 10) twice and (10, 14) twice.

## 7.6 The base layer

The base layer of profile 1 is exactly the base layer of profile 0: chapters 3 to 5 apply to all data modules, unchanged, including the whitening of chapter 5 (5.10). The colour of a module never changes its luminance class, so a luminance-only reader decodes the base layer of a profile 1 symbol with the procedure it uses for profile 0.

## 7.7 Reference cells

Reference cells carry fixed values instead of data. They are not whitened and consume no whitening bits (7.8.4).

In each group, in colour-cell order, the first R_ref / 4 cells have chroma bit 0 and the last R_ref / 4 cells chroma bit 1: 8 and 8 cells when c = 1, 2 and 2 when c = 2. The cells of a group lie around its anchor, so its upper half has bit 0 and its lower half bit 1. Each value thus forms a run whose inner samples are free of colour bleeding from the other value.

Each reference module still carries its base-layer luminance. Once the base layer is decoded, the reader knows the class of every module (7.9.2 step 3), so each reference module has a known expected colour: black or yellow in the bit-0 run, blue or white in the bit-1 run. Each palette colour is measured over the four groups: 32 modules per chroma bit. With whitened base data about half of them are of each class; the chance that the 32 modules of one bit hold no module of one of the two classes is 2^−31.

Black and white anchors come from the function modules: the dark modules of each finder pattern (chapter 5, 5.3), and the quiet zone and separator beside it (5.2, 5.4).

## 7.8 Colour layer

### 7.8.1 Colour message

- When the base container has X = 1 (chapter 3, 3.2.2), the colour message is the extension message of chapter 3, 3.5: a container with X = 0, the same format echo byte as the base container, its own CRC-32C and the padding of chapter 3, 3.8, filled to the colour message capacity K_c. Its records are the extension records.
- When the base container of a colour symbol has X = 0, all records are in the base layer. The colour message is then K_c padding bytes: `EC 11 EC 11 …` starting with `EC` at byte 0. A reader MUST NOT present anything from the colour layer of such a symbol and need not decode it. A generator SHOULD NOT make such a symbol: it pays for colour and carries nothing in it.

### 7.8.2 Capacity and error correction

| Quantity | Value |
|---|---|
| Bits per data cell | 1 |
| Colour bits | N_data |
| Colour codewords N_c | floor(N_data / 8) |
| Fill bits | N_data − 8 · N_c (0 to 7), value 0 before whitening; a reader MUST ignore them |
| Error-correction level lvl_c | 2 for colour profile 1, on the scale of chapter 4, 4.5 (parity at least 50%, close to QR level Q) |
| Block split | `split(N_c, lvl_c)` of chapter 4, 4.6 |
| Colour message capacity K_c | K returned by `split` |

- The colour layer is a Reed-Solomon code exactly as chapter 4 defines it: field and code (4.2), generator (4.3), systematic encoding (4.4), message assignment and codeword order (4.8), decoder requirements (4.9). Only the input differs: N_c and lvl_c instead of N and the format word's level.
- A symbol with colour profile 1 MUST have N_c ≥ 16 (parameter `colour_min_codewords`). Every reader computes N_c from W, H and the chroma cell size by 7.5 and this table. Colour profile 1 at a smaller size is an invalid combination of the format word: the copy that carries it is not decoded (chapter 2, 2.3 and 2.7 step 3), so a black-and-white reader rejects such a symbol as a colour reader does. A generator MUST NOT choose a colour profile for such a size.
- lvl_c is 2 for colour profile 1 and is not a tunable parameter. It is not in the format word or the container: a reader knows it from the colour profile, and a different level would take a new colour-profile value (chapter 9, 9.5).

Smallest valid sizes of colour profile 1, where N_c ≥ 16:

| Chroma cell | Smallest area | Smallest square | Too small for comparison |
|---|---|---|---|
| 1 × 1 (c = 1) | 20 × 24 and 24 × 20, N_c = 22 | 24 × 24, N_c = 34 | 20 × 20, N_c = 12 |
| 2 × 2 (c = 2) | 24 × 36 and 36 × 24, N_c = 17 | 32 × 32, N_c = 22 | 28 × 28, N_c = 15; 24 × 32, N_c = 14 |

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
2. Present the base records. When the base container has X = 1, state that the symbol holds further content in colour that this reader did not read (chapter 3, 3.5 rule 2).

Every reader presents the same base records. A colour reader may present the extension records as well, shown apart from the base records and labelled as colour content that other readers may not show (chapter 3, 3.4.4 rule 5). No reader presents a record that another reader presents differently.

This behaviour is provisional in 0.2.

### 7.9.2 Colour reader

A colour reader follows these steps, in the order of chapter 3 (3.5). Steps 1 to 3 and 6 to 10 are normative. Steps 4 and 5 are RECOMMENDED; a reader MAY use another classifier, and every result is still subject to the checks of steps 6 to 9.

1. **Base layer.** Decode the base layer as in 7.9.1 step 1, with every check of chapter 3 (3.9) on the base container and the module colour check of 7.9.3 right after its CRC-32C. If one fails, present nothing (chapter 3, 3.5 rule 1).
2. **Cells.** Find the colour cells, the reference groups and the data cells from W, H, c, the profile and the function-module map (7.5).
3. **Classes.** Re-encode the corrected base codewords and re-apply chapter 5's placement and whitening. This gives the exact luminance class of every data module, including modules the camera misread. Use these classes, not the thresholded pixels, from here on.
4. **Calibration.**
   - For each finder found, take the mean camera RGB of its dark modules (anchor K) and of the quiet zone and separator beside it (anchor Wt). The finder's own light modules are one- and two-module features that blur darkens, so they are not used for Wt. Interpolate K and Wt bilinearly across the symbol from the corners found (with three finders, fit a plane).
   - Normalise each sampled module value per channel: v' = (v − K) / (Wt − K), clamped to [−0.25, 1.25]. This corrects white balance, exposure and slow lighting gradients.
   - Measure the palette in the normalised space from the four reference groups: P[d][b] = mean v' of the reference modules with class d and chroma bit b. A reader MAY interpolate P between the groups by position. If a mean has fewer than 4 modules, use the nominal value (the sRGB value of 7.4 divided by 255).
   - Local calibration (informative). Screen dimming and refresh under a rolling shutter make horizontal bands, and lens shading is radial; neither is bilinear across four corners. After step 3 a reader knows the class of every module, and whitening makes each class about half chroma bit 0 and half chroma bit 1 in any window. A reader MAY estimate K, Wt and the palette in windows of about 8 × 8 modules from the means of the dark-class and the light-class data modules of the window, and use the reference groups to tell which side of each class is bit 1.
5. **Classification.**
   - Each module m of a data cell, with class d:

         t_m = ((v'_m − P[d][0]) · (P[d][1] − P[d][0])) / |P[d][1] − P[d][0]|²
         e_m = clamp(t_m − 0.5, −1, 1)

     Cell score S = sum of e_m over the cell's modules. Chroma bit = 1 when S > 0, else 0.
   - Erasures: a reader SHOULD mark a colour codeword as an erasure (chapter 4, 4.9) when a cell in it has low confidence (|S| < 0.15 · c²), or when a module in it had its base value corrected in step 3. These thresholds are reader choices.
6. **Error correction.** Remove the whitening (7.8.4), rebuild the colour codewords, and decode them with `split(N_c, lvl_c)` and chapter 4, 4.9. A failure: present the base records and state that the colour content could not be read (chapter 3, 3.5 rule 2).
7. **Extension container.** Check the colour message as chapter 3 requires: its CRC-32C (3.7), its format echo byte (3.2.2) and every other check of 3.9. A failure: as in step 6.
8. **Digest.** Compute the digest of chapter 3 (3.5) over d, the record counts and the records of both messages. On a mismatch, reject the whole symbol and present nothing, base records included (`E_DIGEST_MISMATCH`).
9. **Ordering.** If the extension message holds an action record, or the last base record is an attribute record, the extension is invalid (chapter 3, 3.5 rule 4): present the base records only, as in step 6.
10. **Present.** Present the base records, then the extension records apart from them (chapter 3, 3.5 rule 5).

A reader reading video SHOULD try further frames before settling on a colour failure (chapter 3, 3.5).

### 7.9.3 Module colour check

A maker is not bound to the palette. A red module reads dark by luma and light by the red channel; a cyan module the other way round. Modules in such colours can make one symbol decode to different content under different ways of turning colour into luminance, in profile 0 as well as in profile 1. Chapter 1 (1.4) fixes the reader's luminance; this check rejects symbols that depend on the difference.

A reader that samples colour MUST apply the check to every symbol, profile 0 included, right after the CRC-32C of the base container has passed and before the checks that follow it (chapter 3, 3.9). A reader whose capture has no colour, for example one from a monochrome camera, cannot apply it.

1. Normalise the colour sample v of every data module per channel: v' = (v − K) / (Wt − K), with the anchors of 7.9.2 step 4: K from the dark modules of the finders, Wt from the quiet zone and separators beside them, interpolated across the symbol.
2. A data module is off-palette when the corner of the RGB unit cube nearest to its v' is not a colour of the symbol's palette. The eight corners are black, blue, green, cyan, red, magenta, yellow and white, and the distance is Euclidean. The palette of profile 0 is black and white; that of profile 1 is black, blue, yellow and white (7.4.1). A module at equal distance from two or more corners counts as off-palette.
3. If more than `colour_mismatch_share_max` = 1% of the data modules are off-palette, the reader MUST reject the symbol with `E_COLOUR_AMBIGUOUS` (Malformed, chapter 9, 9.8) and present nothing.
4. A reader SHOULD treat an off-palette module as unreliable when it decodes the base layer, by marking the codeword that holds it as an erasure (chapter 4, 4.9; chapter 5, 5.9). A symbol that depends on only a few off-palette modules then fails the same way in every reader that marks them.

The class of a module does not enter the check: a module the camera read in the wrong class is an ordinary error that the base layer corrects. The value 1% is provisional; measurement of honest captures, with colour casts and chromatic aberration at module edges, decides it before the first public release.

## 7.10 Print

Profile 1 has an optional 4-ink form. A generator that writes a CMYK output (for example PDF with DeviceCMYK) for a profile 1 symbol SHOULD use these inks instead of converting the sRGB values through a colour profile:

| Colour | Inks |
|---|---|
| black | K 100% only (no rich black) |
| blue | C 100% + M 100% |
| white | no ink (paper) |
| yellow | Y 100% |

- Luminance classes survive print. Cyan plus magenta absorbs the red and green parts of the spectrum, so the overprint prints dark; yellow ink absorbs only blue, so it prints light. A luminance-only reader therefore reads the base layer of a printed profile 1 symbol as it reads a black-and-white one.
- A print setup is suitable for profile 1 when, measured on the printed result (D50, 2° observer): K and C+M have L* ≤ 40, paper and Y have L* ≥ 80, and within each class the b* values differ by at least 30 (parameters `print_dark_L_max` = 40, `print_light_L_min` = 80, `print_class_db_min` = 30, generator values of chapter 9, 9.5). The person printing SHOULD check this on a test print; a generator cannot measure it.
- Cell size: a generator that makes a profile 1 symbol for print MUST use c = 2 (chroma cell size bit 1). Where the cyan and magenta plates do not overlap exactly, a blue module gets a cyan fringe on one side and a magenta fringe on the other. With 2 × 2 cells each chroma bit is decided from four modules, so a fringe on one edge of one module weighs less. Every module is a whole number of printer dots, as for the `print` profile (chapter 1, 1.5).
- Known risks:
  - Some printers add faint yellow tracking dots, which disturb yellow and white cells; some printer drivers merge nearby hues. The reference cells correct colour shifts, but they cannot restore two colours that the printer made equal.
  - Plate misregistration. Cyan or magenta ink alone sits near mid-lightness (roughly L* 48 to 55), close to the luminance threshold, so the fringes of misregistered blue modules also disturb the base layer at their edges. On a 0.4 mm module, 0.05 to 0.1 mm of misregistration is 12 to 25% of a module.
  - C + M is 200% ink coverage and spreads more on uncoated paper than black ink alone (print growth, chapter 1, 1.5).

## 7.11 Capacity arithmetic (informative)

Raw bits per module against profile 0, at the same module size, before any measurement:

| Case | Base layer (bits per module × data share) | Colour layer | Ratio to profile 0 |
|---|---|---|---|
| profile 0, base level 0 (parity about 15%) | 1 × 0.85 | — | 1.00 |
| profile 1, c = 2, lvl_c = 2 | 1 × 0.85 | 1/4 × 0.50 | 1.15 |
| profile 1, c = 1, lvl_c = 2 | 1 × 0.85 | 1 × 0.50 | 1.59 |
| pure colour grid, not in this version: 1/4 of rows black and white, 2 bits per colour module | 1/4 × 0.85 | 3/4 × 2 × 0.50 | 1.13 |

- Reference cells, the digest field (33 bytes) and cells lost next to function patterns come on top and matter most in small symbols.
- With c = 2 profile 1 cannot reach the 1.3× gate at these levels. Camera chroma subsampling halves colour resolution, so c = 1 needs about 4 or more camera pixels per module (parameter `chroma_k_min_c1` = 4, provisional); below that a generator SHOULD use c = 2.
- The value 4 accounts for 4:2:0 chroma subsampling only. Phone camera pipelines also smooth chroma more than luma, in low light often over more than 2 × 2 pixels, and lateral chromatic aberration shifts blue against green by up to about a pixel toward the image corners (assumed typical behaviour, not measured). A browser reader cannot turn chroma noise reduction off. Measurement of c = 1 and c = 2 against k, light level and capture path decides the value.
- The pure colour grid carries less than profile 1 with c = 1 because profile 1 gets each module's luminance class free from the corrected base layer.

## 7.12 Parameters

Chapter 9 (9.5) lists these values. A decoding value may change only until the first public release of this specification; a generator or policy value may change in any later version.

| Name | Value | Section | Effect |
|---|---|---|---|
| `colour_default_gain_min` | 1.3 | 7.1 | policy |
| `profile1_palette` | the four colours of 7.4.1 | 7.4.1 | decoding |
| `reference_modules_per_copy` | 32 (R_ref = 32 / c² cells) | 7.5 | decoding |
| `colour_min_codewords` | N_c ≥ 16 | 7.8.2 | decoding |
| `chroma_whitening_seed` | 0x644E9D0D, generator of chapter 5 (5.10.1) | 7.8.4 | decoding |
| `colour_mismatch_share_max` | 1% of the data modules | 7.9.3 | decoding |
| `chroma_k_min_c1` | 4 camera pixels per module, provisional | 7.11 | generator |
| `print_dark_L_max`, `print_light_L_min`, `print_class_db_min` | 40, 80, 30 | 7.10 | generator |

The colour layer's error-correction level is not a parameter: it is 2 for colour profile 1 (7.8.2).

## 7.13 Worked example

Values computed by a script that implements chapters 2 to 5 and 7.5 to 7.8. The symbol is example e of chapter 3 (3.10): 32 × 32, static, level 0, colour profile 1 with 1 × 1 chroma cells (c = 1). Every module class below is the real base-layer value of this symbol.

### 7.13.1 Symbol and layers

| Quantity | Value |
|---|---|
| Format data d (chapter 2, 2.2) | 0x0020082 |
| Copy A F_A, copy B F_B (2.5) | 0x51E36D589747, 0x3FDA30A827CB |
| Base layer (chapters 4, 5) | N = 98, one block, P = 16, K = 82: the 49-byte base container of 3.10 e and 33 padding bytes |
| Colour cells N_cells | 786: with c = 1 every data module is a colour cell |
| Reference cells | 4 groups of 16 cells, R_ref = 32 per copy |
| Data cells N_data | 722 |
| Colour codewords N_c | 90, and 2 fill bits |
| Colour block split | `split(90, 2)`: one block, P = 46, K_c = 44 |

With c = 2 the same size has N_c = 22 and K_c = 10, less than the 23-byte extension container of 3.10 e.

### 7.13.2 Reference cells

Each group is a 4 × 4 block of modules here, around its anchor (10, 10), (22, 10), (10, 22) or (22, 22). In each group the upper two rows have chroma bit 0 and the lower two chroma bit 1. Colours: K black, B blue, Y yellow, W white.

```
TL: x 8 to 11, y 8 to 11      TR: x 20 to 23, y 8 to 11
    Y Y K K                       Y K Y K
    K K Y K                       K K Y Y
    W W B W                       B W B W
    W B W W                       W B B W

BL: x 8 to 11, y 20 to 23     BR: x 20 to 23, y 20 to 23
    K Y Y K                       Y K K K
    K Y K Y                       K Y K Y
    B B B W                       W B W B
    B B W B                       W W B B
```

Copy A is the groups TL and BR, copy B the groups TR and BL. Over the four groups the reader gets 18 samples of black, 14 of yellow, 16 of blue and 16 of white.

### 7.13.3 Colour message and the first 16 data cells

The colour message is the extension container of 3.10 e (23 bytes), padded to K_c = 44 bytes (chapter 3, 3.8):

```
00 10 10 01 EC 95 88 EB 85 95 ED 95 98 EC 84 B8 EC 9A 94 28 CA 4D 68 EC
11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC 11 EC
```

Its 46 parity bytes (chapter 4, 4.4):

```
3E F4 A3 BB 11 95 9C 99 A7 DE B7 27 E0 79 F4 AA DA BE BC 44 6D F9 BA 7A
EB 3D 49 2F EA 0C 59 20 18 15 C1 DC DC 79 D0 4F 01 20 28 F1 F9 43
```

With a single block the codeword stream is the message followed by the parity, so c_c[0] = `00` and c_c[1] = `10`. Whitening bits u[0 … 15] = `11001000 10011101` (`C8 9D`). The first data cells, in colour-cell order, are the data modules of row 0 between format copy A and the TR separator.

| Data cell | Module | s[k] | u[k] | t[k] | Class | Colour |
|---|---|---|---|---|---|---|
| d(0) | (10, 0) | 0 | 1 | 1 | dark | B |
| d(1) | (11, 0) | 0 | 1 | 1 | light | W |
| d(2) | (12, 0) | 0 | 0 | 0 | light | Y |
| d(3) | (13, 0) | 0 | 0 | 0 | dark | K |
| d(4) | (14, 0) | 0 | 1 | 1 | dark | B |
| d(5) | (15, 0) | 0 | 0 | 0 | dark | K |
| d(6) | (16, 0) | 0 | 0 | 0 | dark | K |
| d(7) | (17, 0) | 0 | 0 | 0 | dark | K |
| d(8) | (18, 0) | 0 | 1 | 1 | dark | B |
| d(9) | (19, 0) | 0 | 0 | 0 | light | Y |
| d(10) | (20, 0) | 0 | 0 | 0 | dark | K |
| d(11) | (21, 0) | 1 | 1 | 0 | light | Y |
| d(12) | (22, 0) | 0 | 1 | 1 | light | W |
| d(13) | (23, 0) | 0 | 1 | 1 | dark | B |
| d(14) | (24, 0) | 0 | 0 | 0 | light | Y |
| d(15) | (25, 0) | 0 | 1 | 1 | light | W |

Cells d(0) … d(7) carry c_c[0]; cells d(8) … d(15) carry c_c[1].

### 7.13.4 Classification of two modules (step 5 of 7.9.2)

Anchors: K = (18, 20, 30), Wt = (230, 225, 210). Reference means, already normalised: P[dark][0] = (0.02, 0.03, 0.05), P[dark][1] = (0.10, 0.12, 0.80), P[light][0] = (0.97, 0.95, 0.12), P[light][1] = (0.98, 0.97, 0.96).

| Module | Class | Camera RGB | Normalised v' | t_m | e_m |
|---|---|---|---|---|---|
| 1 | dark | (40, 45, 190) | (0.1038, 0.1220, 0.8889) | 1.1164 | 0.6164 |
| 2 | light | (215, 212, 120) | (0.9292, 0.9366, 0.5000) | 0.4511 | −0.0489 |

Module 1 is clearly blue (bit 1). Module 2 lies between yellow and white. If both are in one cell, they add 0.6164 − 0.0489 = 0.5675 to that cell's score, which is positive, so they vote for bit 1.
