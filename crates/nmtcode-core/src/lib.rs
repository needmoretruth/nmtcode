//! Common types of NMT Code: module grid, format word, container and records, CRC-32C and the reader errors.

#![no_std]

extern crate alloc;

mod grid;

pub use grid::{GridTextError, MAX_SIDE, MIN_SIDE, ModuleGrid, is_valid_side};
