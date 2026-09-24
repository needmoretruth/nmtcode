# NMT Code — 4. Error correction

Specification version 0.1 (draft). Licensed under CC BY 4.0 (see `LICENSE` in this folder).

## 4.1 Scope

This chapter defines the error correction of the data area of the base (black-and-white) layer:

- the Reed-Solomon code (4.2 to 4.4);
- the four error-correction levels (4.5);
- the algorithm that splits the data area into blocks for any symbol size (4.6, 4.7);
- the order in which message bytes enter blocks and codewords leave them (4.8);
- what a decoder must do (4.9);
- the error-correction scheme of format version 0 and the scheme a later format version may add (4.10).

Not in this chapter:

- The code that protects the format word is defined in chapter 2.
- The number of data modules, the mapping of codeword bits to modules, and the modules left over after the last whole codeword are defined in chapter 5.
- The container bytes, including the padding that fills the message capacity exactly, are defined in chapter 3.
- The colour layer's error correction is defined in chapter 7.

## 4.2 Field and code

| Parameter | Value |
|---|---|
| Field | GF(2^8). An element is one byte; bit 7 is the coefficient of z^7 |
| Field polynomial | z^8 + z^4 + z^3 + z^2 + 1 (0x11D) |
| Primitive element | α = 0x02 (the class of z). α has order 255 |
| Code | Reed-Solomon, systematic, shortened to length n |
| Block length n | 3 ≤ n ≤ 255 bytes |
| Parity count P | even, 2 ≤ P ≤ n − 1 |
| Message length k | k = n − P, k ≥ 1 |
| First consecutive root b | 0: the roots of the generator are α^0, α^1, …, α^(P−1) |
| Minimum distance | P + 1 |

Field addition is bitwise XOR. Multiplication is polynomial multiplication modulo 0x11D. The field, primitive element and first root are the same as those of QR Code (ISO/IEC 18004:2024), so existing GF(2^8) tables and QR test vectors for the generator polynomial apply unchanged.

## 4.3 Generator polynomial

For a parity count P:

    g_P(x) = (x − α^0)(x − α^1) ··· (x − α^(P−1))

The product is monic and has degree P. It is written g_P(x) = x^P + g[1]·x^(P−1) + … + g[P], where g[0] = 1. Subtraction equals addition in GF(2^8).

Construction, coefficients highest degree first:

    g = [1]
    for i in 0 .. P−1:
        r = α^i
        g' = g followed by one 0            # multiply by x
        for j in 0 .. len(g)−1:
            g'[j+1] = g'[j+1] XOR mul(g[j], r)
        g = g'

## 4.4 Systematic encoding

A block holds k message bytes m[0], …, m[k−1] and P parity bytes p[0], …, p[P−1]. Its codeword is the byte sequence

    c = m[0], …, m[k−1], p[0], …, p[P−1]      (n = k + P bytes)

Byte j of the block (0 ≤ j < n) is the coefficient of x^(n−1−j) in the codeword polynomial c(x). The message polynomial is m(x) = m[0]·x^(k−1) + … + m[k−1]. The parity polynomial is

    p(x) = (m(x) · x^P) mod g_P(x),   p[0] is the coefficient of x^(P−1)

and c(x) = m(x)·x^P + p(x). Then c(α^i) = 0 for 0 ≤ i < P.

Division by a linear-feedback shift register gives the same result:

    rem = [0] * P
    for each message byte m in m[0] .. m[k−1]:
        f = m XOR rem[0]
        rem = rem[1..P−1] followed by 0       # shift left by one byte
        for j in 0 .. P−1:
            rem[j] = rem[j] XOR mul(f, g[j+1])
    p = rem

## 4.5 Error-correction levels

The level is the 2-bit field of the format word (chapter 2). It sets the target share of parity bytes among all codewords.

| Level lvl | Name | Target parity share | Parity numerator q[lvl] (share = q[lvl] / 100) | Correctable byte errors | Correctable byte erasures | Minimum N (4.6) |
|---|---|---|---|---|---|---|
| 0 | L | at least 15% | 15 | about 7.5% of the codewords | about 15% | 3 |
| 1 | M | at least 30% | 30 | about 15% | about 30% | 3 |
| 2 | Q | at least 50% | 50 | about 25% | about 50% | 3 |
| 3 | H | at least 60% | 60 | about 30% | about 60% | 5 |

A block with P parity bytes corrects up to P/2 byte errors, or up to P byte erasures (4.9). The level is defined by its parity share; the recoverable share is half of it for errors. The four levels match the nominal recovery capacity of QR Code's levels L, M, Q and H (about 7, 15, 25 and 30%), so a comparison with QR Code at "the same level" compares equal correcting power.

Every block of a symbol meets the target share: P · 100 ≥ n · q[lvl] for every block length n.

