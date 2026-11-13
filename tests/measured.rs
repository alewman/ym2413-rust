//! Rung 2a: what was measured on a real YM2413 (S4), as its write-ups state it.
//!
//! S4 is the openMSX YM2413 reverse-engineering project: write-ups by Wouter
//! Vermaelen in the repository andete/ym2413 (commit 4893d1c), of measurements made
//! by driving one real chip from a micro-controller and sampling its two output
//! pins. Each test names the write-up its numbers were typed from. The numbers come
//! from the write-ups' prose and from their tables of measurements; their program
//! fragments were not read (docs/provenance-audit.md). "measured" throughout.
//!
//! Nothing here reads S4's capture files: that is rung 2b (tests/test_captures.py).
//!
//! A transcription of `tests/test_measured.py` (at f88dccf); parametrized cases are loops.
//! Python's slices clamp at the ends and its negative indices count from the end; the helpers
//! `slice` and `middle` keep those meanings where a test relies on them.

// test_the_worked_example_at_rate_13_3 checks S4's arithmetic, a constant, as Python does.
#![allow(clippy::assertions_on_constants)]

mod common;

use std::collections::BTreeSet;

use common::measured::{rhythm_chip, rhythm_chip_at, two_operator_reference};
use common::*;
use ym2413::{OPLL, YM2413};

/// Python's `values[start:stop]`, clamped to the list.
fn slice<T: Clone>(values: &[T], start: usize, stop: usize) -> Vec<T> {
    let stop = stop.min(values.len());
    let start = start.min(stop);
    values[start..stop].to_vec()
}

/// Python's `values[1:-1]`.
fn middle<T: Clone>(values: &[T]) -> Vec<T> {
    if values.len() < 2 {
        return Vec::new();
    }
    values[1..values.len() - 1].to_vec()
}

// ------------------------------------------------- the operator (spec 6)

// tab.txt: "the maximum amplitude value for each volume: 255 180(!) 127 90 63 45
// 31 22 15 11 7 5 3 2 1 1".
const PEAKS: [i32; 16] = [255, 180, 127, 90, 63, 45, 31, 22, 15, 11, 7, 5, 3, 2, 1, 1];

#[test]
fn test_peak_amplitude_for_each_volume() {
    for volume in 0..16u8 {
        let mut chip = tone_chip(
            &PLAIN_SINE,
            Tone {
                f_number: 256,
                block: 1,
                volume,
                ..Tone::default()
            },
        );
        let out = melody(&mut chip, 1024);
        assert!(
            *out.iter().max().unwrap() == PEAKS[volume as usize]
                && *out.iter().min().unwrap() == -PEAKS[volume as usize]
        );
    }
}

#[test]
fn test_peak_amplitude_for_each_sustain_level() {
    // sl-ksr.txt: "these are the exact same amplitudes as for the volume setting".
    // envelope.txt: "internal-sustain-envelope-level = 8 * SL".
    for sl in 0..16u8 {
        let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xFF, 0x00, sl << 4];
        let mut chip = pitched(&tone, 256, 1);
        chip.run(SAMPLE * 200);
        chip.take_outputs();
        assert_eq!(chip.level[C[0]], 8 * sl as i32);
        assert_eq!(
            *melody(&mut chip, 1024).iter().max().unwrap(),
            PEAKS[sl as usize]
        );
    }
}

#[test]
fn test_sustain_level_15_is_level_120_not_the_bottom() {
    // 20150320.txt: "we picked SL=15 ... the EG will only go down to -45dB";
    // "That corresponds to 120 different EG levels". Contradiction C5.
    let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xFF, 0x00, 0xF0];
    assert_eq!(*levels(&mut plain(&tone), C[0], 300).last().unwrap(), 120);
}

fn one_period_of_words(volume: u8) -> Vec<i32> {
    let mut chip = tone_chip(
        &PLAIN_SINE,
        Tone {
            f_number: 256,
            block: 1,
            volume,
            ..Tone::default()
        },
    );
    words(&mut chip, 1024)
        .iter()
        .map(|sample| sample[C[0]].0)
        .collect()
}

#[test]
fn test_a_full_volume_sine_uses_exactly_400_codes() {
    // tab.txt: "If we count the number of unique values in the column for
    // volume=0 we also get 400" (of the 512 codes; +0 and -0 are two).
    let codes: BTreeSet<i32> = one_period_of_words(0).into_iter().collect();
    assert_eq!(codes.len(), 400);
}

#[test]
fn test_the_quietest_volumes_use_four_codes() {
    // tab.txt: "Both volume=14 and 15 (only) use 4 possible values, 1, 0, -1, -2
    // (corresponding to +1, +0, -0, -1)", 14 louder on average than 15.
    for volume in [14, 15] {
        let codes: BTreeSet<i32> = one_period_of_words(volume).into_iter().collect();
        assert_eq!(codes, BTreeSet::from([1, 0, -1, -2]));
    }
}

#[test]
fn test_volume_14_is_louder_than_15_on_average() {
    let count = |volume| {
        one_period_of_words(volume)
            .into_iter()
            .filter(|&word| word == 1 || word == -2)
            .count()
    };
    assert!(count(14) > count(15));
}

#[test]
fn test_the_sine_has_1024_steps() {
    // 20150311.txt: the period divided by the flat-segment length "is always
    // exactly 1024".
    let mut chip = pitched(&PLAIN_SINE, 64, 0); // one table step in 8 samples
    let phases: Vec<i32> = attribute(&mut chip, |c| c.phase[C[0]], 8 * 1024 * 2)
        .into_iter()
        .map(|value| value >> 9)
        .collect();
    let lengths: BTreeSet<usize> = middle(&runs(&phases)).iter().map(|&(_, l)| l).collect();
    assert_eq!(lengths, BTreeSet::from([8]));
    assert_eq!(phases.iter().collect::<BTreeSet<_>>().len(), 1024);
}

#[test]
fn test_negative_values_are_the_bit_inverse_of_positive_ones() {
    // tab.txt: "this creates a 1-complements result ... two possible
    // representations for zero (+0 and -0)".
    let period = one_period_of_words(0);
    let start = (1..1024)
        .find(|&i| period[i - 1] < 0 && 0 <= period[i])
        .unwrap();
    let turned: Vec<i32> = [&period[start..], &period[..start]].concat();
    assert!((0..512).all(|i| turned[i + 512] == !turned[i]));
    assert!(period.contains(&0) && period.contains(&-1));
}

#[test]
fn test_half_sine_is_minus_zero_for_exactly_half_the_period() {
    // small-signals.txt: "the lengths of the segments with value -0 ... are
    // exactly 512 samples long (exactly half a period in this experiment)": "the
    // absolute value of the 2nd part is set to zero, but the sign still toggles".
    let mut tone = PLAIN_SINE;
    tone[3] = 0x10;
    let mut chip = pitched(&tone, 256, 1);
    let period: Vec<i32> = words(&mut chip, 2048)
        .iter()
        .map(|sample| sample[C[0]].0)
        .collect();
    assert!(runs(&period).contains(&(-1, 512)));
    assert_eq!(*period.iter().min().unwrap(), -1);
}

#[test]
fn test_mute_at_envelope_level_124_and_the_peak_widths_before_it() {
    // small-signals.txt, first experiment: one sine period of 1,024 samples, the
    // envelope one level lower every 4,096 samples. "The widths of those peaks
    // are respectively 342 and 332 samples" at levels 112 and 113; "the signal
    // stops changing at the point where we expect envelope level 124" and stays
    // at +0 ("not -0, nor alternating").
    let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00];
    let mut chip = pitched(&tone, 256, 1);
    chip.run(SAMPLE * 2048);
    for (level, width) in [(112, 342), (113, 332), (124, 0)] {
        chip.level[C[0]] = level;
        let period: Vec<i32> = words(&mut chip, 1024)
            .iter()
            .map(|sample| sample[C[0]].0)
            .collect();
        assert_eq!(period.iter().filter(|&&word| word > 0).count(), width);
        if level == 124 {
            assert!(period.iter().all(|&word| word == 0));
        }
    }
}

