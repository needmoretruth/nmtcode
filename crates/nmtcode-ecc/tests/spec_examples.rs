//! Exact-value tests from chapter 4 of the specification: the smallest symbols (4.6), the
//! sample splits (4.7), the worked examples (4.11.1, 4.11.2) and the chapter 4 values of the
//! end-to-end symbol of Annex A (A.2.3, A.2.4).

use nmtcode_ecc::{BlockSplit, EccError, encode_stream, gf, rs, split};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|pair| u8::from_str_radix(pair, 16).unwrap()).collect()
}

/// Block lengths in block order, and parity lengths, of a split.
fn lengths(split: &BlockSplit) -> (Vec<usize>, Vec<usize>) {
    split.blocks().map(|b| (b.len, b.parity_len)).unzip()
}

/// The parity share (N − K) / N in tenths of a percent, rounded half to even, as the tables of
/// 4.7 print it (for example 31.25% is printed 31.2%).
fn share_permille(split: &BlockSplit) -> usize {
    let numerator = 1000 * split.parity_total();
    let denominator = split.codewords();
    let (quotient, remainder) = (numerator / denominator, numerator % denominator);
    let round_up =
        2 * remainder > denominator || (2 * remainder == denominator && quotient % 2 == 1);
    quotient + usize::from(round_up)
}

#[test]
fn smallest_symbol_at_each_level() {
    // 4.6, table "Smallest symbol at each level": N = 20 at every level.
    // The last column, "largest content of one record, codec 0", is K minus the 7-byte
    // smallest container of chapter 3 (3.2).
    let expected = [(0u8, 4usize, 16usize, 9usize), (1, 6, 14, 7), (2, 10, 10, 3), (3, 12, 8, 1)];
    for (level, parity, capacity, largest_content) in expected {
        let s = split(20, level).unwrap();
        assert_eq!(s.capacity() - 7, largest_content, "level {level}");
        assert_eq!(s.block_count(), 1, "level {level}");
        assert_eq!(s.parity_per_block(), parity, "level {level}");
        assert_eq!(s.parity_total(), parity, "level {level}");
        assert_eq!(s.capacity(), capacity, "level {level}");
        assert_eq!(lengths(&s), (vec![20], vec![parity]), "level {level}");
    }
}

