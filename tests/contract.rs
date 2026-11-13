//! Rung 0: the contract with the boards (ym2413-python's docs/api.md, docs/validation.md).
//!
//! Not a judge of the chip: these keep the core usable by sms-python and
//! famicom-python as api.md describes it.
//!
//! A transcription of `tests/test_contract.py` (at f88dccf). Three tests are about Python
//! objects (a frozen dataclass, plain attributes, PyPy's 80-attribute limit); each keeps its
//! place with the Rust counterpart of what it guards, said in its comment.

mod common;

use common::*;
use ym2413::{OPLLVariant, CLOCKS_PER_CYCLE, CYCLES_PER_SAMPLE, DS1001, OPLL, YM2413};

const BOTH: [OPLLVariant; 2] = [YM2413, DS1001];

/// (input clocks to run first, port, byte), aimed at registers that exist.
fn random_stream(seed: u64, writes: usize) -> Vec<(u64, u8, u8)> {
    let mut rng = PyRandom::new(seed);
    let mut registers: Vec<u8> = (0x00..0x08).collect();
    registers.extend([0x0E, 0x0F]);
    registers.extend(0x10..0x19);
    registers.extend(0x20..0x29);
    registers.extend(0x30..0x39);
    let mut stream = Vec::new();
    for _ in 0..writes {
        stream.push((rng.randrange(0, 40) as u64, 0, rng.choice(&registers)));
        stream.push((
            rng.randrange(0, 2500) as u64,
            1,
            rng.randrange(0, 256) as u8,
        ));
    }
    stream
}

fn play(chip: &mut OPLL, stream: &[(u64, u8, u8)]) {
    for &(clocks, port, value) in stream {
        chip.run(clocks);
        chip.write(port, value);
    }
}

#[test]
fn the_helper_draws_what_pythons_random_draws() {
    // Not in the reference: the streams below are Python's only if PyRandom is random.Random.
    // The values were printed by PyPy 3.11's random module.
    let mut rng = PyRandom::new(5);
    let a: Vec<i64> = (0..5).map(|_| rng.randrange(0, 40)).collect();
    let b: Vec<i32> = (0..5).map(|_| rng.choice(&[1, 2, 3, 4, 5, 6, 7])).collect();
    assert_eq!(a, [39, 16, 22, 33, 1]);
    assert_eq!(b, [7, 4, 7, 2, 6]);
    assert_eq!(rng.random(), 0.05185325369909766);
    assert_eq!(rng.randrange(0, 2500), 642);
    let mut rng = PyRandom::new((1 << 40) + 7);
    let c: Vec<i64> = (0..5).map(|_| rng.randrange(0, 256)).collect();
    assert_eq!(c, [243, 128, 64, 192, 185]);
    let mut rng = PyRandom::new(0);
    let d: Vec<i64> = (0..5).map(|_| rng.randrange(0, 256)).collect();
    assert_eq!(d, [197, 215, 20, 132, 248]);
}

#[test]
fn test_the_names_api_md_lists_are_the_public_surface() {
    // Python lists `__all__`; here the six names are re-exported at the crate root (the `use`
    // above) and the five methods exist with the reference's shape.
    let _: fn(&mut OPLL, u8, u8) = OPLL::write;
    let _: fn(&mut OPLL, u64) = OPLL::run;
    let _: fn(&mut OPLL) -> Vec<i32> = OPLL::take_samples;
    let _: fn(&mut OPLL) -> Vec<(i32, i32)> = OPLL::take_outputs;
    let _: fn(&mut OPLL) = OPLL::reset;
}

#[test]
fn test_the_time_constants() {
    // api.md: the output rate is the clock divided by 72. Spec 2.2, 2.3.
    assert_eq!(CLOCKS_PER_CYCLE, 4);
    assert_eq!(CYCLES_PER_SAMPLE, 18);
}

#[test]
fn test_a_variant_is_a_frozen_record_of_plain_values() {
    // Python: a frozen dataclass whose fields are ints, strings, bools and tuples of them.
    // Rust: `YM2413` is a `const`, so changing a copy leaves it as it was, and the fields are
    // plain values by their types.
    let mut copy = YM2413;
    copy.channels = 3;
    assert_eq!(YM2413.channels, 9);
    assert_ne!(copy, YM2413);
}

#[test]
fn test_one_sample_per_72_clocks() {
    for variant in BOTH {
        let mut chip = OPLL::new(variant);
        chip.run(72 * 1000 + 71);
        assert_eq!(chip.take_samples().len(), 1000);
        chip.run(1);
        assert_eq!(chip.take_outputs().len(), 1);
    }
}

#[test]
fn test_take_samples_is_the_sum_of_take_outputs() {
    for variant in BOTH {
        let (mut a, mut b) = (OPLL::new(variant.clone()), OPLL::new(variant));
        let stream = random_stream(5, 400);
        play(&mut a, &stream);
        play(&mut b, &stream);
        let pairs = a.take_outputs();
        assert!(!pairs.is_empty() && pairs.iter().any(|&pair| pair != (0, 0)));
        let sums: Vec<i32> = pairs
            .iter()
            .map(|(melody, rhythm)| melody + rhythm)
            .collect();
        assert_eq!(b.take_samples(), sums);
        assert!(a.take_outputs().is_empty() && b.take_samples().is_empty());
    }
}