## 4.6 Block splitting

The input is:

- N, the number of codewords in the data area. Chapter 5 gives the number of data modules D for the symbol; N = floor(D / 8).
- lvl, the error-correction level (0 to 3): the format word's level for the base layer (chapter 2), `chroma_ecc_level` for the colour layer (chapter 7, 7.8.2).

The output is:

- B, the number of blocks;
- n[b], the length of block b, for 0 ≤ b < B;
- P, the parity count, the same for every block;
- k[b] = n[b] − P, the message bytes of block b;
- K, the message capacity of the symbol in bytes.

All arithmetic is on non-negative integers. `ceil_div(a, d)` is floor((a + d − 1) / d).

    q = [15, 30, 50, 60]

    function split(N, lvl):
        require 0 <= lvl <= 3 and N >= N_min[lvl]   # N_min = [3, 3, 3, 5]
        B     = ceil_div(N, 255)             # fewest blocks with n <= 255
        s     = floor(N / B)                 # short block length
        r     = N mod B                      # number of long blocks
        n_max = ceil_div(N, B)               # longest block length
        P     = 2 * ceil_div(n_max * q[lvl], 200)
        for b in 0 .. B−1:
            n[b] = s + 1 if b < r else s     # long blocks come first
            k[b] = n[b] − P
        K = N − B * P
        return B, n, P, k, K

Properties of the result, for every level lvl and every N ≥ N_min[lvl]:

- Every block length is at most 255, and two block lengths differ by at most one.
- The block lengths add up to N, so every codeword of the data area belongs to exactly one block.
- P is even and at least 2.
- P is computed from the longest block and rounded up to an even number, so every block meets the target share of 4.5. The rounding up makes the actual share higher than the target for small N (for example 18.8% instead of 15% at N = 32, level 0).
- Every block has at least one message byte. For B = 1 this holds from N_min[lvl] upward. At level 3, N = 3 would leave one message byte but N = 4 would leave none (P = 4), so the minimum is set at 5, above which every N works. For B ≥ 2 every block has at least 128 bytes, P is at most 154 (level 3) and every block keeps at least 50 message bytes.

Rules:

- A generator MUST NOT produce a layer whose N is less than N_min[lvl] (3 for levels 0 to 2, 5 for level 3). A reader MUST reject such a layer. The base layer always meets this: the smallest symbol, 20 × 20, has N = 20 (table below). The colour layer has its own, higher minimum (chapter 7, 7.8.2).
- A generator and a reader MUST use exactly this algorithm. No other block layout is allowed in format version 0.
- The data area always uses all N codewords. The generator fills the message to exactly K bytes by the padding rule of chapter 3 (3.8); the container MUST fit in K bytes (chapter 3, 3.2.3).

Smallest symbol at each level. Chapter 5 (5.7) gives D; the smallest container (lead byte, body length, content type, CRC-32C, empty content) is 7 bytes (chapter 3, 3.2).

| Level | Smallest symbol | N | P | K | Largest content of one record, codec 0 |
|---|---|---|---|---|---|
| 0 | 20 × 20 | 20 | 4 | 16 | 9 bytes |
| 1 | 20 × 20 | 20 | 6 | 14 | 7 bytes |
| 2 | 20 × 20 | 20 | 10 | 10 | 3 bytes |
| 3 | 20 × 20 | 20 | 12 | 8 | 1 byte |

Why the fewest blocks: with a fixed parity share, a longer block corrects more errors in total, and the interleaving of 4.8 already spreads a local defect across blocks. The decoding cost of a 255-byte block is small against the reader's frame budget.

## 4.7 Block splits for sample sizes

Computed with the algorithm of 4.6. "Blocks" lists the long blocks first.

