# 6. Payload coding

Spec version 0.1 (draft). This chapter is part of the NMT Code format specification and is licensed under CC BY 4.0.

This chapter defines the codec layer: how the content bytes of a symbol are turned into the container's coded field and back. The container fields that carry the codec ID, the dictionary ID and the declared content length are defined in 03-container-and-records.md. Error correction (04-error-correction.md) and placement (05-geometry-and-placement.md) act on the container after this layer.

## 6.1 Interface

A decoder of this layer receives four values from the container (03-container-and-records.md):

| Name | Meaning | Range |
|---|---|---|
| `c` | codec ID | unsigned integer; see 6.2 |
| `d` | dictionary ID | unsigned integer; see 6.11 |
| `L` | declared content length: the number of content bytes after decoding | 0 ≤ `L` ≤ `LIMIT` (6.4) |
| `C` | coded field: the bytes this layer decodes | `N` bytes, `N` ≥ 0, delimited by the container |

The output is exactly `L` bytes of content. Content is an octet string; its meaning is given by the container's content type, which no codec reads.

The encoder receives the content bytes and produces (`c`, `d`, `L`, `C`).

## 6.2 Codec ID registry

The codec ID registry is append-only. An assigned ID never changes meaning and is never reused.

| ID | Name | Applicable content | Dictionary ID `d` | Section |
|---:|---|---|---|---|
| 0 | stored | any | MUST be 0 | 6.5 |
| 1 | digits | every byte in 0x30–0x39 (`0`–`9`) | MUST be 0 | 6.6 |
| 2 | upper-alphanumeric | every byte in the 45-character set of 6.7 | MUST be 0 | 6.7 |
| 3 | token table + short-text model | any | 0 = built-in model 0, or a registered dictionary of kind "short-text model" | 6.8 |
| 4 | Hangul syllable packing | well-formed UTF-8 | MUST be 0 | 6.9 |
| 5 | brotli | any | 0 = brotli's built-in dictionary only, or a registered dictionary of kind "brotli prefix" | 6.10 |
| 6–9 | reserved: text and entropy codecs (for example a frequency-ordered Hangul syllable table) | — | — | — |
| 10–13 | reserved: structured-data codecs (for example a format-aware codec for tabular or schema-described data) | — | — | — |
| 14–15 | reserved | — | — | — |
| ≥ 16 | reserved for future format versions | — | — | — |

- IDs 6–15 MAY be assigned by a later 0.x revision of this specification without a new format version. A decoder that does not implement an ID MUST fail with `E_UNSUPPORTED_CODEC`.
- IDs ≥ 16 MUST NOT appear in format version 0. A format-version-0 decoder MUST fail with `E_UNSUPPORTED_CODEC`.
- A decoder MUST fail with `E_DICTIONARY_MISMATCH` when `d` is not allowed for `c` by the table above, or when the registered kind of `d` (6.11) does not match `c`.

## 6.3 Encoder selection rule

1. The encoder computes the coded field for every applicable codec (6.2) and, for codecs 3 and 5, for dictionary 0 and every registered dictionary of the matching kind that it carries.
2. It compares the candidates by the total size, in bytes, of the container fields that depend on the choice (the codec ID field, the dictionary ID field and the coded field, as serialised by 03-container-and-records.md). With one-byte codec and dictionary ID fields this is the same as comparing `N`.
3. It picks the smallest. Ties go to the lowest codec ID, then to the lowest dictionary ID.
4. Stored (codec 0) is always a candidate. So when no codec is shorter than stored, stored is chosen by rule 3.

An encoder in its default mode MUST apply this rule. An encoder MAY offer a documented mode that restricts the candidate set (for example to skip codec 5 for speed). Decoders do not depend on which candidate was chosen. Codec 5 output depends on the brotli encoder implementation, so two conforming encoders can pick different candidates for the same content.

## 6.4 Decoder safety

Limits:

| Name | Value | Meaning |
|---|---|---|
| `MAX_CONTENT_LEN_V0` | 16,777,216 (16 MiB) | absolute cap on `L` in format version 0 |
| `LIMIT` | reader-chosen, ≤ `MAX_CONTENT_LEN_V0` | the cap a given reader applies; a reader MAY choose a lower value |

Rules:

1. The decoder MUST check `L` ≤ `LIMIT` before allocating any buffer that depends on `L`. If the check fails it MUST fail with `E_TOO_LARGE`.
2. The decoder MUST run the codec-specific length checks of 6.6–6.10 before allocating.
3. The decoder allocates at most `L` bytes of output plus codec state bounded in this chapter (the brotli window of 6.10 and the tables of 6.8).
4. Output MUST be exactly `L` bytes. A codec that would produce more than `L` bytes fails at the first byte beyond `L`; a codec whose input ends before `L` bytes are produced fails.
5. Every malformed input — an out-of-range value, a reserved code, a read past the end of `C` (except the zero extension that codec 3 defines), trailing data, non-zero padding bits — MUST produce `E_MALFORMED`. It MUST NOT cause a panic, an abort, an out-of-bounds access or unbounded work.
6. Decoding work MUST be bounded by a constant times (`N` + `L` + the size of the dictionary in use).

