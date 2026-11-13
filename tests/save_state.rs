//! Save states (the `serde` feature; not in ym2413-python, whose state is plain attributes a
//! board copies): a chip serialised half way through a stream and restored carries on exactly as
//! the one that was never stopped, for both variants and for a variant that is neither.

#![cfg(feature = "serde")]

mod common;

use common::*;
use ym2413::{OPLLVariant, DS1001, OPLL, YM2413};

/// Writes to every register, with sound: the music of `scripts/lockstep_gen.py` in miniature.
fn stream(seed: u64) -> Vec<(u64, u8, u8)> {
    let mut rng = PyRandom::new(seed);
    let mut out = Vec::new();
    for _ in 0..1500 {
        out.push((rng.randrange(0, 40) as u64, 0, rng.randrange(0, 0x40) as u8));
        out.push((
            rng.randrange(0, 3000) as u64,
            1,
            rng.randrange(0, 256) as u8,
        ));
    }
    out
}

#[test]
fn a_restored_chip_continues_identically() {
    let mut odd = YM2413;
    odd.name = "neither".into();
    odd.snare_last_byte = 0x48;
    odd.channels = 7;
    let variants: [OPLLVariant; 3] = [YM2413, DS1001, odd];
    for variant in variants {
        for seed in 0..4 {
            let writes = stream(seed);
            let (first, second) = writes.split_at(writes.len() / 2);
            let mut chip = OPLL::new(variant.clone());
            chip.trace = Some(Vec::new());
            for &(clocks, port, value) in first {
                chip.run(clocks);
                chip.write(port, value);
            }
            chip.run(37); // inside a slot
            let saved = bincode::serialize(&chip).unwrap();
            let mut restored: OPLL = bincode::deserialize(&saved).unwrap();
            assert_eq!(restored, chip);
            for each in [&mut chip, &mut restored] {
                for &(clocks, port, value) in second {
                    each.run(clocks);
                    each.write(port, value);
                }
                each.run(SAMPLE * 500);
            }
            assert!(chip.melody.iter().any(|&v| v != 0));
            assert_eq!(restored, chip);
            assert_eq!(restored.take_outputs(), chip.take_outputs());
        }
    }
}