#[test]
fn sample_splits_of_4_7() {
    // Every row of the table of 4.7:
    // (N, level, B, [(count, length)], P, errors per block, K, actual share in tenths of %).
    type Row = (usize, u8, usize, &'static [(usize, usize)], usize, usize, usize, usize);
    let rows: [Row; 28] = [
        (32, 0, 1, &[(1, 32)], 6, 3, 26, 188),
        (32, 1, 1, &[(1, 32)], 10, 5, 22, 312),
        (32, 2, 1, &[(1, 32)], 16, 8, 16, 500),
        (32, 3, 1, &[(1, 32)], 20, 10, 12, 625),
        (50, 0, 1, &[(1, 50)], 8, 4, 42, 160),
        (50, 1, 1, &[(1, 50)], 16, 8, 34, 320),
        (50, 2, 1, &[(1, 50)], 26, 13, 24, 520),
        (50, 3, 1, &[(1, 50)], 30, 15, 20, 600),
        (64, 0, 1, &[(1, 64)], 10, 5, 54, 156),
        (64, 1, 1, &[(1, 64)], 20, 10, 44, 312),
        (64, 2, 1, &[(1, 64)], 32, 16, 32, 500),
        (64, 3, 1, &[(1, 64)], 40, 20, 24, 625),
        (128, 0, 1, &[(1, 128)], 20, 10, 108, 156),
        (128, 1, 1, &[(1, 128)], 40, 20, 88, 312),
        (128, 2, 1, &[(1, 128)], 64, 32, 64, 500),
        (128, 3, 1, &[(1, 128)], 78, 39, 50, 609),
        (512, 0, 3, &[(2, 171), (1, 170)], 26, 13, 434, 152),
        (512, 1, 3, &[(2, 171), (1, 170)], 52, 26, 356, 305),
        (512, 2, 3, &[(2, 171), (1, 170)], 86, 43, 254, 504),
        (512, 3, 3, &[(2, 171), (1, 170)], 104, 52, 200, 609),
        (1311, 0, 6, &[(3, 219), (3, 218)], 34, 17, 1107, 156),
        (1311, 1, 6, &[(3, 219), (3, 218)], 66, 33, 915, 302),
        (1311, 2, 6, &[(3, 219), (3, 218)], 110, 55, 651, 503),
        (1311, 3, 6, &[(3, 219), (3, 218)], 132, 66, 519, 604),
        (13000, 0, 51, &[(46, 255), (5, 254)], 40, 20, 10960, 157),
        (13000, 1, 51, &[(46, 255), (5, 254)], 78, 39, 9022, 306),
        (13000, 2, 51, &[(46, 255), (5, 254)], 128, 64, 6472, 502),
        (13000, 3, 51, &[(46, 255), (5, 254)], 154, 77, 5146, 604),
    ];
    for (n, level, blocks, runs, parity, errors, capacity, share) in rows {
        let s = split(n, level).unwrap();
        let expected_lengths: Vec<usize> =
            runs.iter().flat_map(|&(count, len)| core::iter::repeat_n(len, count)).collect();
        let (got_lengths, got_parity) = lengths(&s);
        assert_eq!(s.codewords(), n);
        assert_eq!(s.level(), level);
        assert_eq!(s.block_count(), blocks, "N = {n}, level {level}");
        assert_eq!(got_lengths, expected_lengths, "N = {n}, level {level}");
        assert_eq!(got_parity, vec![parity; blocks], "N = {n}, level {level}");
        assert_eq!(s.parity_per_block(), parity, "N = {n}, level {level}");
        assert_eq!(s.parity_per_block() / 2, errors, "N = {n}, level {level}");
        assert_eq!(s.capacity(), capacity, "N = {n}, level {level}");
        assert_eq!(s.parity_total(), blocks * parity, "N = {n}, level {level}");
        assert_eq!(share_permille(&s), share, "N = {n}, level {level}");
    }
}

#[test]
fn rounding_example_of_4_6() {
    // 4.6: "18.8% instead of 15% at N = 32, level 0".
    let s = split(32, 0).unwrap();
    assert_eq!(share_permille(&s), 188);
}

#[test]
fn minimum_codewords_and_levels() {
    // 4.6: N_min = [3, 3, 3, 5]; a layer below it is rejected, and the level is 0 to 3.
    for level in 0..=2u8 {
        assert_eq!(split(2, level), Err(EccError::LayerTooSmall));
        assert_eq!(split(0, level), Err(EccError::LayerTooSmall));
        let s = split(3, level).unwrap();
        assert_eq!((s.parity_per_block(), s.capacity()), (2, 1), "level {level}");
    }
    assert_eq!(split(4, 3), Err(EccError::LayerTooSmall));
    // 4.6: "At level 3, N = 3 would leave one message byte but N = 4 would leave none (P = 4),
    // so the minimum is set at 5". N = 3 is still rejected by the minimum.
    assert_eq!(split(3, 3), Err(EccError::LayerTooSmall));
    let s = split(5, 3).unwrap();
    assert_eq!((s.parity_per_block(), s.capacity()), (4, 1));
    for level in [4u8, 5, 255] {
        assert_eq!(split(20, level), Err(EccError::InvalidLevel));
    }
}

#[test]
fn properties_of_4_6_hold_for_every_n() {
    // The property list of 4.6, checked for every N from N_min to 20 000 at every level.
    let q = [15usize, 30, 50, 60];
    for level in 0..=3u8 {
        let minimum = if level == 3 { 5 } else { 3 };
        for n in minimum..=20_000usize {
            let s = split(n, level).unwrap();
            let (lens, parities) = lengths(&s);
            let parity = s.parity_per_block();
            assert_eq!(lens.len(), s.block_count());
            assert_eq!(s.block_count(), n.div_ceil(255));
            assert!(lens.iter().all(|&len| len <= 255));
            let (min, max) = (*lens.iter().min().unwrap(), *lens.iter().max().unwrap());
            assert!(max - min <= 1);
            assert_eq!(max, s.max_block_len());
            assert!(lens.windows(2).all(|w| w[0] >= w[1]), "long blocks first");
            assert_eq!(lens.iter().sum::<usize>(), n);
            assert!(parities.iter().all(|&p| p == parity));
            assert!(parity >= 2 && parity.is_multiple_of(2));
            assert!(lens.iter().all(|&len| parity * 100 >= len * q[usize::from(level)]));
            assert!(lens.iter().all(|&len| len > parity), "every block has a message byte");
            assert_eq!(s.capacity(), n - s.block_count() * parity);
            let starts: Vec<usize> = s.blocks().map(|b| b.message_start).collect();
            let mut offset = 0;
            for (block, start) in s.blocks().zip(starts) {
                assert_eq!(start, offset);
                assert_eq!(block.message_len, block.len - parity);
                offset += block.message_len;
            }
            assert_eq!(offset, s.capacity());
            if s.block_count() >= 2 {
                assert!(min >= 128);
                assert!(parity <= 154);
                assert!(min - parity >= 50);
            }
        }
    }
}

#[test]
fn generator_g16_of_4_11_1() {
    let expected = hex("01 3B 0D 68 BD 44 D1 1E 08 A3 41 29 E5 62 32 24 3B");
    let generator = rs::generator(16).unwrap();
    assert_eq!(generator, expected);
    let powers = [0u8, 120, 104, 107, 109, 102, 161, 76, 3, 91, 191, 147, 169, 182, 194, 225, 120];
    let logs: Vec<u8> = generator.iter().map(|&c| gf::log(c).unwrap()).collect();
    assert_eq!(logs, powers);
    assert_eq!(rs::Encoder::new(16).unwrap().generator(), expected.as_slice());
}

#[test]
fn block_of_4_11_1() {
    let s = split(50, 1).unwrap();
    assert_eq!(s.block_count(), 1);
    assert_eq!((s.short_block_len(), s.long_block_count()), (50, 0));
    assert_eq!(s.max_block_len(), 50);
    assert_eq!(s.parity_per_block(), 16);
    let block = s.block(0).unwrap();
    assert_eq!((block.len, block.message_len), (50, 34));
    assert_eq!(s.capacity(), 34);
    assert_eq!(share_permille(&s), 320);

    let message: Vec<u8> = (0..34u8).collect();
    let parity = hex("36 5A E8 16 6F 40 AC F0 72 10 3F 0F BB 20 DD F4");
    assert_eq!(rs::Encoder::new(16).unwrap().parity(&message).unwrap(), parity);

    let stream = encode_stream(&s, &message).unwrap();
    let mut expected = message.clone();
    expected.extend_from_slice(&parity);
    assert_eq!(stream, expected);
    assert_eq!(rs::encode(&message, 16).unwrap(), expected);
    assert_eq!(rs::syndromes(&stream, 16).unwrap(), vec![0u8; 16]);
}

#[test]
fn stream_order_of_4_11_2() {
    let s = split(512, 1).unwrap();
    assert_eq!(s.block_count(), 3);
    assert_eq!((s.short_block_len(), s.long_block_count()), (170, 2));
    assert_eq!(lengths(&s).0, vec![171, 171, 170]);
    assert_eq!(s.parity_per_block(), 52);
    let ranges: Vec<(usize, usize)> =
        s.blocks().map(|b| (b.message_start, b.message_start + b.message_len - 1)).collect();
    assert_eq!(ranges, vec![(0, 118), (119, 237), (238, 355)]);
    assert_eq!(s.capacity(), 356);

    let table = [(0, 0, 0), (1, 1, 0), (2, 2, 0), (3, 0, 1), (509, 2, 169), (510, 0, 170)];
    for (index, block, byte) in table.into_iter().chain([(511, 1, 170)]) {
        assert_eq!(s.locate(index), Some((block, byte)), "c[{index}]");
        assert_eq!(s.stream_index(block, byte), Some(index), "c[{index}]");
    }
    assert_eq!(s.locate(512), None);
    assert_eq!(s.stream_index(2, 170), None, "byte 170 exists only in the long blocks");
    assert_eq!(s.stream_index(3, 0), None);

    // The same order seen through encode_stream: each entry equals the byte of the block
    // encoded on its own by 4.4.
    let message: Vec<u8> = (0..356u32).map(|i| u8::try_from(i % 251).unwrap()).collect();
    let stream = encode_stream(&s, &message).unwrap();
    let blocks: Vec<Vec<u8>> = s
        .blocks()
        .map(|b| {
            let part = &message[b.message_start..b.message_start + b.message_len];
            rs::encode(part, 52).unwrap()
        })
        .collect();
    for (index, block, byte) in table.into_iter().chain([(511, 1, 170)]) {
        assert_eq!(stream[index], blocks[block][byte], "c[{index}]");
    }
}

#[test]
fn chapter_4_values_of_annex_a_2() {
    // A.2.3: N = 42 at level 0 gives B = 1, n = 42, P = 8, K = 34.
    let s = split(42, 0).unwrap();
    assert_eq!((s.block_count(), s.parity_per_block(), s.capacity()), (1, 8, 34));
    // A.2.4: message, parity and codeword stream.
    let message = hex("03 16 00 28 02 DE A7 40 BA 96 DC EE AA EE 6B EF \
         DF 35 76 47 AE BA F7 91 33 56 F1 6E EC 11 EC 11 EC 11");
    let parity = hex("5F BF C3 A7 FC 87 2F A6");
    let stream = encode_stream(&s, &message).unwrap();
    assert_eq!(&stream[..34], message.as_slice());
    assert_eq!(&stream[34..], parity.as_slice());
}