Error classes (names used in this chapter; 09-versions-and-registries.md may map them to reader messages):

| Error | Condition | Reader message intent |
|---|---|---|
| `E_UNSUPPORTED_CODEC` | codec ID reserved or not implemented | a newer reader is needed |
| `E_UNKNOWN_DICTIONARY` | `d` ≠ 0 and not in the reader's registry revision, or in the private-use range | a newer reader, or the issuing application, is needed |
| `E_DICTIONARY_MISMATCH` | `d` not allowed for `c` | the symbol is invalid |
| `E_TOO_LARGE` | `L` > `LIMIT` | the content exceeds this reader's limit |
| `E_MALFORMED` | any other decoding failure | the symbol is invalid |

## 6.5 Codec 0 — stored

`C` is the content. The decoder MUST fail with `E_MALFORMED` unless `N` = `L`.

## 6.6 Codec 1 — digits

Applicable when every content byte is an ASCII digit (0x30–0x39). The number of digits is `L`.

Encoding:

1. Split the digits, from the start, into groups of three. The last group has 1, 2 or 3 digits.
2. Each group is read as a decimal number `v` and written as an unsigned field, most significant bit first:

   | Digits in group | Field width | Valid `v` |
   |---:|---:|---|
   | 3 | 10 bits | 0–999 |
   | 2 | 7 bits | 0–99 |
   | 1 | 4 bits | 0–9 |

3. The fields are concatenated. Their total length is `B` = 10·⌊`L`/3⌋ + (0 if `L` mod 3 = 0, 4 if `L` mod 3 = 1, 7 if `L` mod 3 = 2).
4. The bit string is padded with 0 bits to a byte boundary. `N` = ⌈`B`/8⌉.

Decoding: the decoder MUST check `N` = ⌈`B`/8⌉ before allocating. It MUST fail with `E_MALFORMED` on a group value outside the valid range or a non-zero padding bit. Leading zeros are preserved because the group width, not the value, fixes the number of digits.

Density: 10 bits per 3 digits, the same as QR numeric mode (3.33 bits per digit; the information content of a random digit is 3.32 bits).

## 6.7 Codec 2 — upper-case alphanumeric

Applicable when every content byte is in this 45-character set. The value of a character is its position in the list:

| Values | Characters |
|---|---|
| 0–9 | `0`–`9` |
| 10–35 | `A`–`Z` |
| 36–44 | space, `$`, `%`, `*`, `+`, `-`, `.`, `/`, `:` |

This is the same set, in the same value order, as QR alphanumeric mode. The set is kept, although it lacks `?`, `=`, `&` and lower case, so that every string QR can put in alphanumeric mode is coded here at the same density; content outside the set goes to codec 3. Two characters in 11 bits is within 0.2 % of the information bound log2 45 = 5.49 bits per character, so a different packing of this set gains nothing measurable.

Encoding:

1. Pair the characters from the start. A pair (`a`, `b`) is written as 45·`a` + `b` in 11 bits (valid 0–2024).
2. If `L` is odd, the last character is written in 6 bits (valid 0–44).
3. `B` = 11·⌊`L`/2⌋ + 6·(`L` mod 2). Pad with 0 bits to a byte boundary. `N` = ⌈`B`/8⌉.

Decoding: the decoder MUST check `N` = ⌈`B`/8⌉ before allocating, and MUST fail with `E_MALFORMED` on a pair value > 2024, a single value > 44, or a non-zero padding bit.

## 6.8 Codec 3 — token table and short-text model

Codec 3 codes the content as a sequence of symbols. Each symbol is a literal byte or a token from a static table. The symbols are entropy-coded with a range coder under a static model. The model is model 0 (6.8.3) when `d` = 0, or a registered short-text model dictionary (6.8.4) otherwise.

### 6.8.1 Symbol alphabet

| Symbol index | Meaning | Output bytes |
|---|---|---|
| 0–255 | literal byte of that value | 1 |
| 256 + `i`, 0 ≤ `i` < `TOKEN_COUNT` | token `i` of the token table (6.8.2) | the token's bytes |

`TOKEN_COUNT` = 240 in this draft, so the alphabet size is `A` = 496.

### 6.8.2 Token table

Parameter `TOKEN_TABLE_V0`: tunable in 0.x, fixed before 1.0. The entries below are a draft chosen from public knowledge of common URL and payload parts: URI schemes and payload prefixes (IDs 0–29), host pieces and common hosts (30–65), top-level domains from the IANA root zone database with a leading dot, plus common second-level registrations (66–138), path and query pieces (139–208), keys of Wi-Fi, one-time-password and payment payloads (209–222), and English text pieces (223–239). The final table is tuned against the measurement corpus in stage 7 of the project plan and frozen before 1.0.

