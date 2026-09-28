//! Property tests: random symbols, module sizes, quiet zones, bootstrap and resampling.

mod common;

use common::*;
use nmtcode_detect::{decode_png, find_symbols, read_png};
use nmtcode_render::{BootstrapLevel, BootstrapSide, RenderOptions, render_png};
use proptest::prelude::*;

fn side() -> impl Strategy<Value = u32> {
    (5u32..=24).prop_map(|k| 4 * k)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, .. ProptestConfig::default() })]

    #[test]
    fn integer_module_sizes(
        w in side(),
        h in side(),
        seed in any::<u64>(),
        module_px in 2u32..=6,
        quiet_zone in 2u32..=6,
        bootstrap in any::<bool>(),
        level in prop_oneof![Just(BootstrapLevel::M), Just(BootstrapLevel::Q), Just(BootstrapLevel::H)],
        forced_side in prop_oneof![Just(None), Just(Some(BootstrapSide::Left)), Just(Some(BootstrapSide::Above))],
    ) {
        let grid = make_symbol(w, h, seed);
        let options = RenderOptions {
            module_px,
            quiet_zone,
            bootstrap,
            bootstrap_level: level,
            bootstrap_side: forced_side,
            ..RenderOptions::default()
        };
        let found = read_png(&render_png(&grid, &options).unwrap()).unwrap();
        prop_assert_eq!(found, vec![grid]);
    }

    #[test]
    fn resampled(
        w in side(),
        h in side(),
        seed in any::<u64>(),
        base in 2u32..=6,
        target in 2.5f64..8.0,
        bootstrap in any::<bool>(),
    ) {
        let grid = make_symbol(w, h, seed);
        let options = RenderOptions { module_px: base, bootstrap, ..RenderOptions::default() };
        let image = decode_png(&render_png(&grid, &options).unwrap()).unwrap();
        let factor = target / f64::from(base);
        let found = find_symbols(&resample(&image, factor));
        prop_assert_eq!(found, vec![grid]);
    }
}