#[test]
fn test_total_attenuation_is_clipped_at_127() {
    // small-signals.txt, second experiment (volume 8): the total "increases by
    // one every 4096 samples until it reaches level 127 ... Beyond this point the
    // total attenuation level remains at 127 (until the envelope level reaches
    // 124)". With volume 8 (64 steps) the total passes 127 at envelope level 63.
    let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00];
    let mut chip = tone_chip(
        &tone,
        Tone {
            f_number: 256,
            block: 1,
            volume: 8,
            ..Tone::default()
        },
    );
    chip.run(SAMPLE * 2048);
    let mut shapes = Vec::new();
    for level in [62, 63, 64, 100, 123] {
        chip.level[C[0]] = level;
        shapes.push(
            words(&mut chip, 1024)
                .iter()
                .filter(|sample| sample[C[0]].0 > 0)
                .count(),
        );
    }
    assert!(
        shapes[0] > shapes[1]
            && shapes[1] == shapes[2]
            && shapes[2] == shapes[3]
            && shapes[3] == shapes[4]
            && shapes[4] > 0
    );
    assert_eq!(
        shapes[1],
        (0..512)
            .filter(|&phase| reference_value(phase, 127, false) >> 3 > 0)
            .count()
    );
}

#[test]
fn test_a_quiet_modulator_outputs_plus_zero() {
    // small-signals.txt, third experiment: "a strong indication that also the
    // modulator output is kept at a constant value +0 once the modulator
    // envelope level reaches value 124".
    let tone = [0x21, 0x21, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00];
    let mut chip = plain(&tone);
    chip.run(SAMPLE * 100);
    chip.level[M[0]] = 123;
    assert!(!attribute(&mut chip, |c| c.previous[0], 600)
        .iter()
        .all(|&v| v == 0));
    chip.level[M[0]] = 124;
    assert!(attribute(&mut chip, |c| c.previous[0], 600)[2..]
        .iter()
        .all(|&v| v == 0));
}

// ------------------------------------------- modulation and feedback (6.11-13)

#[test]
fn test_modulator_into_carrier() {
    // mod-car.txt: registers $10 = $40, block 0, both operators at half speed, so
    // "each of the 1024 entries in the sine table is repeated 16 times"; TL from 0
    // to 63; with the final model "the graph matches _exactly_ with the measured
    // data ... for all values". The depth at TL 0 is "-8pi..8pi".
    for total_level in [0u8, 8, 32, 37, 48, 63] {
        let tone = [0x20, 0x20, total_level, 0x00, 0xF0, 0xF0, 0x00, 0x00];
        let mut chip = struck(&tone, 0x40, 0);
        let got: Vec<i32> = words(&mut chip, 16 * 1024)
            .iter()
            .map(|sample| sample[C[0]].0)
            .collect();
        assert_eq!(
            got,
            two_operator_reference(total_level as i32, 0, 16 * 1024, 32)
        );
    }
}

#[test]
fn test_the_modulator_is_one_sample_out_of_step_with_the_carrier() {
    // mod-car.txt: "Instead of seeing the same value for 16 consecutive samples we
    // only see 15 samples with the same value, followed by 1 other value again
    // followed by 15 constant samples."
    let tone = [0x20, 0x20, 8, 0x00, 0xF0, 0xF0, 0x00, 0x00];
    let mut chip = struck(&tone, 0x40, 0);
    let got: Vec<i32> = words(&mut chip, 16 * 1024)
        .iter()
        .map(|sample| sample[C[0]].0)
        .collect();
    let lengths: Vec<usize> = runs(&got).iter().map(|&(_, length)| length).collect();
    let pairs = lengths
        .windows(2)
        .filter(|w| (w[0], w[1]) == (15, 1))
        .count();
    assert!(pairs > 200);
}