Each token is shown as a JSON string literal (so `"\r\n"` is the two bytes 0x0D 0x0A and spaces are visible). Every token is ASCII.

| ID | Token | ID | Token | ID | Token | ID | Token |
|---:|---|---:|---|---:|---|---:|---|
| 0 | `"https://www."` | 60 | `"pf.kakao.com"` | 120 | `".online"` | 180 | `"/product/"` |
| 1 | `"http://www."` | 61 | `"smartstore.naver.com"` | 121 | `".shop"` | 181 | `"/item/"` |
| 2 | `"https://"` | 62 | `"blog.naver.com"` | 122 | `".store"` | 182 | `"/event/"` |
| 3 | `"http://"` | 63 | `"cafe.naver.com"` | 123 | `".to"` | 183 | `"/news/"` |
| 4 | `"HTTPS://"` | 64 | `"t.me"` | 124 | `".gg"` | 184 | `"/blog/"` |
| 5 | `"HTTP://"` | 65 | `"wa.me"` | 125 | `".co.kr"` | 185 | `"/post/"` |
| 6 | `"mailto:"` | 66 | `".com/"` | 126 | `".or.kr"` | 186 | `"/view/"` |
| 7 | `"tel:"` | 67 | `".net/"` | 127 | `".go.kr"` | 187 | `"/detail"` |
| 8 | `"sms:"` | 68 | `".org/"` | 128 | `".ac.kr"` | 188 | `"/page/"` |
| 9 | `"smsto:"` | 69 | `".kr/"` | 129 | `".ne.kr"` | 189 | `"/home"` |
| 10 | `"SMSTO:"` | 70 | `".com"` | 130 | `".co.jp"` | 190 | `"/main"` |
| 11 | `"geo:"` | 71 | `".net"` | 131 | `".ne.jp"` | 191 | `"/shop/"` |
| 12 | `"ftp://"` | 72 | `".org"` | 132 | `".or.jp"` | 192 | `"/user/"` |
| 13 | `"wss://"` | 73 | `".edu"` | 133 | `".co.uk"` | 193 | `"/share/"` |
| 14 | `"ws://"` | 74 | `".gov"` | 134 | `".com.au"` | 194 | `"/invite/"` |
| 15 | `"magnet:?xt=urn:btih:"` | 75 | `".io"` | 135 | `".com.br"` | 195 | `"/join"` |
| 16 | `"market://details?id="` | 76 | `".co"` | 136 | `".com.cn"` | 196 | `"/docs/"` |
| 17 | `"bitcoin:"` | 77 | `".kr"` | 137 | `".com.tw"` | 197 | `"/tree/"` |
| 18 | `"ethereum:"` | 78 | `".jp"` | 138 | `".co.za"` | 198 | `"/blob/"` |
| 19 | `"lightning:"` | 79 | `".cn"` | 139 | `"/index.html"` | 199 | `"/releases"` |
| 20 | `"otpauth://totp/"` | 80 | `".de"` | 140 | `"index."` | 200 | `"/issues/"` |
| 21 | `"otpauth://hotp/"` | 81 | `".uk"` | 141 | `".html"` | 201 | `"/pull/"` |
| 22 | `"WIFI:S:"` | 82 | `".fr"` | 142 | `".htm"` | 202 | `"/status/"` |
| 23 | `"WIFI:T:WPA;S:"` | 83 | `".ru"` | 143 | `".php"` | 203 | `"/channel/"` |
| 24 | `"MECARD:N:"` | 84 | `".br"` | 144 | `".aspx"` | 204 | `"/playlist?list="` |
| 25 | `"BEGIN:VCARD"` | 85 | `".in"` | 145 | `".asp"` | 205 | `"/reel/"` |
| 26 | `"END:VCARD"` | 86 | `".it"` | 146 | `".jsp"` | 206 | `"/shorts/"` |
| 27 | `"VERSION:3.0"` | 87 | `".es"` | 147 | `".do"` | 207 | `"/article/"` |
| 28 | `"VERSION:4.0"` | 88 | `".nl"` | 148 | `".pdf"` | 208 | `"/search"` |
| 29 | `"LOCATION:"` | 89 | `".au"` | 149 | `".jpg"` | 209 | `";T:WPA;P:"` |
| 30 | `"www."` | 90 | `".ca"` | 150 | `".png"` | 210 | `";P:"` |
| 31 | `"m."` | 91 | `".ch"` | 151 | `".json"` | 211 | `";H:true;"` |
| 32 | `"mail."` | 92 | `".se"` | 152 | `"/?"` | 212 | `";;"` |
| 33 | `"blog."` | 93 | `".no"` | 153 | `"?id="` | 213 | `"?secret="` |
| 34 | `"docs."` | 94 | `".pl"` | 154 | `"&id="` | 214 | `"&issuer="` |
| 35 | `"api."` | 95 | `".eu"` | 155 | `"?v="` | 215 | `"&algorithm=SHA1"` |
| 36 | `"app."` | 96 | `".us"` | 156 | `"/watch?v="` | 216 | `"&digits=6"` |
| 37 | `"shop."` | 97 | `".me"` | 157 | `"?q="` | 217 | `"&period=30"` |
| 38 | `"cdn."` | 98 | `".tv"` | 158 | `"/search?q="` | 218 | `"?amount="` |
| 39 | `"play.google.com/store/apps/details?id="` | 99 | `".info"` | 159 | `"?page="` | 219 | `"&label="` |
| 40 | `"apps.apple.com/"` | 100 | `".biz"` | 160 | `"&page="` | 220 | `"&message="` |
| 41 | `"youtube.com"` | 101 | `".app"` | 161 | `"?ref="` | 221 | `"bc1q"` |
| 42 | `"youtu.be"` | 102 | `".dev"` | 162 | `"?si="` | 222 | `"0x"` |
| 43 | `"google.com"` | 103 | `".ai"` | 163 | `"?utm_source="` | 223 | `" the "` |
| 44 | `"github.com"` | 104 | `".xyz"` | 164 | `"&utm_source="` | 224 | `"The "` |
| 45 | `"naver.com"` | 105 | `".be"` | 165 | `"&utm_medium="` | 225 | `" and "` |
| 46 | `"naver.me"` | 106 | `".at"` | 166 | `"&utm_campaign="` | 226 | `" of "` |
| 47 | `"kakao.com"` | 107 | `".dk"` | 167 | `"&utm_content="` | 227 | `" to "` |
| 48 | `"instagram.com"` | 108 | `".fi"` | 168 | `"?lang="` | 228 | `" in "` |
| 49 | `"facebook.com"` | 109 | `".tw"` | 169 | `"/ko/"` | 229 | `" is "` |
| 50 | `"x.com"` | 110 | `".hk"` | 170 | `"/en/"` | 230 | `" for "` |
| 51 | `"twitter.com"` | 111 | `".sg"` | 171 | `"/ko-kr/"` | 231 | `" you"` |
| 52 | `"linkedin.com"` | 112 | `".vn"` | 172 | `"/en-us/"` | 232 | `" on "` |
| 53 | `"tiktok.com"` | 113 | `".id"` | 173 | `"/wiki/"` | 233 | `" with "` |
| 54 | `"amazon.com"` | 114 | `".th"` | 174 | `"/p/"` | 234 | `" that "` |
| 55 | `"wikipedia.org"` | 115 | `".mx"` | 175 | `"/s/"` | 235 | `"ing "` |
| 56 | `"bit.ly"` | 116 | `".ar"` | 176 | `"/api/"` | 236 | `"tion"` |
| 57 | `"forms.gle"` | 117 | `".tr"` | 177 | `"/login"` | 237 | `". "` |
| 58 | `"maps.app.goo.gl"` | 118 | `".ua"` | 178 | `"/download"` | 238 | `", "` |
| 59 | `"open.kakao.com"` | 119 | `".site"` | 179 | `"/products/"` | 239 | `"\r\n"` |