| N | Level | B | Blocks (count × length) | P | Errors per block (P/2) | K | Actual parity share |
|---|---|---|---|---|---|---|---|
| 32 | 0 | 1 | 1 × 32 | 6 | 3 | 26 | 18.8% |
| 32 | 1 | 1 | 1 × 32 | 10 | 5 | 22 | 31.2% |
| 32 | 2 | 1 | 1 × 32 | 16 | 8 | 16 | 50.0% |
| 32 | 3 | 1 | 1 × 32 | 20 | 10 | 12 | 62.5% |
| 50 | 0 | 1 | 1 × 50 | 8 | 4 | 42 | 16.0% |
| 50 | 1 | 1 | 1 × 50 | 16 | 8 | 34 | 32.0% |
| 50 | 2 | 1 | 1 × 50 | 26 | 13 | 24 | 52.0% |
| 50 | 3 | 1 | 1 × 50 | 30 | 15 | 20 | 60.0% |
| 64 | 0 | 1 | 1 × 64 | 10 | 5 | 54 | 15.6% |
| 64 | 1 | 1 | 1 × 64 | 20 | 10 | 44 | 31.2% |
| 64 | 2 | 1 | 1 × 64 | 32 | 16 | 32 | 50.0% |
| 64 | 3 | 1 | 1 × 64 | 40 | 20 | 24 | 62.5% |
| 128 | 0 | 1 | 1 × 128 | 20 | 10 | 108 | 15.6% |
| 128 | 1 | 1 | 1 × 128 | 40 | 20 | 88 | 31.2% |
| 128 | 2 | 1 | 1 × 128 | 64 | 32 | 64 | 50.0% |
| 128 | 3 | 1 | 1 × 128 | 78 | 39 | 50 | 60.9% |
| 512 | 0 | 3 | 2 × 171 + 1 × 170 | 26 | 13 | 434 | 15.2% |
| 512 | 1 | 3 | 2 × 171 + 1 × 170 | 52 | 26 | 356 | 30.5% |
| 512 | 2 | 3 | 2 × 171 + 1 × 170 | 86 | 43 | 254 | 50.4% |
| 512 | 3 | 3 | 2 × 171 + 1 × 170 | 104 | 52 | 200 | 60.9% |
| 1311 | 0 | 6 | 3 × 219 + 3 × 218 | 34 | 17 | 1107 | 15.6% |
| 1311 | 1 | 6 | 3 × 219 + 3 × 218 | 66 | 33 | 915 | 30.2% |
| 1311 | 2 | 6 | 3 × 219 + 3 × 218 | 110 | 55 | 651 | 50.3% |
| 1311 | 3 | 6 | 3 × 219 + 3 × 218 | 132 | 66 | 519 | 60.4% |
| 13000 | 0 | 51 | 46 × 255 + 5 × 254 | 40 | 20 | 10960 | 15.7% |
| 13000 | 1 | 51 | 46 × 255 + 5 × 254 | 78 | 39 | 9022 | 30.6% |
| 13000 | 2 | 51 | 46 × 255 + 5 × 254 | 128 | 64 | 6472 | 50.2% |
| 13000 | 3 | 51 | 46 × 255 + 5 × 254 | 154 | 77 | 5146 | 60.4% |

## 4.8 Message assignment and the codeword stream

### 4.8.1 Message bytes into blocks

Chapter 3 delivers the message stream M[0], …, M[K−1]. The stream fills the blocks in block order, each block with a contiguous run:

    offset = 0
    for b in 0 .. B−1:
        block b message bytes m[0 .. k[b]−1] = M[offset .. offset + k[b] − 1]
        offset = offset + k[b]

Each block is then encoded by 4.4.

### 4.8.2 Codewords out of blocks

The codeword stream c[0], …, c[N−1] takes the bytes of the encoded blocks round-robin, by byte position. It starts with byte 0 of block 0, byte 0 of block 1, …, byte 0 of block B−1, then byte 1 of block 0, and so on. A block that has no byte at the current position is skipped:

    i = 0
    for j in 0 .. n_max − 1:
        for b in 0 .. B−1:
            if j < n[b]:
                c[i] = byte j of block b
                i = i + 1

Byte j of block b is the byte of 4.4: message bytes first, then parity bytes.

Closed form, with s and r from 4.6:

- For 0 ≤ j < s and 0 ≤ b < B: byte j of block b is c[j · B + b].
- If r > 0, for 0 ≤ b < r: byte s of block b is c[s · B + b]. These are the last r codewords of the stream.

Chapter 5 places c[0], c[1], … on the data modules in its placement order, each codeword most significant bit first. A reader reverses the mapping above to rebuild the blocks.

Consequence: any run of up to B consecutive codewords in the stream touches each block at most once.

## 4.9 Decoder requirements

Terms:

- An **error** is a codeword byte whose value is wrong and whose position the decoder does not know in advance.
- An **erasure** is a codeword byte that the reader has marked as unreliable before decoding, for example because a module in it could not be classified. Its value is ignored.

For a block with parity count P, e errors and s erasures:

- A decoder can correct the block when 2e + s ≤ P.
- A reader MUST correct every block with 2e ≤ P and no erasures.
- A reader SHOULD mark erasures. A reader that passes erasures to the decoder MUST correct every block with 2e + s ≤ P.
- A reader MAY decode one block several times with different erasure sets, for example by dropping the least reliable marks when s > P.

A bounded-distance decoder (one that corrects only within 2e + s ≤ P) MUST reject the block, rather than output a corrected block, in each of these cases:

- s > P;
- the error-locator polynomial has a degree larger than (P − s) / 2;
- the number of distinct roots of the error locator in the field differs from its degree;
- a root points to a position outside the n bytes of the shortened block;
- the syndromes of the corrected block, recomputed, are not all zero.