#[test]
fn test_every_half_period_all_modulation_depths_cross_zero_together() {
    // mod-car.txt: "every half period, the waveforms for all TL-values indeed
    // _exactly_ overlap": where the modulator passes zero the carrier is
    // unmodulated, so at every depth it changes sign on the same sample. (The
    // sine table has no exact zero, spec 6.3, so the words beside the crossing
    // still differ a little with depth.)
    let mut crossings = Vec::new();
    for total_level in [0u8, 20, 63] {
        let tone = [0x20, 0x20, total_level, 0x00, 0xF0, 0xF0, 0x00, 0x00];
        let mut chip = struck(&tone, 0x40, 0);
        let got: Vec<i32> = words(&mut chip, 16 * 1024 + 32)
            .iter()
            .map(|sample| sample[C[0]].0)
            .collect();
        crossings.push(
            [8192usize, 16384]
                .iter()
                .map(|&centre| {
                    (centre - 16..centre + 16)
                        .filter(|&i| (got[i - 1] < 0) != (got[i] < 0) && got[i - 1] != 0)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        );
    }
    let expected = vec![vec![8192], vec![16384]];
    assert!(crossings[0] == crossings[1] && crossings[1] == crossings[2]);
    assert_eq!(crossings[2], expected);
}

#[test]
fn test_feedback() {
    // fb.txt: TL 63 with FB 7, TL 37 and TL 0 with FB 0 to 7; "This model can
    // _exactly_ predict the measured data" once the two remembered outputs have
    // settled, chaos at TL 0, FB 7 included. The form is the sum of the last two
    // outputs; the amount is S1's Table III-6 (see test_manual).
    for (total_level, feedback) in [(37u8, 1u8), (37, 4), (37, 7), (0, 5), (0, 7), (63, 7)] {
        let tone = [0x20, 0x20, total_level, feedback, 0xF0, 0xF0, 0x00, 0x00];
        let mut chip = struck(&tone, 0x40, 0);
        assert!(chip.previous[0] == 0 && chip.before_previous[0] == 0); // nothing remembered yet
        let got: Vec<i32> = words(&mut chip, 16 * 1024)
            .iter()
            .map(|sample| sample[C[0]].0)
            .collect();
        assert_eq!(
            got,
            two_operator_reference(total_level as i32, feedback as i32, 16 * 1024, 32)
        );
    }
}

#[test]
fn test_feedback_0_is_no_feedback() {
    // fb.txt: "FB=0 really means _no_ feedback".
    assert_ne!(
        two_operator_reference(37, 0, 4000, 32),
        two_operator_reference(37, 1, 4000, 32)
    );
}

#[test]
fn test_each_time_the_same_note_is_played_the_wave_is_the_same() {
    // mod-car.txt: "Each time the same note is played, the wave looks exactly the
    // same" (spec 7.10: the phases are reset when the damp phase ends). Shown on
    // a carrier alone: with a sounding modulator, the modulator (held through
    // key-off, spec 8.23) damps from a low level on every new note under T14,
    // and where its damp steps fall depends on the free-running counter (U13).
    let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x0F]; // RR 15: released to 127
    let mut chip = tone_chip(
        &tone,
        Tone {
            f_number: 0x123,
            block: 3,
            key: 0,
            ..Tone::default()
        },
    );
    let mut takes = Vec::new();
    for wait in [700, 1234, 999] {
        press(&mut chip, 0, 0x123, 3);
        takes.push(melody(&mut chip, 500));
        note_k(&mut chip, 0, 0x123, 3, 0);
        chip.run(SAMPLE * wait);
        chip.take_outputs();
    }
    assert!(takes[0] == takes[1] && takes[1] == takes[2] && *takes[0].iter().max().unwrap() > 200);
}

#[test]
fn test_the_carriers_damp_end_resets_both_phases() {
    // Spec 7.10: "both of a channel's phase counters are set to zero" when the
    // envelope leaves damp, even while the modulator is still damping.
    let tone = [0x21, 0x21, 0x10, 0x00, 0xF0, 0xF0, 0x00, 0x0F];
    let mut chip = pitched(&tone, 0x123, 3);
    chip.run(SAMPLE * 300);
    note_k(&mut chip, 0, 0x123, 3, 0);
    chip.run(SAMPLE * 300);
    chip.take_outputs();
    assert!(chip.level[C[0]] == 127 && chip.level[M[0]] < 124);
    press(&mut chip, 0, 0x123, 3);
    chip.run(SAMPLE);
    chip.take_outputs();
    let step = ((0x123 * 2) << 3) >> 1;
    assert_eq!(chip.state[M[0]], 0); // the modulator is still damping
    assert!(chip.phase[C[0]] == step && chip.phase[M[0]] == step);
}

// ------------------------------------------------------- the phase (spec 7)

// 20150316.txt, first table: MULTI, block, F-number -> flat-segment length.
// None is "inf": the phase does not move.
#[rustfmt::skip]
const REPEATS: [(u8, i32, i32, Option<f64>); 30] = [
    (0, 0, 0, None), (0, 0, 1, None), (0, 0, 2, Some(512.0)), (0, 0, 3, Some(512.0)),
    (0, 0, 4, Some(256.0)), (0, 0, 8, Some(128.0)), (0, 1, 0, None), (0, 1, 1, Some(512.0)),
    (0, 1, 2, Some(256.0)), (0, 1, 3, Some(170.67)), (0, 1, 4, Some(128.0)), (0, 1, 8, Some(64.0)),
    (0, 2, 0, None), (0, 2, 1, Some(256.0)), (0, 2, 2, Some(128.0)), (0, 2, 3, Some(85.33)),
    (0, 2, 4, Some(64.0)), (0, 2, 8, Some(32.0)), (1, 0, 0, None), (1, 0, 1, Some(512.0)),
    (1, 0, 2, Some(256.0)), (1, 0, 3, Some(170.67)), (1, 0, 4, Some(128.0)), (1, 0, 8, Some(64.0)),
    (1, 1, 0, None), (1, 1, 1, Some(256.0)), (1, 1, 2, Some(128.0)), (1, 1, 3, Some(85.33)),
    (1, 1, 4, Some(64.0)), (1, 1, 8, Some(32.0)),
];

#[test]
fn test_flat_segment_lengths() {
    // "Whenever fnum=0 the phase doesn't change"; "ML=0,block=0,fnum=1 is the only
    // additional combination"; "the phase keeps it's current value, it's not reset".
    for (multi, block, f_number, repeats) in REPEATS {
        let mut tone = PLAIN_SINE;
        tone[1] = 0x20 | multi;
        let mut chip = pitched(&tone, f_number, block);
        chip.phase[C[0]] = 5 << 9;
        let phases: Vec<i32> = attribute(&mut chip, |c| c.phase[C[0]], 512 * 7)
            .into_iter()
            .map(|value| value >> 9)
            .collect();
        let Some(repeats) = repeats else {
            assert!(phases.iter().all(|&p| p == 5));
            continue;
        };
        let lengths: Vec<usize> = middle(&runs(&phases)).iter().map(|&(_, l)| l).collect();
        let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
        assert!((mean - repeats).abs() < 0.05);
        let allowed = [repeats as usize, repeats.ceil() as usize]; // int(r), -int(-r // 1)
        assert!(lengths.iter().all(|length| allowed.contains(length)));
        if repeats == 170.67 {
            // "I measured lengths 171,171,170 for 3 successive segments"
            let mut first = lengths[..3].to_vec();
            first.sort();
            assert_eq!(first, [170, 171, 171]);
        }
    }
}

#[test]
fn test_period_lengths_at_high_pitch() {
    // 20150316.txt, second table: MULTI, block, F-number 256 -> period in samples.
    let mut periods: Vec<(u8, i32, f64)> = (0..8)
        .map(|block| (0, block, (4096 >> block) as f64))
        .collect();
    periods.extend([
        (1, 7, 16.0),
        (2, 7, 8.0),
        (3, 7, 5.33),
        (4, 7, 4.0),
        (5, 7, 3.2),
        (6, 7, 2.67),
        (7, 7, 2.29),
        (8, 7, 2.0),
    ]);
    for (multi, block, period) in periods {
        let mut tone = PLAIN_SINE;
        tone[1] = 0x20 | multi;
        let mut chip = pitched(&tone, 256, block);
        let before = chip.phase[C[0]];
        chip.run(SAMPLE);
        let step = (chip.phase[C[0]] - before).rem_euclid(1 << 19);
        assert_eq!(py_round((1 << 19) as f64 / step as f64, 2), period);
    }
}

#[test]
fn test_the_step_overflows_19_bits_harmlessly() {
    // 20150316.txt, remark 7: MULTI 15, block 7, F-number 511 predicts $EF880,
    // "=0x6F880" in 19 bits.
    let mut tone = PLAIN_SINE;
    tone[1] = 0x2F;
    let mut chip = pitched(&tone, 511, 7);
    let before = chip.phase[C[0]];
    chip.run(SAMPLE);
    assert_eq!((chip.phase[C[0]] - before).rem_euclid(1 << 19), 0x6F880);
}

#[test]
fn test_vibrato_steps_for_the_top_pitch_range() {
    // 20151201.txt: $10 = $C0, $20 = $1D (F-number $1C0, block 6); "regions" of
    // 1,024 samples, the pattern repeating every 8,192. The eight step sizes are
    // the ones docs/validation.md records from that write-up. Approximate in the
    // source: "I only verified the last row", by aligning waveforms.
    let mut chip = pitched(&[0x21, 0x61, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00], 0x1C0, 6);
    chip.vibrato_time = 8191;
    let mut steps = Vec::new();
    let mut last = chip.phase[C[0]];
    for _ in 0..8192 {
        chip.run(SAMPLE);
        steps.push((chip.phase[C[0]] - last).rem_euclid(1 << 19));
        last = chip.phase[C[0]];
    }
    chip.take_outputs();
    #[rustfmt::skip]
    let expected = vec![
        (28672, 1024), (28768, 1024), (28896, 1024), (28768, 1024),
        (28672, 1024), (28576, 1024), (28448, 1024), (28576, 1024),
    ];
    assert_eq!(runs(&steps), expected);
}

// ---------------------------------------------- decay and release (spec 8.5-8.8)

// 20150320.txt (rate 14: "in sequence 2048, 1024 and 1024 samples. And this is
// exactly what we measured"; rates 4, 9 and 19 confirmed the same way) and
// attack.txt section 3, the re-measured rows: samples spent on each level, 0.5
// meaning two levels in one sample. Each row is one turn of a cycle.
fn step_lengths() -> Vec<(i32, Vec<f64>)> {
    let ones = |n: usize| vec![1.0; n];
    let halves = |n: usize| vec![0.5; n];
    let cat = |parts: &[Vec<f64>]| parts.concat();
    vec![
        (4, vec![8192.0]),
        (9, vec![4096.0, 4096.0, 4096.0, 2048.0, 2048.0]),
        (14, vec![2048.0, 1024.0, 1024.0]),
        (19, vec![1024.0, 512.0, 512.0, 512.0, 512.0, 512.0, 512.0]),
        (44, vec![8.0, 8.0, 8.0, 8.0]),
        (45, vec![8.0, 8.0, 8.0, 4.0, 4.0]),
        (46, vec![8.0, 4.0, 4.0, 8.0, 4.0, 4.0]),
        (47, vec![8.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0]),
        (48, vec![4.0, 4.0, 4.0, 4.0]),
        (49, vec![4.0, 4.0, 4.0, 2.0, 2.0]),
        (50, vec![4.0, 2.0, 2.0, 4.0, 2.0, 2.0]),
        (51, vec![4.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]),
        (52, vec![2.0; 8]),
        (53, cat(&[vec![2.0; 6], ones(4)])),
        (54, [2.0, 2.0, 1.0, 1.0, 1.0, 1.0].repeat(2)),
        (55, cat(&[vec![2.0, 2.0], ones(12)])),
        (56, ones(16)),
        (57, cat(&[halves(4), ones(12)])),
        (58, cat(&[halves(4), ones(4)]).repeat(2)),
        (59, cat(&[halves(12), ones(4)])),
        (60, vec![0.5]),
        (61, vec![0.5]),
    ]
}

/// A carrier falling at an effective rate from level 0 (see test_manual).
fn decaying_chip(rate: i32) -> OPLL {
    let tone = [0x00, 0x01, 0x3F, 0x00, 0x00, 0xF0, 0x00, (rate >> 2) as u8];
    let mut chip = struck(&tone, 0x40, 2 * (rate & 3));
    chip.run(SAMPLE);
    chip
}

#[test]
fn test_decay_step_lengths() {
    for (rate, cycle) in step_lengths() {
        let mut chip = decaying_chip(rate);
        chip.counter = 12345;
        let span = (cycle.iter().sum::<f64>() * 6.0) as usize + 40;
        let trace = levels(&mut chip, C[0], span.min(126 * 8192));
        let segments = middle(&runs(&trace));
        let got: Vec<f64> = if cycle.contains(&0.5) {
            // Two levels in one sample: describe each sample by what it did.
            let deltas: Vec<i32> = trace
                .windows(2)
                .filter(|w| w[1] < 127)
                .map(|w| w[1] - w[0])
                .collect();
            assert!(!deltas.contains(&0));
            deltas
                .iter()
                .filter(|&&delta| delta != 0)
                .map(|&delta| if delta == 2 { 0.5 } else { 1.0 })
                .collect()
        } else {
            segments.iter().map(|&(_, length)| length as f64).collect()
        };
        let doubled = cycle.repeat(3);
        let size = cycle.len();
        assert!(
            (0..size).any(|offset| slice(&got, offset, offset + 2 * size) == doubled[..2 * size]),
            "{rate}: {:?}",
            slice(&got, 0, 20)
        );
    }
}

#[test]
fn test_rates_60_to_63_move_two_levels_every_sample() {
    // 20150320.txt: "60..63 -> as expected: increase 2 levels per step".
    let trace = levels(&mut decaying_chip(63), C[0], 20);
    assert_eq!(trace[..6], [2, 4, 6, 8, 10, 12]);
}

fn first_segment_set(attack: u8, decay: u8, block: i32) -> BTreeSet<i32> {
    let tone = [
        0x00,
        0x21,
        0x3F,
        0x00,
        0x00,
        attack << 4 | decay,
        0x00,
        0xF0,
    ];
    let mut found = BTreeSet::new();
    // The patterns repeat every 8 x 2^s samples (0x4000 at the slowest rate
    // here); a start every $40 samples meets every first-segment length.
    for start in (0..0x4000).step_by(0x40) {
        let mut chip = struck(&tone, 0x40, block);
        chip.counter = start;
        while chip.level[C[0]] != 0 {
            chip.run(SAMPLE);
        }
        let mut length = 0;
        while chip.level[C[0]] == 0 {
            chip.run(SAMPLE);
            length += 1;
        }
        chip.take_outputs();
        found.insert((length as f64 / 256.0).round_ties_even() as i32 * 0x100);
    }
    found
}

#[test]
fn test_first_segment_after_attack_4_decay_2() {
    // adsr-transitions.txt: "for the combination AR=4/DR=2 such numbers are
    // '0x200, 0x600 and 0xe00' ... eventually also found a segment with width
    // 0xa00": (2n + 1) x L. This is what fixes the first pattern's rotation.
    assert_eq!(
        first_segment_set(4, 2, 0),
        BTreeSet::from([0x200, 0x600, 0xA00, 0xE00])
    );
}

#[test]
fn test_first_segment_after_attack_5_decay_2() {
    // adsr-transitions.txt, "Extend to higher frequencies". Every length S4
    // measured must occur, and none that S4's account excludes.
    let cases: [(i32, &[i32]); 3] = [
        // "5-out-of-8 ... I measured lengths: 0x100 0x300 0x400 0x500 0x700 0x900
        // 0xb00 0xc00 0xd00 0xf00"
        (2, &[1, 3, 4, 5, 7, 9, 0xB, 0xC, 0xD, 0xF]),
        // "6-out-of-8 ... 0x100 0x200 0x300 0x500 0x600 0x700 0xa00 0xd00 0xe00.
        // The model predicts 0x900 0xb00 0xf00 should also be present"
        (4, &[1, 2, 3, 5, 6, 7, 9, 0xA, 0xB, 0xD, 0xE, 0xF]),
        // "7-out-of-8 ... 0x100 ... 0x700 0xb00 0xc00. The values 0x{9,a,d,e,f}00
        // should also be present (only 0x800 and 0x1000 are missing)"
        (6, &[1, 2, 3, 4, 5, 6, 7, 9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF]),
    ];
    for (block, lengths) in cases {
        assert_eq!(
            first_segment_set(5, 2, block),
            lengths.iter().map(|&length| 0x100 * length).collect()
        );
    }
}

// ---------------------------------------------------------- attack (8.10-8.14)

// attack.txt 2.3 and 2.9: the two sequences.
#[rustfmt::skip]
const FROM_127: [i32; 43] = [127, 119, 111, 104, 97, 90, 84, 78, 73, 68, 63, 59, 55, 51, 47, 44, 41, 38, 35, 32,
                             29, 27, 25, 23, 21, 19, 17, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
#[rustfmt::skip]
const FROM_124: [i32; 41] = [124, 116, 108, 101, 94, 88, 82, 76, 71, 66, 61, 57, 53, 49, 45, 42, 39, 36, 33, 30,
                             28, 26, 24, 22, 20, 18, 16, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1];

fn attacking_chip(attack: u8, block: i32, release: u8) -> OPLL {
    let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, attack << 4, 0x00, release];
    struck(&tone, 0x40, block)
}

fn index_of(values: &[i32], wanted: i32) -> usize {
    values.iter().position(|&v| v == wanted).unwrap()
}

#[test]
fn test_attack_sequence_from_127() {
    // attack.txt 2.3: "if we start at envelope level 127 and iteratively apply
    // 'f4' we get our measured sequence"; "the same as the levels for any attack
    // rate below 12:0"; and 2.6: "In column 12:0 we see the familiar sequence".
    for attack in [7, 10, 11, 12] {
        let mut chip = attacking_chip(attack, 0, 15);
        let trace = [vec![127], levels(&mut chip, C[0], 3000)].concat();
        let upto = &trace[..index_of(&trace, 0) + 1];
        let got: Vec<i32> = runs(upto).iter().map(|&(level, _)| level).collect();
        assert_eq!(got, FROM_127);
    }
}

#[test]
fn test_attack_sequence_from_124_after_a_damp_phase() {
    // attack.txt 2.9: "Each time there is a damp phase (release didn't finish
    // ...) attack starts from 124."
    let mut chip = attacking_chip(10, 0, 0);
    chip.run(SAMPLE * 400); // up to level 0
    note_k(&mut chip, 0, 0x40, 0, 0); // release rate 0: the level stays
    press(&mut chip, 0, 0x40, 0);
    let trace = levels(&mut chip, C[0], 3000);
    let start = index_of(&trace, 124);
    let climb: Vec<i32> = runs(&trace[start..])
        .iter()
        .map(|&(level, _)| level)
        .collect();
    assert_eq!(slice(&climb, 0, FROM_124.len()), FROM_124);
    assert_eq!(*trace[..start + 1].iter().max().unwrap(), 124); // damp went no further
}

#[test]
fn test_three_single_samples_then_a_plateau() {
    // attack.txt 2.2: at rate 10:0 "we see lengths of 13 plus 3 extra samples.
    // This is true for all attack rates less than 12:0"; 2.4: at 11:0 "only a
    // short plateau of 5 samples". (7:0 is 128 samples: 125 plus 3.)
    for (attack, plateau) in [(7, 125), (10, 13), (11, 5)] {
        let mut chip = attacking_chip(attack, 0, 15);
        let trace = levels(&mut chip, C[0], 6000);
        let all = runs(&trace[..index_of(&trace, 0)]);
        let lengths: Vec<usize> = all[1..].iter().map(|&(_, length)| length).collect();
        let mut index = 4;
        while index + 4 < lengths.len() {
            let mut four = slice(&lengths, index, index + 4);
            four.sort();
            assert_eq!(four, [1, 1, 1, plateau]);
            index += 4;
        }
    }
}

// attack.txt 2.6: "columns <n>:1 4x formula f<m> followed by 12x formula f<m+1>;
// <n>:2 4x followed by 4x; <n>:3 12x followed by 4x"; 12:x uses f3 and f4, 13:x
// f2 and f3, 14:x f1 and f2; f<n> is x - (x >> n) - 1.
#[test]
fn test_fast_attack_mixes_two_formulas() {
    for rate in 48..60 {
        let slower = 4 - (rate - 48) / 4;
        let faster_in_16 = [0, 4, 8, 12][(rate & 3) as usize];
        let mut used = Vec::new();
        for counter in 0..16 {
            let mut chip = attacking_chip((rate >> 2) as u8, 2 * (rate & 3), 15);
            chip.run(SAMPLE);
            chip.counter = counter;
            chip.level[C[0]] = 127;
            let trace = [vec![127], levels(&mut chip, C[0], 16)].concat();
            for w in trace.windows(2) {
                let (before, after) = (w[0], w[1]);
                if before >= 64 {
                    // where the two formulas give different results
                    let shift = [1, 2, 3, 4]
                        .into_iter()
                        .find(|&n| before - (before >> n) - 1 == after)
                        .unwrap();
                    assert!(shift == slower || shift == slower - 1);
                    used.push(shift == slower - 1);
                }
            }
            // The first sample of each start is at a different place in the cycle.
        }
        // Python: `used and [...]`, an empty list when nothing was used.
        let first_samples: Vec<bool> = if used.is_empty() {
            Vec::new()
        } else {
            (0..16)
                .map(|counter| attack_formula_is_faster(rate, counter))
                .collect()
        };
        assert_eq!(first_samples.iter().filter(|&&b| b).count(), faster_in_16);
        if rate & 3 == 2 {
            // four on, four off
            assert_eq!(runs(&first_samples.repeat(2))[1].1, 4);
        }
    }
}

fn attack_formula_is_faster(rate: i32, counter: i32) -> bool {
    let mut chip = attacking_chip((rate >> 2) as u8, 2 * (rate & 3), 15);
    chip.run(SAMPLE);
    chip.counter = counter;
    chip.level[C[0]] = 127;
    chip.run(SAMPLE);
    let slower = 4 - (rate - 48) / 4;
    chip.level[C[0]] == 127 - (127 >> (slower - 1)) - 1
}

#[test]
fn test_the_worked_example_at_rate_13_3() {
    // attack.txt 2.6: "the envelope level in that cell is 21. If we apply formula
    // 'f2' on it we obtain 15 ... we need formula 'f3' to get to 13."
    assert!(21 - (21 >> 2) - 1 == 15 && 15 - (15 >> 3) - 1 == 13);
    let mut seen = BTreeSet::new();
    for counter in 0..16 {
        let mut chip = attacking_chip(13, 6, 15);
        chip.run(SAMPLE);
        chip.counter = counter;
        chip.level[C[0]] = 21;
        seen.insert(levels(&mut chip, C[0], 2));
    }
    assert!(seen.contains(&vec![15, 13]));
}

#[test]
fn test_attack_15_is_instant_only_when_damp_ends() {
    // attack.txt 2.8: "attack=15 is only checked at the end of the damp phase";
    // "whenever attack rate 0 or 15 is active the attack curve is paused".
    let mut chip = attacking_chip(0, 0, 15);
    assert!(levels(&mut chip, C[0], 500).iter().all(|&v| v == 127)); // rate 0: no attack
    write(&mut chip, 0x05, 0xF0);
    assert!(levels(&mut chip, C[0], 500).iter().all(|&v| v == 127)); // 15 now: still stalled
    write(&mut chip, 0x05, 0xC0);
    assert_eq!(*levels(&mut chip, C[0], 100).last().unwrap(), 0); // any other rate continues
                                                                  // "the very first sample after attack indeed already has maximum amplitude":
                                                                  // the envelope is at 0 in the sample in which damp ends. With a tone at a
                                                                  // quarter of the sample rate (MULTI 4) that sample's phase is a quarter
                                                                  // period, the peak; that it is computed in the same sample is CHOICE (U20).
    let mut chip = struck(&[0x00, 0x24, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00], 256, 7);
    assert_eq!(melody(&mut chip, 1), [255]);
    assert_eq!(chip.level[C[0]], 0);
}

// ----------------------------------------------------- the states (8.16-8.25)

#[test]
fn test_damp_is_rate_12_and_ends_at_124() {
    // envelope.txt: "the DP phase corresponds to a base decay-rate of '12'. This
    // indeed corresponds to a 0..-48dB time of 10.22ms"; attack.txt 2.9: it stops
    // at 124. From level 0 at rate 48: 124 steps, one every 4 samples.
    let mut chip = attacking_chip(4, 0, 0);
    chip.run(SAMPLE * 30_000);
    chip.take_outputs();
    assert_eq!(chip.level[C[0]], 0);
    note_k(&mut chip, 0, 0x40, 0, 0);
    press(&mut chip, 0, 0x40, 0);
    let trace = levels(&mut chip, C[0], 700);
    let damp = &trace[..index_of(&trace, 124) + 1];
    assert!(124 * 4 - 4 <= damp.len() && damp.len() <= 124 * 4 + 4);
    let lengths: BTreeSet<usize> = middle(&runs(damp)).iter().map(|&(_, l)| l).collect();
    assert_eq!(lengths, BTreeSet::from([4]));
    assert_eq!(*trace.iter().max().unwrap(), 124);
}

#[test]
fn test_damp_is_faster_at_higher_pitch() {
    // envelope.txt: "for higher frequencies the rate is faster, just as it is for
    // all the other rates".
    let mut spans = Vec::new();
    for block in [0, 6] {
        let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, 0x40, 0x00, 0x00];
        let mut chip = struck(&tone, 0x40, block);
        chip.run(SAMPLE);
        chip.level[C[0]] = 0;
        chip.state[C[0]] = 2;
        note_k(&mut chip, 0, 0x40, block, 0);
        press(&mut chip, 0, 0x40, block);
        spans.push(index_of(&levels(&mut chip, C[0], 700), 124));
    }
    assert!(spans[1] < spans[0]);
}