Transcription check: the serialisation that concatenates, in ID order, one byte holding the token's length followed by the token's bytes is 1,794 bytes long and has SHA-256 `a8e0f1be8a427a782b1c2e6179c1876a6d1690a74224f946c487e1ab803e3717`.

A token is a plain byte string. Matching is exact and case-sensitive, and a token can match anywhere (".com" inside "x.company" is legal; the decoder only concatenates bytes).

### 6.8.3 Model 0

Model 0 is an order-0 model: one frequency vector, no context. Parameter `MODEL0_WEIGHTS`: tunable in 0.x, fixed before 1.0.

| Symbols | Count | Frequency each |
|---|---:|---:|
| bytes 0x61–0x7A (`a`–`z`) | 26 | 1300 |
| bytes 0x30–0x39 (`0`–`9`) | 10 | 700 |
| bytes 0x41–0x5A (`A`–`Z`) | 26 | 250 |
| bytes of `/ . - _ = ? & : % # + ~` and space (0x20) | 13 | 560, except `/` (0x2F) = 864 |
| other bytes 0x21–0x7E | 20 | 60 |
| bytes 0x09, 0x0A, 0x0D | 3 | 60 |
| all other bytes (0x00–0x08, 0x0B, 0x0C, 0x0E–0x1F, 0x7F–0xFF) | 158 | 4 |
| tokens (symbols 256–495) | 240 | 36 |

The frequencies sum to exactly 65,536 = 2^16 (the frequency of `/` absorbs the remainder of 304).

### 6.8.4 Short-text model dictionaries

A registered dictionary of kind "short-text model" defines an order-`k` hashed context model. Its contents are tuned in stage 7 of the project plan; its byte format is fixed here.

Context and slot selection:

1. The context is the last `k` content bytes already output, oldest first. Positions before the start of the content count as 0x00.
2. `h` = FNV-1a 32-bit over those `k` bytes: `h` = 0x811C9DC5; for each byte `x`: `h` = (`h` XOR `x`) × 0x01000193 mod 2^32.
3. `slot` = `h` >> (32 − `b`) (the top `b` bits; `slot` = 0 when `b` = 0).
4. The frequency vector for the next symbol is `vectors[slotmap[slot]]`.

The model stores no check, parity or verification value per slot, and a decoder MUST NOT compare any stored value against the context. Contexts that hash to the same slot share one vector; collisions are accepted by design.

Byte format (big-endian fixed-width integers; LEB128 as defined in 01-scope-and-conventions.md):

| Offset | Size | Field | Valid range |
|---|---|---|---|
| 0 | 1 | format | 0 |
| 1 | 1 | order `k` | 0–4 |
| 2 | 1 | hash bits `b` | 0–12; `b` = 0 if and only if `k` = 0 |
| 3 | 2 | vector count `V` | 1–4096 |
| 5 | 2 | alphabet size `A` | 256 + `TOKEN_COUNT` (496) |
| 7 | 2·2^`b` | `slotmap`: one u16 per slot | each < `V` |
| 7 + 2·2^`b` | variable | `V` vectors, in index order | see below |

Each vector:

| Field | Encoding | Meaning |
|---|---|---|
| `n` | LEB128 | number of explicit entries, 0–`A` |
| `n` × (`gap`, `e`) | LEB128, LEB128 | the first entry's symbol is `gap`; each later entry's symbol is the previous symbol + 1 + `gap`; the symbol MUST be < `A`; its frequency is `e` + 2 |

Symbols without an explicit entry have frequency 1. The frequencies of every vector MUST sum to exactly 65,536. No bytes follow the last vector.

A decoder MUST reject (as a build or load error, before any symbol is decoded) a model dictionary that violates this table. Registered dictionaries are pinned by SHA-256 (6.11), so this check runs once per dictionary.

### 6.8.5 Range coder

Constants: frequency total 2^16; range width 32 bits; renormalisation threshold 2^24. For a vector `f`, `cum[s]` = Σ `f[t]` for `t` < `s`.

Decoder (normative). Let `C[i]` = 0 for `i` ≥ `N` (zero extension).

```
code  = C[0]<<24 | C[1]<<16 | C[2]<<8 | C[3]
range = 0xFFFFFFFF
p     = 4
out   = empty
while len(out) < L:
    f, cum = frequency vector selected by the model (6.8.3 or 6.8.4)
    r = range >> 16
    v = code / r                      (integer division)
    if v >= 65536: fail E_MALFORMED
    s = the unique symbol with cum[s] <= v < cum[s] + f[s]
    code  = code - r * cum[s]
    range = r * f[s]
    while range < 2^24:
        code  = (code << 8) | C[p];  p = p + 1
        range = range << 8
    append the output bytes of s to out
    if len(out) > L: fail E_MALFORMED
if N > p: fail E_MALFORMED             (bytes the decoder never read)
if N > 0 and C[N-1] == 0x00: fail E_MALFORMED
```

All values fit in 32 bits (`code` < `range` ≤ 2^32 − 1 holds after every step for a valid stream). The final two checks make the length of `C` canonical for a given symbol sequence.

Encoder (normative by its result; the description uses exact integers):

```
low = 0 (unbounded integer); range = 0xFFFFFFFF; n = 0
for each symbol s, with f, cum from the same model state the decoder will see:
    r = range >> 16
    low   = low + r * cum[s]
    range = r * f[s]
    while range < 2^24:
        low = low << 8;  range = range << 8;  n = n + 1
M = n + 4
find the smallest m, 0 <= m <= M, such that
    V = ceil(low / 2^(8(M-m))) * 2^(8(M-m))   satisfies   V < low + range
C = the first m bytes of V written as an M-byte big-endian integer
```

`V` lies in the final interval, so the decoder reproduces every symbol. By minimality, `C` never ends in 0x00, and `N` = `m` ≤ `M` = the decoder's final `p`. An implementation with a fixed-width `low` MUST propagate carries into bytes already produced (as the LZMA range encoder does with its cached byte); this does not change the output.

Termination is by `L`: there is no end-of-stream symbol. Empty content gives `N` = 0.

### 6.8.6 Reference parse

The decoder accepts any symbol sequence. The reference encoder, used for test vectors, parses greedily: at each position it takes the longest token that matches there (two distinct tokens of equal length cannot both match at one position), and a literal byte when no token matches. Encoders MAY use any other parse, for example one that minimises the coded length.

## 6.9 Codec 4 — Hangul syllable packing

Applicable when the content is well-formed UTF-8 (no surrogate code points, no overlong forms, no value above U+10FFFF). The output of a valid decode is always well-formed UTF-8.

### 6.9.1 Structure