If any block of the base layer is rejected, the base layer is undecodable. The reader MUST NOT present any content of that symbol and MAY try another capture.

No guarantee against miscorrection. When 2e + s > P, a bounded-distance decoder can return a valid but wrong codeword without any sign of failure. For a received word that is far from every codeword, published analysis puts the chance of such a wrong decoding at roughly 1/t! with t = P/2 (R. J. McEliece and L. Swanson, "On the decoder error probability for Reed-Solomon codes", IEEE Transactions on Information Theory 32(5), 1986). At P = 2 or P = 4 that chance is large. This chapter therefore promises detection of nothing. The CRC-32C at the end of the container (chapter 3) is the check that catches a miscorrection, and a reader MUST verify it after error correction.

A reader MAY use a decoder that corrects beyond 2e + s ≤ P (for example list decoding or decoding with soft module values). Every result of such a decoder is still subject to the CRC-32C check of chapter 3 and to the other checks of chapters 2 to 6.

## 4.10 Error-correction scheme of format version 0

In format version 0 the base layer's error correction is always the Reed-Solomon code of this chapter. The format word has no field that selects another scheme.

A later format version (chapter 9, 9.2) may define a soft-decision LDPC profile. The candidate construction is a binary LDPC code whose parity-check matrix is built by plain progressive edge growth (PEG) or as a regular Gallager matrix, decoded with belief propagation on a flooding schedule. It would be defined only if measurement on real captures shows a clear gain in payload per module over this Reed-Solomon code at a decoding cost within the reader's frame budget (chapter 1, 1.2).

Design note — code families this specification will not use:

| Family | Reason |
|---|---|
| 5G NR LDPC base graphs and polar codes (3GPP TS 38.212) | declared under the ETSI IPR policy, which licenses on FRAND terms, not royalty-free |
| Rate-compatible protograph LDPC families | covered by patents until 2032-04 (for example US 8,689,083) |
| RaptorQ (RFC 6330) | covered by declared patents until 2030-12 (for example US 9,419,749); the holder's free covenant excludes phones |
| Raptor (RFC 5053) | follow-on patents declared against it run until about 2028-11 (for example US 8,887,020) |
| Layered (row-by-row) LDPC decoding schedules | covered by patents until 2028-08 (for example US 7,730,377); the flooding schedule avoids them |
| Standard quasi-cyclic base matrices | copying a standard's base matrix brings that standard's patent declarations with it; a later profile chooses its own circulant shifts |

Reed-Solomon codes (1960), Gallager's regular LDPC codes (1962) and plain PEG construction (published 2001 and 2005) have no known live patent.

## 4.11 Worked examples

Values in this section were computed by a script that implements 4.2 to 4.8. Annex A (A.2) gives a complete symbol with N = 42 at level 0.

### 4.11.1 N = 50 codewords at level 1 (M)

Block split (4.6):

| Step | Value |
|---|---|
| B = ceil_div(50, 255) | 1 |
| s = floor(50 / 1), r = 50 mod 1 | 50, 0 |
| n_max | 50 |
| P = 2 · ceil_div(50 · 30, 200) = 2 · ceil_div(1500, 200) | 16 |
| n[0], k[0] | 50, 34 |
| K | 34 |
| Actual parity share | 16 / 50 = 32.0% |

Generator polynomial g_16(x), coefficients g[0] … g[16], highest degree first:

    hex:            01 3B 0D 68 BD 44 D1 1E 08 A3 41 29 E5 62 32 24 3B
    as powers of α: α^0 α^120 α^104 α^107 α^109 α^102 α^161 α^76 α^3
                    α^91 α^191 α^147 α^169 α^182 α^194 α^225 α^120

Message bytes m[0] … m[33] (M[i] = i):

    00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10
    11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21

Parity bytes p[0] … p[15]:

    36 5A E8 16 6F 40 AC F0 72 10 3F 0F BB 20 DD F4

The block has one member, so the codeword stream c[0] … c[49] is the 34 message bytes followed by the 16 parity bytes. The recomputed syndromes c(α^0) … c(α^15) are all 00.

### 4.11.2 Stream order with unequal blocks: N = 512 at level 1 (M)

Block split: B = 3, s = 170, r = 2, so n = [171, 171, 170], P = 52, k = [119, 119, 118], K = 356. Block 0 takes M[0 … 118], block 1 takes M[119 … 237], block 2 takes M[238 … 355].

| Stream index | Block | Byte of block |
|---|---|---|
| c[0] | 0 | 0 |
| c[1] | 1 | 0 |
| c[2] | 2 | 0 |
| c[3] | 0 | 1 |
| c[509] | 2 | 169 |
| c[510] | 0 | 170 |
| c[511] | 1 | 170 |

Byte 170 exists only in the two long blocks, so the stream ends with it for blocks 0 and 1.