#[test]
fn test_release_runs_to_127() {
    // attack.txt 2.9: "the release phase changes the envelope level all the way
    // to 127".
    let mut chip = attacking_chip(15, 0, 15);
    chip.run(SAMPLE * 10);
    note_k(&mut chip, 0, 0x40, 0, 0);
    assert_eq!(*levels(&mut chip, C[0], 2000).last().unwrap(), 127);
}

fn sustaining(sl: u8, decay: u8, sustained: u8, release: u8) -> OPLL {
    let tone = [
        0x00,
        sustained << 5 | 0x01,
        0x3F,
        0x00,
        0x00,
        0xF0 | decay,
        0x00,
        sl << 4 | release,
    ];
    struck(&tone, 0x40, 0)
}

#[test]
fn test_decay_stops_on_equality_so_a_level_passed_is_never_met() {
    // envelope.txt b2: "started with a high SL value, waited till the envelope
    // decayed beyond a certain level and then changed SL to a value that
    // represents a level above the current level. This makes the envelope
    // continue to decay until -48dB."
    let mut chip = sustaining(15, 8, 1, 0);
    chip.run(SAMPLE * 3000);
    assert!(40 < chip.level[C[0]] && chip.level[C[0]] < 100);
    write(&mut chip, 0x07, 0x20);
    assert_eq!(*levels(&mut chip, C[0], 12_000).last().unwrap(), 127);
}