- The coded field is a bit string, most significant bit first. When `L` = 0, `N` MUST be 0.
- Bit 0 is the initial mode: 0 = H mode, 1 = A mode.
- Then units follow. In H mode a unit is 14 bits; in A mode, 7 bits. Some units change the mode and output nothing.
- Decoding stops when exactly `L` bytes have been output. After the last unit, fewer than 8 bits remain, and they MUST all be 0. A unit that would take the output beyond `L` bytes, or a read beyond the end of `C`, is `E_MALFORMED`.
- Before allocating, the decoder MUST check 7·`L` ≤ 16·`N` (a 14-bit unit outputs at most 4 bytes), else `E_MALFORMED`.

### 6.9.2 H-mode units (14 bits)

Hangul syllables use Unicode's arithmetic decomposition: syllable = U+AC00 + (`LIdx`·21 + `VIdx`)·28 + `TIdx`, with 19 leading consonants, 21 vowels and 28 trailing-consonant values (`TIdx` = 0 is "no final consonant"). No Unicode data file is needed.

Parameter `HANGUL_SPACE_FINALS` (tunable in 0.x, fixed before 1.0) is the ordered list `F` = [0, 1, 4, 7, 8, 16, 17, 19, 20, 21, 23, 26] of `TIdx` values, that is: none, ㄱ, ㄴ, ㄷ, ㄹ, ㅁ, ㅂ, ㅅ, ㅆ, ㅇ, ㅊ, ㅍ. These are the finals that most often end a Korean word; a syllable with one of them followed by a space is coded in one unit.

| Code `u` | Count | Output |
|---|---:|---|
| 0–11171 | 11,172 | the syllable U+AC00 + `u` |
| 11172–15959 | 4,788 | a syllable followed by U+0020: with `m` = `u` − 11172, `j` = ⌊`m`/399⌋, `LV` = `m` mod 399, the syllable U+AC00 + 28·`LV` + `F[j]` |
| 15960–16087 | 128 | U+0000–U+007F (`u` − 15960) |
| 16088–16183 | 96 | U+00A0–U+00FF |
| 16184–16231 | 48 | U+2010–U+203F (dashes, quotation marks, bullets, ellipsis, ※) |
| 16232–16263 | 32 | U+3000–U+301F (ideographic space, CJK punctuation and brackets) |
| 16264–16357 | 94 | U+3131–U+318E (Hangul compatibility jamo, such as ㅋ and ㅠ) |
| 16358–16361 | 4 | ". ", ", ", "? ", "! " (the character followed by U+0020) |
| 16362 | 1 | `SWITCH_A`: continue in A mode |
| 16363 | 1 | `ESC16`: the next 16 bits are a code point 0x0000–0xFFFF; 0xD800–0xDFFF is `E_MALFORMED` |
| 16364 | 1 | `ESC21`: the next 21 bits are a code point 0x000000–0x10FFFF; surrogates or larger values are `E_MALFORMED` |
| 16365–16383 | 19 | reserved: `E_MALFORMED` |

Each output code point is appended in UTF-8.

### 6.9.3 A-mode units (7 bits)

| Code | Output |
|---|---|
| 0x00–0x7E | that ASCII character |
| 0x7F | `SWITCH_H`: continue in H mode (U+007F itself is coded in H mode) |

A-mode runs make ASCII cheap inside Korean text: a run of `n` ASCII characters costs 7·`n` bits plus 14 bits to enter and 7 bits to return, instead of 14·`n` bits as direct H-mode codes. Entering A mode saves bits for a run of 4 or more characters inside a text (3 is a tie) and of 3 or more at its end (2 is a tie).

### 6.9.4 Reference encoder

The decoder accepts any unit sequence. The reference encoder, used for test vectors, minimises the total bit length by dynamic programming over (position, mode). Ties are broken by taking, at each step from the start, the first option in this order that still reaches the minimum: in H mode — syllable + space, syllable, punctuation + space, direct code, escape, `SWITCH_A`; in A mode — literal, `SWITCH_H`. The initial mode is H when both modes reach the minimum. The reference encoder never emits two mode switches in a row. An escape is used only for a code point that has no direct code (ESC16 in the BMP, ESC21 above it).

### 6.9.5 Density

| Coding | Bits per syllable | Bits for a word-final syllable and its space | Coverage |
|---|---:|---:|---|
| codec 4 | 14 | 14 (final in `F`), else 28 | all 11,172 syllables and, by escape, every scalar value |
| QR byte mode, UTF-8 | 24 | 32 | all |
| QR byte mode, EUC-KR through an ECI designator | 16 (+12 once for the ECI header) | 24 | the 2,350 syllables of KS X 1001 only; reader support varies |
| information bound, uniform over all syllables | 13.45 | — | — |

Relative to UTF-8 in QR this is 0.58× per syllable, and less when spaces are merged; relative to EUC-KR through ECI it is 0.875× per syllable. Frequent syllables cannot become cheaper than 14 bits in this codec. A frequency-ordered syllable table or a syllable context model, which needs corpus data, may be added later as a new codec ID in the range 6–9.

## 6.10 Codec 5 — brotli

`C` is exactly one brotli stream as defined by RFC 7932.