#[test]
fn test_run_a_then_b_equals_run_a_plus_b() {
    // validation.md, rung 0: for any split, including splits inside a slot.
    for variant in BOTH {
        for seed in 0..4 {
            let stream = random_stream(seed, 400);
            let (mut whole, mut pieces) = (OPLL::new(variant.clone()), OPLL::new(variant.clone()));
            let mut rng = PyRandom::new(100 + seed);
            for &(clocks, port, value) in &stream {
                whole.run(clocks);
                let mut left = clocks;
                while left != 0 {
                    let piece = left.min(rng.choice(&[1, 1, 2, 3, 5, 7, 72, 500]));
                    pieces.run(piece);
                    left -= piece;
                }
                whole.write(port, value);
                pieces.write(port, value);
            }
            assert_eq!(whole.take_outputs(), pieces.take_outputs());
            assert_eq!(whole, pieces);
        }
    }
}

/// What a save-state walker does: copy every attribute but the configuration. Here every
/// field is a plain value, so the walker is `clone_from` onto a chip of the same variant.
fn copy_state(source: &OPLL, target: &mut OPLL) {
    assert_eq!(source.variant, target.variant);
    target.clone_from(source);
}

#[test]
fn test_state_is_plain_and_a_copy_continues_identically() {
    // Python also checks each attribute's type (ints, lists of ints, a bytearray); the Rust
    // fields are those types by declaration.
    for variant in BOTH {
        let stream = random_stream(9, 400);
        let mut chip = OPLL::new(variant.clone());
        play(&mut chip, &stream[..400]);
        chip.take_outputs();
        let mut twin = OPLL::new(variant);
        copy_state(&chip, &mut twin);
        play(&mut chip, &stream[400..]);
        play(&mut twin, &stream[400..]);
        assert_eq!(chip.take_outputs(), twin.take_outputs());
        assert_eq!(chip, twin);
    }
}

#[test]
fn test_fewer_than_80_attributes() {
    // PyPy turns an instance with 80 or more attributes into a dict. Rust has no such limit;
    // what the test protects, a chip a board can copy cheaply and compare, is checked here:
    // a fresh chip equals its clone.
    for variant in BOTH {
        let chip = OPLL::new(variant);
        assert_eq!(chip.clone(), chip);
    }
}

#[test]
fn test_nothing_callable_is_kept_as_state() {
    // Python: no attribute is callable. Rust: no field is a closure or a pointer to shared
    // state; the chip is plain data that can move between threads.
    fn plain_data<T: Clone + PartialEq + Send + Sync + 'static>(_: &T) {}
    plain_data(&OPLL::new(YM2413));
}

#[test]
fn test_the_ds1001_has_no_rhythm_side_and_no_channels_6_to_8() {
    // api.md; spec 1, 11.14 (S6, tier 3); PROVISIONAL in its reach (T15).
    let mut chip = OPLL::new(DS1001);
    write(&mut chip, 0x0E, 0x3F);
    for channel in 6..9 {
        write(&mut chip, 0x30 + channel, 0x10);
        note(&mut chip, channel, 0x150, 4);
    }
    chip.run(SAMPLE * 3000);
    assert!(chip.take_outputs().iter().all(|&pair| pair == (0, 0)));
    // ...while channel 5 sounds.
    write(&mut chip, 0x35, 0x10);
    note(&mut chip, 5, 0x150, 4);
    chip.run(SAMPLE * 3000);
    let pairs = chip.take_outputs();
    assert!(pairs.iter().map(|(melody, _)| melody.abs()).max().unwrap() > 100);
    assert!(pairs.iter().all(|&(_, rhythm)| rhythm == 0));
}

#[test]
fn test_reset_keeps_the_place_in_the_sample() {
    // CHOICE: reset() does not move the chip's place in its 72-clock cycle, so a
    // board always gets one sample per 72 clocks.
    let mut chip = plain(&PLAIN_SINE);
    chip.run(100);
    chip.reset();
    chip.run(72 * 10 - 100);
    assert_eq!(chip.take_samples().len(), 10);
}

#[test]
fn test_a_sample_is_within_the_range_the_words_allow() {
    // T6: nine melody words and ten percussion words of at most 255 each.
    let mut chip = OPLL::new(YM2413);
    play(&mut chip, &random_stream(3, 400));
    for (melody_sum, rhythm_sum) in chip.take_outputs() {
        assert!(melody_sum.abs() <= 9 * 255 && rhythm_sum.abs() <= 10 * 255);
    }
}

#[test]
fn test_melody_helper_sees_a_note() {
    assert_eq!(
        *melody(&mut plain(&PLAIN_SINE), 600).iter().max().unwrap(),
        255
    );
}