#[test]
fn test_sustain_can_stick_at_level_9() {
    // envelope.txt b3: from an odd level, a decay of two levels a sample; "I
    // indeed measured a level of 9 instead of 8 (as expected for SL=1)".
    let mut chip = sustaining(1, 1, 1, 0);
    chip.run(SAMPLE);
    chip.level[C[0]] = 5;
    write(&mut chip, 0x05, 0xFF);
    assert_eq!(*levels(&mut chip, C[0], 50).last().unwrap(), 9);
}

#[test]
fn test_changing_sl_during_sustain_does_nothing() {
    // envelope.txt b1.
    let mut chip = sustaining(2, 12, 1, 0);
    chip.run(SAMPLE * 2000);
    assert_eq!(chip.level[C[0]], 16);
    write(&mut chip, 0x07, 0xA0);
    assert!(levels(&mut chip, C[0], 3000).iter().all(|&v| v == 16));
}

#[test]
fn test_eg_typ_switches_sustain_between_holding_and_falling() {
    // envelope.txt d1: "Switching EG_TYPE back and forth keeps switching between
    // the SL and RR phases."
    let mut chip = sustaining(2, 12, 1, 8);
    chip.run(SAMPLE * 2000);
    assert!(levels(&mut chip, C[0], 600).iter().all(|&v| v == 16));
    write(&mut chip, 0x01, 0x01);
    let falling = levels(&mut chip, C[0], 600);
    assert!(*falling.last().unwrap() > 18);
    write(&mut chip, 0x01, 0x21);
    assert_eq!(
        levels(&mut chip, C[0], 600)
            .iter()
            .collect::<BTreeSet<_>>()
            .len(),
        1
    );
}