- `d` = 0: the stream is decoded as RFC 7932 specifies, with the built-in static dictionary of RFC 7932 Appendix A.
- `d` ≠ 0, a registered dictionary of kind "brotli prefix" with bytes `D`: the decoder behaves as an RFC 7932 decoder that has already output the `len(D)` bytes of `D` before the first meta-block. `D` is in the sliding window, the count of bytes already decoded (which bounds backward distances and decides whether a distance refers to the static dictionary) includes `len(D)`, and the two previous bytes used for literal context modelling are the last two bytes of `D`. The bytes of `D` are not part of the output. The RFC 7932 static dictionary stays available.

Rules:

1. The window size is the one the stream header declares (WBITS 10–24 in RFC 7932). Streams in the large-window extension beyond RFC 7932 are `E_MALFORMED`.
2. A decoder SHOULD allocate a ring buffer no larger than min(2^WBITS, `len(D)` + `L`), rounded up as its implementation needs.
3. The decoder MUST stop with `E_MALFORMED` at the first output byte beyond `L`. It MUST NOT use a "grow the output until the stream ends" mode without this cap.
4. After the meta-block with ISLAST = 1 (and, if set, ISLASTEMPTY), the unused bits of the final byte MUST be 0 and no bytes may follow in `C`; otherwise `E_MALFORMED`.
5. The total output MUST equal `L`.
6. Encoders MUST NOT emit metadata meta-blocks; decoders skip them as RFC 7932 requires.

## 6.11 Dictionary registry

Dictionary IDs name byte strings that every reader of a format version carries. A symbol never contains a dictionary; it names one by its ID in the container.

ID ranges:

| Range | Use |
|---|---|
| 0 | none: codec 3 uses model 0, codec 5 uses only the RFC 7932 built-in dictionary |
| 1–127 | dictionaries registered by this specification |
| 128–16383 | reserved for future registration |
| 16384–32767 | private use by a closed application; a general reader MUST fail with `E_UNKNOWN_DICTIONARY` |
| ≥ 32768 | reserved |

Each registry entry has these fields:

| Field | Meaning |
|---|---|
| ID | the dictionary ID |
| Kind | "brotli prefix" (usable with codec 5) or "short-text model" (usable with codec 3, format of 6.8.4) |
| Size | exact length in bytes |
| SHA-256 | of the exact bytes, lowercase hexadecimal |
| Sources and licence | each source of the data and its licence |
| Attribution | the NOTICE text, when a source requires attribution |
| Added in | the specification version that registered it |

Rules:

1. A published ID is immutable forever: its bytes never change, and a retired ID is never reused. A change is a new ID.
2. The bytes of every registered dictionary are a normative annex of this specification.
3. A reader MUST verify the SHA-256 of each dictionary it carries, at build time or at first use. A reader MUST fail with `E_UNKNOWN_DICTIONARY` for an ID it does not carry and MUST NOT guess.
4. A reader states the specification version whose registry it implements, and carries every dictionary registered up to it.
5. Budget: the dictionaries registered for format version 0, together with the token table of 6.8.2, total at most 65,536 bytes. The RFC 7932 built-in dictionary is outside this budget.
6. Source data MUST be one of: public domain (including works that copyright law leaves unprotected, such as statutes and court decisions); CC0; KOGL Type 0; CC BY 4.0, with the attribution in NOTICE; factual lists without copyright protection (such as the IANA root zone TLD names); or text written for this project. Data under ShareAlike, NonCommercial or NoDerivatives terms, or under a copyleft licence, MUST NOT be used.

Registry contents in this version:

| ID | Kind | Size | SHA-256 | Sources and licence | Added in |
|---:|---|---:|---|---|---|
| 0 | none | 0 | — | — | 0.1 |

The first entries (a brotli prefix dictionary for URLs and short text, a Korean short-text model) are built and measured in stage 7 of the project plan and registered before 1.0.

## 6.12 Worked examples

All values were computed by a script and checked by decoding them back. The QR figures are for QR versions 1–9 (count field 10 bits numeric, 9 alphanumeric, 8 byte) and use the cheapest segmentation of the exact same string; they count QR's 4-bit mode indicator and count field but not its terminator or padding. The NMT figures are the coded field `C` only. The container (03-container-and-records.md) adds the content length, codec ID and dictionary ID, which do the job of QR's mode indicator and count field.

### 6.12.1 Codec 1: `0123456789`

| Group | Value | Field |
|---|---:|---|
| `012` | 12 | `0000001100` (10 bits) |
| `345` | 345 | `0101011001` (10 bits) |
| `678` | 678 | `1010100110` (10 bits) |
| `9` | 9 | `1001` (4 bits) |

`B` = 34 bits, 6 padding bits. `C` = `03 15 9A 9A 40` (5 bytes, 40 bits). QR numeric: 4 + 10 + 34 = 48 bits.

### 6.12.2 Codec 2: `HTTPS://EXAMPLE.COM/ABC`

