//! Helpers shared by the integration tests.

#![allow(dead_code)]

use nmtcode_payload::{codec3, leb128};

/// One vector of a short-text model: explicit (symbol, frequency) entries in
/// increasing symbol order; every other symbol has frequency 1.
pub type Entries = Vec<(u16, u32)>;

/// Serialises a short-text model dictionary in the byte format of 6.8.4.
pub fn model_bytes(order: u8, hash_bits: u8, slotmap: &[u16], vectors: &[Entries]) -> Vec<u8> {
    let mut out = vec![0, order, hash_bits];
    out.extend_from_slice(&u16::try_from(vectors.len()).unwrap().to_be_bytes());
    out.extend_from_slice(&496u16.to_be_bytes());
    for &slot in slotmap {
        out.extend_from_slice(&slot.to_be_bytes());
    }
    for vector in vectors {
        leb128::write(u32::try_from(vector.len()).unwrap(), &mut out);
        let mut previous: Option<u16> = None;
        for &(symbol, frequency) in vector {
            let gap = match previous {
                None => symbol,
                Some(p) => symbol - p - 1,
            };
            leb128::write(u32::from(gap), &mut out);
            leb128::write(frequency - 2, &mut out);
            previous = Some(symbol);
        }
    }
    out
}

/// Model 0 written as an order-0 dictionary: every symbol explicit.
pub fn model0_as_dictionary() -> Vec<u8> {
    let entries: Entries =
        (0u16..496).map(|s| (s, codec3::model0_frequency(usize::from(s)))).collect();
    model_bytes(0, 0, &[0], &[entries])
}

/// A vector that gives `boost` symbols a high frequency and leaves the rest at
/// 1, with the remainder on symbol `sink`, summing to 2^16.
pub fn skewed_vector(boost: &[u16], sink: u16) -> Entries {
    let mut symbols: Vec<u16> = boost.iter().copied().filter(|&s| s != sink).collect();
    symbols.push(sink);
    symbols.sort_unstable();
    symbols.dedup();
    let others = 496 - u32::try_from(symbols.len()).unwrap();
    let boosted = 1000u32;
    let sink_frequency = 65_536 - others - boosted * (u32::try_from(symbols.len()).unwrap() - 1);
    symbols.into_iter().map(|s| (s, if s == sink { sink_frequency } else { boosted })).collect()
}

/// An order-2 model with 2^6 slots and three vectors.
pub fn order2_model() -> Vec<u8> {
    let lower: Vec<u16> = (u16::from(b'a')..=u16::from(b'z')).collect();
    let digits: Vec<u16> = (u16::from(b'0')..=u16::from(b'9')).collect();
    let tokens: Vec<u16> = (256..496).step_by(7).collect();
    let vectors = vec![
        skewed_vector(&lower, u16::from(b'e')),
        skewed_vector(&digits, u16::from(b'0')),
        skewed_vector(&tokens, u16::from(b'/')),
    ];
    let slotmap: Vec<u16> = (0..64u16).map(|i| i % 3).collect();
    model_bytes(2, 6, &slotmap, &vectors)
}