/// Samples from key-off at level 0 to level 8, a rate's signature.
fn release_span(sustained: u8, sus: u8, release: u8) -> usize {
    let tone = [
        0x00,
        sustained << 5 | 0x01,
        0x3F,
        0x00,
        0x00,
        0xF0,
        0x00,
        release,
    ];
    let mut chip = struck(&tone, 0x40, 0);
    chip.run(SAMPLE * 3);
    chip.counter = 0;
    chip.level[C[0]] = 0;
    chip.write(0, 0x20);
    chip.write(1, sus << 5);
    index_of(&levels(&mut chip, C[0], 40_000), 8)
}

#[test]
fn test_release_rates() {
    // envelope.txt: percussive tone, "SUS-ON selects the RS curve with a
    // basis-rate=5. SUS-OFF selects RR' with basis-rate=7"; sustained tone, "For
    // SUS-ON the basis-rate is fixed at 5, for SUS-OFF the basis rate is the
    // selected RR-rate."
    let by_rate: Vec<(u8, usize)> = [5, 7, 9, 3]
        .into_iter()
        .map(|rate| (rate, release_span(1, 0, rate)))
        .collect();
    let rate = |r: u8| by_rate.iter().find(|&&(x, _)| x == r).unwrap().1;
    assert_eq!(
        by_rate
            .iter()
            .map(|&(_, span)| span)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(release_span(1, 0, 9), rate(9)); // sustained, SUS off: RR
    assert_eq!(release_span(1, 1, 9), rate(5)); // sustained, SUS on: 5
    assert_eq!(release_span(0, 0, 9), rate(7)); // percussive, SUS off: 7
    assert_eq!(release_span(0, 1, 9), rate(5)); // percussive, SUS on: 5
    assert_eq!(release_span(0, 0, 3), rate(7));
}

#[test]
fn test_sus_and_rr_changes_act_at_once_during_release() {
    // envelope.txt a and c: "The change takes effect immediately."
    let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x02];
    let mut chip = struck(&tone, 0x40, 0);
    chip.run(SAMPLE * 3);
    note_k(&mut chip, 0, 0x40, 0, 0);
    let slow = *levels(&mut chip, C[0], 2000).last().unwrap();
    write(&mut chip, 0x07, 0x0C);
    let fast = *levels(&mut chip, C[0], 200).last().unwrap();
    assert!(fast - slow > 20);
    write(&mut chip, 0x20, 0x20); // SUS on: rate 5
    assert!(*levels(&mut chip, C[0], 200).last().unwrap() - fast < 2);
}

#[test]
fn test_key_off_goes_to_release_from_any_state() {
    // envelope.txt e: "changing KEY-ON->OFF always goes to the (2nd or only)
    // release phase", from damp, attack and decay as well.
    for when in ["damp", "attack", "decay"] {
        let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, 0x51, 0x00, 0xFD]; // SL 15: a long decay
        let mut chip = struck(&tone, 0x40, 0);
        if when == "damp" {
            chip.run(SAMPLE);
            chip.level[C[0]] = 40;
            chip.state[C[0]] = 0;
            chip.run(SAMPLE * 8);
        } else if when == "attack" {
            chip.run(SAMPLE * 3000);
            assert_eq!(chip.state[C[0]], 1);
        } else {
            chip.run(SAMPLE * 40_000);
            assert_eq!(chip.state[C[0]], 2);
        }
        chip.take_outputs();
        note_k(&mut chip, 0, 0x40, 0, 0);
        assert_eq!(*levels(&mut chip, C[0], 600).last().unwrap(), 127, "{when}");
    }
}

#[test]
fn test_a_modulator_has_no_release() {
    // envelope.txt, "Modulator-envelope has no release phase?": at key-off "The
    // carrier envelope starts to decay, but the modulation depth remains the
    // same!"; and for EG_TYPE=0 it "does not go to the (2nd) release phase".
    let tone = [0x21, 0x21, 0x00, 0x00, 0xF4, 0xF4, 0x2F, 0x2F];
    let mut chip = plain(&tone);
    chip.run(SAMPLE * 30_000); // DR 4 at block 4 (rate 17) takes ~13,000 to level 16
    assert!(chip.level[M[0]] == 16 && chip.level[C[0]] == 16);
    note_k(&mut chip, 0, 0x100, 4, 0);
    chip.run(SAMPLE * 500);
    assert!(chip.level[M[0]] == 16 && chip.level[C[0]] == 127);
    // A percussive modulator carries on as if the key were still on: its own
    // decay and then its own RR, not rate 7. A twin chip that keeps the key on
    // shows the same modulator levels sample for sample.
    let tone = [0x01, 0x21, 0x00, 0x00, 0xF4, 0xF4, 0x22, 0x2F];
    let (mut released, mut held) = (plain(&tone), plain(&tone));
    for chip in [&mut released, &mut held] {
        chip.run(SAMPLE * 3000);
    }
    note_k(&mut released, 0, 0x100, 4, 0);
    note_k(&mut held, 0, 0x100, 4, 1);
    let trace = levels(&mut released, M[0], 30_000);
    assert_eq!(trace, levels(&mut held, M[0], 30_000));
    assert!(trace[0] < *trace.last().unwrap() && *trace.last().unwrap() < 127);
}

// Key scale of rate, sl-ksr.txt: R, KSR, BLOCK, F-number bit 8 -> rate; each
// line "confirmed" on the chip (a to j).
#[rustfmt::skip]
const CONFIRMED_RATES: [(u8, u8, i32, i32, i32); 10] = [
    (4, 0, 4, 0, 18), (4, 0, 4, 1, 18), (4, 1, 1, 0, 18), (3, 1, 3, 0, 18),
    (2, 1, 5, 0, 18), (4, 1, 4, 1, 25), (4, 1, 4, 0, 24), (4, 1, 3, 0, 22),
    (3, 1, 5, 0, 22), (2, 0, 5, 0, 10),
];

#[test]
fn test_confirmed_effective_rates() {
    // "a narrow step takes 512 samples ... 13 - log2(512) = 4 ... an effective
    // rate of 4*4+2 = 18"; "wide, narrow, narrow" when the low bits are 2.
    for (rate_field, ksr, block, msb, rate) in CONFIRMED_RATES {
        let tone = [
            0x00,
            0x20 | ksr << 4 | 0x01,
            0x3F,
            0x00,
            0x00,
            0xF0 | rate_field,
            0x00,
            0xF0,
        ];
        let mut chip = struck(&tone, msb << 8 | 0x40, block);
        chip.run(SAMPLE);
        let narrow = 1usize << (13 - (rate >> 2));
        let trace = levels(&mut chip, C[0], narrow * 30);
        let all = runs(&trace);
        let lengths: Vec<usize> = slice(&all, 2, all.len().saturating_sub(1))
            .iter()
            .map(|&(_, length)| length)
            .collect();
        let pattern: Vec<usize> = match rate & 3 {
            0 => vec![2],
            1 => vec![2, 2, 2, 1, 1],
            _ => vec![2, 1, 1],
        };
        let size = pattern.len();
        let expected: Vec<usize> = pattern.repeat(4).iter().map(|each| narrow * each).collect();
        assert!(
            (0..size)
                .any(|offset| slice(&lengths, offset, offset + size * 2) == expected[..size * 2]),
            "{rate}"
        );
    }
}