23 characters: 11 pairs of 11 bits and one 6-bit tail (`C` → 12). `B` = 127 bits, 1 padding bit.

Fields: `01100011010 10100110010 10100011000 11110111010 01010010111 00111011000 10001111010 01010100000 01000110100 10000001001 00111001101 001100`.

`C` = `63 54 CA 8C 7B A5 2E 76 23 D2 A0 46 90 24 E6 98` (16 bytes, 128 bits). QR alphanumeric: 4 + 9 + 127 = 140 bits.

Codec 3 with model 0 also gives 16 bytes; by the tie rule of 6.3 the encoder picks codec 2.

### 6.12.3 Codec 3: `https://github.com/needmoretruth/nmtcode`

40 bytes. Reference parse (6.8.6), model 0 (`d` = 0), 24 symbols:

| # | Symbol | Meaning | `cum` | `f` |
|---:|---:|---|---:|---:|
| 1 | 258 | token 2 `"https://"` | 56968 | 36 |
| 2 | 300 | token 44 `"github.com"` | 58480 | 36 |
| 3 | 47 | `/` | 4696 | 864 |
| 4–16 | | `n e e d m o r e t r u t h`, each a literal with `f` = 1300 | see 6.8.3 | 1300 |
| 17 | 47 | `/` | 4696 | 864 |
| 18–24 | | `n m t c o d e`, literals | | 1300 |

`cum` for a lowercase letter `x` is 21840 + 1300·(`x` − `a`) (for example `n` = 38740, `e` = 27040).

The encoder shifts 18 times (`M` = 22) and the shortest value in the final interval needs `m` = 19 bytes:

`C` = `DE A7 40 BA 96 DC EE AA EE 6B EF DF 35 76 47 AE BA F7 91` (19 bytes, 152 bits; the model's information content for this parse is 147.3 bits).

QR: the whole string in byte mode is 4 + 8 + 320 = 332 bits (0.46× for NMT). A QR encoder that upper-cases the scheme and host — a different string with the same meaning under RFC 3986 — reaches 298 bits (`HTTPS://GITHUB.COM/` alphanumeric plus the path in byte mode).

### 6.12.4 Codec 4: `안녕하세요 NMT Code`

24 bytes of UTF-8 (5 syllables, a space, 8 ASCII characters).

| Unit | Bits | Meaning |
|---|---|---|
| mode | `0` | start in H mode |
| 6472 | `01100101001000` | 안 (U+C548) |
| 1365 | `00010101010101` | 녕 (U+B155) |
| 10584 | `10100101011000` | 하 (U+D558) |
| 5432 | `01010100111000` | 세 (U+C138) |
| 11415 | `10110010010111` | 요 (U+C694, `TIdx` 0 = `F[0]`, `LV` 243) followed by a space |
| 16362 | `11111111101010` | `SWITCH_A` |
| 0x4E 0x4D 0x54 0x20 0x43 0x6F 0x64 0x65 | 8 × 7 bits | `N M T` space `C o d e` |

141 bits, 3 padding bits. `C` = `32 90 2A AD 2B 0A 9C 59 2F FF 54 E9 B5 10 43 DF 93 28` (18 bytes, 144 bits).

QR: byte mode with UTF-8 is 4 + 8 + 192 = 204 bits (216 with a UTF-8 ECI designator), so NMT is 0.71×. EUC-KR through ECI is 12 + 4 + 8 + 152 = 176 bits (0.82×).

### 6.12.5 Candidate sizes and the selection rule

`N` in bytes for each codec on the four strings. The brotli column is informative: it comes from one brotli implementation at quality 11, and other encoders give other sizes.

| Content | 0 stored | 1 | 2 | 3 (model 0) | 4 | 5 (brotli) | Chosen |
|---|---:|---:|---:|---:|---:|---:|---|
| `0123456789` | 10 | 5 | 7 | 9 | 9 | 14 | 1 |
| `HTTPS://EXAMPLE.COM/ABC` | 23 | — | 16 | 16 | 21 | 27 | 2 (tie with 3) |
| `https://github.com/needmoretruth/nmtcode` | 40 | — | — | 19 | 36 | 40 | 3 |
| `안녕하세요 NMT Code` | 24 | — | — | 35 | 18 | 28 | 4 |

## 6.13 Parameters tunable in 0.x

Each parameter below has the concrete value given in this chapter and is fixed before 1.0. A change in a 0.x revision changes the decoded output of existing symbols that use it, so 09-versions-and-registries.md records it.

| Parameter | Section | Value in 0.1 |
|---|---|---|
| `TOKEN_TABLE_V0` | 6.8.2 | 240 tokens, serialisation SHA-256 `a8e0f1be…3e3717` |
| `MODEL0_WEIGHTS` | 6.8.3 | the class table |
| `HANGUL_SPACE_FINALS` | 6.9.2 | [0, 1, 4, 7, 8, 16, 17, 19, 20, 21, 23, 26] |