#[test]
fn test_key_scale_level_codes() {
    // ksl.txt: measured for "KSL=1, 2 and 3", an "exact match" with Yamaha's
    // table read as 1.5, 3 and 6 dB per octave in that order (contradiction C3).
    fn attenuation(code: u8) -> i32 {
        let tone = [0x21, 0x21, 0x3F, code << 6, 0x00, 0xF0, 0x00, 0x00];
        let mut chip = pitched(&tone, 0x1E0, 7);
        let peak = *melody(&mut chip, 300).iter().max().unwrap();
        (0..128).find(|&steps| carrier_peak(steps) == peak).unwrap()
    }

    // Yamaha's cell for octave 7, F-number top bits 15: 21 dB at 3 dB/oct, which
    // is 56 envelope steps of 0.375 dB; half and double for the other settings.
    assert_eq!(
        [attenuation(0), attenuation(1), attenuation(2)],
        [0, 28, 56]
    );
    assert_eq!(
        *melody(
            &mut pitched(&[0x21, 0x21, 0x3F, 0xC0, 0, 0xF0, 0, 0], 0x1E0, 7),
            300
        )
        .iter()
        .max()
        .unwrap(),
        carrier_peak(112)
    );
}

// ------------------------------------------------------------ tremolo (10)

#[test]
fn test_tremolo_segment_lengths() {
    // am.txt: "(only) 14 different levels (0..13)"; "top: 960 samples (15 x 64),
    // middle: 512 samples (8 x 64), bottom: 192 samples (3 x 64)".
    let mut chip = plain(&[0x21, 0xA1, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00]);
    let mut seen = Vec::new();
    for _ in 0..13_440 * 2 + 2000 {
        chip.run(SAMPLE);
        seen.push(chip.tremolo);
    }
    chip.take_outputs();
    let segments = middle(&runs(&seen));
    assert_eq!(
        segments.iter().map(|&(v, _)| v).collect::<BTreeSet<_>>(),
        (0..14).collect()
    );
    for &(value, length) in &segments {
        assert_eq!(
            length,
            if value == 0 {
                960
            } else if value == 13 {
                192
            } else {
                512
            }
        );
    }
    let mut cycle: Vec<i32> = segments[..26].iter().map(|&(value, _)| value).collect();
    cycle.sort();
    let mut expected: Vec<i32> = [vec![0, 13], (1..13).collect(), (1..13).collect()].concat();
    expected.sort();
    assert_eq!(cycle, expected);
}

#[test]
fn test_tremolo_attenuates_by_envelope_steps() {
    // am.txt: the extra term is "16 * am", am 0 to 13: one envelope step each;
    // the measured swing is 255 down to "approximately ... +144".
    let mut chip = pitched(&[0x21, 0xA1, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00], 256, 4);
    let out = melody(&mut chip, 13_440);
    let peaks: BTreeSet<i32> = (0..13_440 - 128)
        .step_by(64)
        .map(|i| *out[i..i + 128].iter().max().unwrap())
        .collect();
    let (high, low) = (*peaks.iter().max().unwrap(), *peaks.iter().min().unwrap());
    assert!(high == 255 && low == carrier_peak(13) && carrier_peak(13) == 145);
}

// ------------------------------------------------------ rhythm (spec 11)

// lfsr.txt: 120 consecutive snare samples at F-number 0, "sample value 255 to '0'
// and all other values to '1'".
const SNARE_BITS: &str = concat!(
    "1101010010",
    "0111011001",
    "0011010111",
    "0101110000",
    "1011010001",
    "1001110010",
    "0110100101",
    "1000011101",
    "1001010001",
    "0111000111",
    "0001101111",
    "1010010001"
);

// rhythm.md, the 2020 update: the noise bit as seen by the hi-hat and by the
// snare in the same 50 samples.
const HI_HAT_BITS: &str = "00011000110100100001001000110011000010110011001101";
const SNARE_BITS_50: &str = "10010001100000110111000101001100101101111111010011";

fn bits(text: &str) -> Vec<i32> {
    text.bytes().map(|b| (b - b'0') as i32).collect()
}

/// The noise bit at the hi-hat's slot and at the snare's, sample by sample.
fn noise_as_used(chip: &mut OPLL, count: usize) -> (Vec<i32>, Vec<i32>) {
    let (mut hi_hat, mut snare) = (Vec::new(), Vec::new());
    assert!(chip.slot == 0 && chip.clock_phase == 0);
    for _ in 0..count {
        for slot in 0..18 {
            if slot == 13 {
                hi_hat.push(chip.noise & 1);
            } else if slot == 16 {
                snare.push(chip.noise & 1);
            }
            chip.run(4);
        }
    }
    chip.take_outputs();
    (hi_hat, snare)
}

/// The 23-bit register value at slot 0 that gives the observed bits, by
/// elimination over GF(2), or None if no value does. `observed` is a list of
/// (steps from slot 0 of the first sample, bit).
fn solve_noise_state(observed: &[(usize, i32)]) -> Option<i32> {
    let mut masks: Vec<u32> = (0..23).map(|stage| 1 << stage).collect(); // each stage as a sum of the unknowns
    let mut rows: Vec<(u32, i32)> = Vec::new();
    let last = observed.iter().map(|&(steps, _)| steps).max().unwrap();
    for step in 0..=last {
        if let Some(&(_, bit)) = observed.iter().rev().find(|&&(steps, _)| steps == step) {
            rows.push((masks[0], bit));
        }
        let next = masks[0] ^ masks[14];
        masks.remove(0);
        masks.push(next);
    }
    // Python's dict keeps insertion order: pivots in the order they were found.
    let mut pivots: Vec<(u32, (u32, i32))> = Vec::new();
    for (mut mask, mut bit) in rows {
        for &(pivot, (pivot_mask, pivot_bit)) in &pivots {
            if mask >> pivot & 1 != 0 {
                mask ^= pivot_mask;
                bit ^= pivot_bit;
            }
        }
        if mask == 0 {
            if bit != 0 {
                return None;
            }
            continue;
        }
        let pivot = 31 - mask.leading_zeros();
        for (_, other) in pivots.iter_mut() {
            if other.0 >> pivot & 1 != 0 {
                other.0 ^= mask;
                other.1 ^= bit;
            }
        }
        pivots.push((pivot, (mask, bit)));
    }
    if pivots.len() < 23 {
        return None;
    }
    Some(pivots.iter().map(|&(pivot, (_, bit))| bit << pivot).sum())
}

#[test]
fn test_the_snare_noise_seen_once_a_sample() {
    // lfsr.txt: the Berlekamp-Massey algorithm on those 120 bits gives the
    // register with taps 1, 8, 9 and 23, period 8,388,607; "if I take only the 60
    // first samples ... the resulting LFSR can then correctly predict the
    // continuation".
    let printed = bits(SNARE_BITS);
    for sequence in [
        printed.clone(),
        noise_as_used(&mut OPLL::new(YM2413), 400).1,
    ] {
        assert!((23..sequence.len()).all(|n| sequence[n]
            == sequence[n - 1] ^ sequence[n - 8] ^ sequence[n - 9] ^ sequence[n - 23]));
    }
    // ...and the printed bits are a stretch of the chip model's own sequence.
    let observed: Vec<(usize, i32)> = printed
        .iter()
        .enumerate()
        .map(|(n, &bit)| (16 + 18 * n, bit))
        .collect();
    let state = solve_noise_state(&observed);
    assert!(matches!(state, Some(s) if s != 0));
    let mut chip = OPLL::new(YM2413);
    chip.noise = state.unwrap();
    assert_eq!(noise_as_used(&mut chip, 120).1, printed);
}

#[test]
fn test_the_register_steps_once_a_slot() {
    // rhythm.md, 2020 update: "if we iterate the HH-state 3 times we end up in
    // the SD-state"; "HH gets calculated in slot 13 and SD in slot 16"; 30 bits
    // fixed the state and "the next 20 samples matched perfectly".
    let hi_hat = bits(HI_HAT_BITS);
    let snare = bits(SNARE_BITS_50);
    let observed: Vec<(usize, i32)> = hi_hat[..30]
        .iter()
        .enumerate()
        .map(|(n, &bit)| (13 + 18 * n, bit))
        .collect();
    let state = solve_noise_state(&observed);
    assert!(matches!(state, Some(s) if s != 0));
    let mut chip = OPLL::new(YM2413);
    chip.noise = state.unwrap();
    assert_eq!(
        noise_as_used(&mut chip, 50),
        (hi_hat.clone(), snare.clone())
    );
    // Stepped once a sample instead, no register value gives both.
    assert_ne!(hi_hat, snare);
}

#[test]
fn test_the_snare_at_f_number_0_shows_the_noise_alone() {
    // lfsr.txt: with F-number 0, "all samples have the same sign" and the signal
    // toggles between zero and a peak that follows the envelope.
    let mut chip = rhythm_chip_at(0x08, 0, 0, 0x1C0, 0);
    chip.run(SAMPLE * 40);
    chip.take_outputs();
    let (mut noise_bits, mut heard) = (Vec::new(), Vec::new());
    for _ in 0..300 {
        for slot in 0..18 {
            if slot == 16 {
                noise_bits.push(chip.noise & 1);
            }
            chip.run(4);
        }
        heard.push(chip.take_outputs()[0].1);
    }
    let fresh: Vec<i32> = words(&mut rhythm_chip_at(0x08, 0, 0, 0x1C0, 0), 300)
        .iter()
        .map(|sample| sample[16].1)
        .collect();
    assert!(fresh.iter().all(|&word| word >= 0) && *fresh.iter().max().unwrap() > 100);
    assert!(heard.len() == 300 && noise_bits.iter().any(|&b| b != 0));
}

#[test]
fn test_the_snare_table() {
    // rhythm.md, "Snare drum": noise 0 gives +0 or the negative peak, noise 1
    // the positive peak or -0, by one bit of the phase; "+max and -max appear
    // diagonally". (Which column goes with which phase is U6.)
    let mut chip = rhythm_chip_at(0x08, 0x100, 1, 0x1C0, 0);
    chip.run(SAMPLE * 40);
    chip.take_outputs();
    let mut kinds = BTreeSet::new();
    let mut noise = 0;
    for _ in 0..3000 {
        for slot in 0..18 {
            if slot == 16 {
                noise = chip.noise & 1;
            }
            chip.run(4);
        }
        let phase_bit = (chip.phase[16] >> 17) & 1;
        let word = chip.held[16];
        let kind = if word == 0 {
            "+0"
        } else if word == -1 {
            "-0"
        } else if word > 0 {
            "+peak"
        } else {
            "-peak"
        };
        kinds.insert((noise, phase_bit, kind));
    }
    chip.take_outputs();
    assert_eq!(
        kinds,
        BTreeSet::from([(0, 0, "+0"), (0, 1, "-peak"), (1, 0, "+peak"), (1, 1, "-0")])
    );
}

#[test]
fn test_the_bass_drum_is_the_user_tone_with_the_measured_settings() {
    // rhythm.md, "Base Drum": the user tone 01 01 18 0F DF F8 6A 6D, played
    // "'simultaneously'" at F-number $1FF, block 0, is "a pretty good match"; "the
    // values shown in the above table gave the best result". In the model the two
    // are the same words.
    let mut chip = OPLL::new(YM2413);
    user_tone(&mut chip, &[0x01, 0x01, 0x18, 0x0F, 0xDF, 0xF8, 0x6A, 0x6D]);
    write(&mut chip, 0x36, 0x00);
    write(&mut chip, 0x35, 0x00);
    for channel in [5, 6] {
        note_k(&mut chip, channel, 0x1FF, 0, 0);
    }
    write(&mut chip, 0x0E, 0x20);
    chip.write(0, 0x0E);
    chip.write(1, 0x30);
    chip.write(0, 0x25);
    chip.write(1, 0x11);
    let trace = words(&mut chip, 4000);
    let drum: Vec<i32> = trace.iter().map(|sample| sample[15].1).collect();
    let tone: Vec<i32> = trace.iter().map(|sample| sample[11].0).collect();
    assert!(drum == tone && *drum.iter().max().unwrap() > 100);
}

#[test]
fn test_the_tom_is_a_plain_sine_at_channel_8s_pitch() {
    // rhythm.md, "Tom tom": "a simple sine wave that's varying in amplitude";
    // "the frequency of the TOM waveform varies with the settings from registers
    // 0x18 and 0x28".
    for (f_number, block) in [(0x100, 3), (0x100, 4)] {
        let mut chip = rhythm_chip_at(0x04, 0x150, 2, f_number, block);
        let tom: Vec<i32> = words(&mut chip, 2048)
            .iter()
            .map(|sample| sample[14].1)
            .collect();
        let crossings = tom.windows(2).filter(|w| w[0] < 0 && 0 <= w[1]).count() as f64;
        // the ROM row's multiplier is 5 (byte $00 = $05)
        let expected = 2048.0 * 5.0 * ((f_number << block) >> 1) as f64 / (1 << 18) as f64;
        assert!((crossings - expected).abs() <= 1.0);
        assert!(*tom.iter().max().unwrap() > 200);
    }
}

#[test]
fn test_the_top_cymbal_is_two_peaks_and_reaches_full_scale() {
    // rhythm.md, "Top cymbal": "the amplitude only seems to take on two distinct
    // values (maximum positive or maximum negative)"; "at the end of the attack
    // phase the signal does span the full -255..+255 range"; "both the frequency
    // settings for channel 7 ... and for channel 8 ... influence how the TCY
    // waveform looks".
    let mut chip = rhythm_chip_at(0x02, 0x100, 3, 0x100, 4);
    write(&mut chip, 0x38, 0x00);
    let mut takes = Vec::new();
    for f7 in [0x100, 0x180] {
        let mut chip = rhythm_chip_at(0x02, f7, 3, 0x100, 4);
        takes.push(
            words(&mut chip, 3000)
                .iter()
                .map(|sample| sample[17].1)
                .collect::<Vec<i32>>(),
        );
    }
    let cymbal = &takes[0];
    assert!(*cymbal.iter().max().unwrap() == 255 && *cymbal.iter().min().unwrap() == -256);
    // two values at a time: one size, both signs, the size falling with the
    // envelope once the attack has reached full scale
    let sizes: Vec<i32> = cymbal
        .iter()
        .map(|&word| if word >= 0 { word } else { !word })
        .collect();
    let peak = index_of(&sizes, 255);
    assert!(sizes[peak..].windows(2).all(|w| w[0] >= w[1]));
    assert_ne!(takes[0], takes[1]);
    let other: Vec<i32> = words(&mut rhythm_chip_at(0x02, 0x100, 3, 0x140, 4), 3000)
        .iter()
        .map(|sample| sample[17].1)
        .collect();
    assert_ne!(&other, cymbal);
}

#[test]
fn test_rhythm_keys_are_ored_with_the_channel_keys() {
    // rhythm.md: "in rhythm mode _both_ the on/off bits in register 0x0E and the
    // bits in registers 0x26-0x28 are taken into account. It's as-if the
    // 'corresponding' bits are OR'ed together." Loosely measured (marked TODO in
    // the source): PROVISIONAL in the core (T9).
    let mut chip = rhythm_chip(0);
    assert!(rhythm(&mut chip, 500).iter().all(|&v| v == 0));
    note_k(&mut chip, 8, 0x1C0, 0, 1);
    assert!(*rhythm(&mut chip, 500).iter().max().unwrap() > 50);
}

#[test]
fn test_aliases_of_the_channel_registers() {
    // ksl.txt: "a write to register R#19 has the same effect as a write to R#10",
    // $19-$1F, $29-$2F and $39-$3F to $10-$16, $20-$26 and $30-$36.
    for alias in (0x19..0x20).chain(0x29..0x30).chain(0x39..0x40) {
        let mut chip = OPLL::new(YM2413);
        write(&mut chip, alias, 0x15);
        assert_eq!(chip.registers[alias as usize - 9], 0x15);
        assert_eq!(chip.registers.iter().map(|&b| b as u32).sum::<u32>(), 0x15);
    }
    let mut chip = tone_chip(
        &PLAIN_SINE,
        Tone {
            key: 0,
            ..Tone::default()
        },
    );
    write(&mut chip, 0x29, 0x19); // key on, block 4, F-number bit 8 kept
    assert_eq!(*melody(&mut chip, 600).iter().max().unwrap(), 255);
}
