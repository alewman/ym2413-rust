//! Rung 1: Yamaha's application manual (S1), by section, table and printed page.
//!
//! Every check is a sentence or a table row of "FM Operator Type-LL (OPLL)
//! Application Manual". "documented" throughout; where S1 and the measurements of
//! rung 2 disagree, the test says which was taken and why (contradictions C1, C2,
//! C6). Tables III-1, III-2, III-5, III-6, III-7, III-8-2 and III-9 were read from
//! the page images and typed here.
//!
//! A transcription of `tests/test_manual.py` (at f88dccf); parametrized cases are loops.

mod common;

use std::collections::BTreeSet;

use common::*;
use ym2413::{OPLL, YM2413};

// ---------------------------------------------------------------- the bus

#[test]
fn test_an_address_write_then_a_data_write_stores_the_byte() {
    // S1 Table II-1 (page 4): A0 = 0 address write mode, A0 = 1 data write mode.
    let mut chip = OPLL::new(YM2413);
    chip.write(0, 0x30);
    chip.write(1, 0x5A);
    assert_eq!(chip.registers[0x30], 0x5A);
}

#[test]
fn test_the_address_stays_latched() {
    // S1 III-1 (page 8): "we only have to send the sub-address the first time".
    let mut chip = OPLL::new(YM2413);
    chip.write(0, 0x10);
    chip.write(1, 0x11);
    chip.write(1, 0x22);
    assert_eq!(chip.registers[0x10], 0x22);
}

#[test]
fn test_initial_clear_zeroes_every_register() {
    // S1 III-1 (page 8): "All registers are cleared to 0 by initialization".
    let mut chip = plain(&[0x21, 0x21, 0x10, 0x07, 0xF0, 0xF0, 0x00, 0x0F]);
    write(&mut chip, 0x0E, 0x3F);
    chip.reset();
    chip.take_outputs(); // what was played before the reset
    assert_eq!(chip.registers, [0; 0x40]);
    assert!(melody(&mut chip, 500).iter().all(|&v| v == 0));
}

#[test]
fn test_72_clocks_make_a_sample() {
    // S1 III-1-6 (page 14): fsam = fM / 72; with section IV-1's 3.579545 MHz that
    // is 49,716 Hz. S1 rounds to "50 kHz" (C6).
    let mut chip = OPLL::new(YM2413);
    chip.run(CLOCK_HZ);
    let count = chip.take_samples().len() as u64;
    assert!(count == CLOCK_HZ / 72 && count == 49_715);
}

// ------------------------------------------------------- frequency and MULTI

const MULTIPLIER: [f64; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
]; // S1 Table III-1 (page 9)

#[test]
fn test_table_iii_1_multiplying_factors() {
    // F-number 128, block 2: with multiplier 1 one period is 1,024 samples, so in
    // 2,048 samples there are twice the multiplier's worth of cycles.
    for multi in 0..16u8 {
        let mut tone = PLAIN_SINE;
        tone[1] = 0x20 | multi;
        let mut chip = pitched(&tone, 128, 2);
        assert_eq!(
            rises(&melody(&mut chip, 2048)),
            (2.0 * MULTIPLIER[multi as usize]).round_ties_even() as usize
        );
    }
}

// S1 Table III-8-2 (page 16): note, frequency in Hz, F-number, for octave 4.
#[rustfmt::skip]
const F_NUMBERS: [(&str, f64, i32); 12] = [
    ("G", 392.2, 257), ("G#", 415.3, 272), ("A", 440.0, 288), ("A#", 466.2, 305),
    ("B", 493.9, 323), ("C", 523.3, 343), ("C#", 554.4, 363), ("D", 587.3, 385),
    ("D#", 622.2, 408), ("E", 659.3, 432), ("F", 698.5, 458), ("F#", 740.0, 485),
];

#[test]
fn test_table_iii_8_2_f_numbers() {
    // S1 III-1-6 (pages 14, 15): F-number = fmus x 2^18 / fsam / 2^(block - 1).
    // The table is computed for fsam = 50 kHz, not the real 49,716 Hz (C6): at
    // 50 kHz the chip's phase step for the printed F-number is the printed note.
    for (name, hertz, f_number) in F_NUMBERS {
        let mut chip = pitched(&PLAIN_SINE, f_number, 4);
        let before = chip.phase[C[0]];
        chip.run(SAMPLE);
        let step = (chip.phase[C[0]] - before).rem_euclid(1 << 19);
        // Within one F-number step (1.53 Hz): the table drops fractions (B is 323.7).
        assert!(
            (50_000.0 * step as f64 / (1 << 19) as f64 - hertz).abs() < 1.53,
            "{name}"
        );
    }
}

#[test]
fn test_the_frequency_formula_at_the_real_sample_rate() {
    // Spec 7.5: fsam x F-number x 2^block x multiplier / 2^19. F-number 256,
    // block 4, multiplier 1: 1/128 of the sample rate, 388.4 Hz.
    let mut chip = pitched(&PLAIN_SINE, 256, 4);
    assert_eq!(rises(&melody(&mut chip, 128 * 100)), 100);
}

// -------------------------------------------------------------- key scaling

// S1 Table III-2 (page 9): Rks by BLOCK and F-number MSB, for KSR 0 and KSR 1.
const RKS: [[i32; 16]; 2] = [
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
];

#[test]
fn test_table_iii_2_key_scale_of_rate() {
    // S1 III-1-2 (page 9): "RATE = 4 x R + Rks". A decay at DR 9 runs at rate
    // 36 + Rks, and each rate has its own count of steps in 4,096 samples.
    for ksr in 0..2u8 {
        for cell in 0..16i32 {
            let (block, msb) = (cell >> 1, cell & 1);
            let tone = [
                0x00,
                0x20 | ksr << 4 | 0x01,
                0x3F,
                0x00,
                0x00,
                0xF9,
                0x00,
                0xF0,
            ];
            let mut chip = struck(&tone, msb << 8 | 0x40, block);
            chip.run(SAMPLE); // damp ends and the instant attack puts the level at 0
            assert_eq!(chip.level[C[0]], 0);
            chip.counter = 0;
            let rate = 36 + RKS[ksr as usize][cell as usize];
            // Eight opportunities (spec 8.5) are 2^(16 - rate // 4) samples, in which a
            // rate steps 4, 5, 6 or 7 times by its low two bits: each rate from 36 to 51
            // has its own pair of numbers.
            let window = 1usize << (16 - (rate >> 2));
            let trace = levels(&mut chip, C[0], 2 * window);
            assert_eq!(
                (trace[window - 1], *trace.last().unwrap()),
                (4 + (rate & 3), 2 * (4 + (rate & 3))),
                "{rate}"
            );
            assert!(*levels(&mut chip, C[0], window / 2).last().unwrap() < 3 * (4 + (rate & 3)));
        }
    }
}

#[test]
fn test_rate_is_zero_when_r_is_zero() {
    // S1 III-1-2 (page 9): "Note that when R = 0, RATE = 0", whatever Rks is.
    let tone = [0x00, 0x31, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0xF0];
    let mut chip = pitched(&tone, 0x1FF, 7);
    assert!(levels(&mut chip, C[0], 3000)[10..].iter().all(|&v| v == 0));
}

// S1 Table III-5 (page 11), bottom row: dB at 3 dB/oct in octave 7.
#[rustfmt::skip]
const KSL_ROW: [f64; 16] = [0.0, 9.0, 12.0, 13.875, 15.0, 16.125, 16.875, 17.625,
                            18.0, 18.75, 19.125, 19.5, 19.875, 20.25, 20.625, 21.0];

/// The largest output of a modulator at full envelope level.
fn modulator_peak(ksl: u8, total_level: u8, f_top: i32, block: i32) -> i32 {
    let f_number = f_top << 5 | 0x08;

    // A multiplier that moves the phase a few table steps a sample, so that a
    // period is short and still passes close to the sine's top.
    let per_sample = |m: usize| -> f64 {
        ((f_number * (2.0 * MULTIPLIER[m]).round_ties_even() as i32) << block) as f64 / 1024.0
    };

    let mut multi = 0;
    for m in 1..16 {
        if (per_sample(m) - 3.0).abs() < (per_sample(multi) - 3.0).abs() {
            multi = m; // Python's min() keeps the first of equals
        }
    }
    let tone = [
        0x20 | multi as u8,
        0x21,
        ksl << 6 | total_level,
        0x00,
        0xF0,
        0xF0,
        0x00,
        0x00,
    ];
    let mut chip = pitched(&tone, f_number, block);
    let span = (1024.0 / per_sample(multi)) as usize + 16;
    attribute(&mut chip, |c| c.previous[0], span)[8..]
        .iter()
        .map(|v| v.abs())
        .max()
        .unwrap()
}

fn decibels(peak: i32) -> f64 {
    20.0 * (2042.0 / peak as f64).log10()
}

#[test]
fn test_table_iii_5_attenuation_at_3_db_per_octave() {
    // All sixteen cells of one row. The printed row for octave 7, less 3 dB per
    // octave below it, floored at 0 (the table's other rows show exactly that).
    // Code 2 is the 3 dB setting: measured (S4), see test_measured (C3).
    for block in 0..8 {
        for f_top in 0..16 {
            let printed = 0f64.max(KSL_ROW[f_top as usize] - 3.0 * (7 - block) as f64);
            let got = decibels(modulator_peak(2, 0, f_top, block));
            assert!(
                (got - printed).abs() < 0.2,
                "{block} {f_top} {got} {printed}"
            );
        }
    }
}

#[test]
fn test_table_iii_5_notes_half_and_double() {
    // S1 page 11: "Half of the above data at 1.5 dB/oct", "Double ... at 6 dB/oct".
    // To the nearest envelope step (0.375 dB) below.
    for (code, factor) in [(1, 0.5), (3, 2.0)] {
        for block in [2, 5, 7] {
            for f_top in 0..16 {
                let printed = factor * 0f64.max(KSL_ROW[f_top as usize] - 3.0 * (7 - block) as f64);
                let got = decibels(modulator_peak(code, 0, f_top, block));
                assert!(
                    -0.5 < got - printed && got - printed < 0.2,
                    "{block} {f_top} {got} {printed}"
                );
            }
        }
    }
}

#[test]
fn test_table_iii_3_total_level() {
    // S1 Table III-3 (page 11): the six bits weigh 0.75 to 24 dB.
    for (bit, db) in [0.75, 1.5, 3.0, 6.0, 12.0, 24.0].into_iter().enumerate() {
        assert!((decibels(modulator_peak(0, 1 << bit, 8, 4)) - db).abs() < 0.1);
    }
}

#[test]
fn test_volume_weights() {
    // S1 III-1-8 (page 17): "The minimum resolution is 3 dB". Within the rounding
    // of a nine-bit word.
    for (bit, db) in [3.0, 6.0, 12.0, 24.0].into_iter().enumerate() {
        let mut chip = tone_chip(
            &PLAIN_SINE,
            Tone {
                volume: 1 << bit,
                ..Tone::default()
            },
        );
        let peak = *melody(&mut chip, 600).iter().max().unwrap();
        assert!((20.0 * (255.0 / peak as f64).log10() - db).abs() < 0.65);
    }
}

#[test]
fn test_sustain_level_weights() {
    // S1 III-1-5 (page 12): SL's four bits weigh 3, 6, 12 and 24 dB.
    for (bit, db) in [3.0, 6.0, 12.0, 24.0].into_iter().enumerate() {
        let tone = [0x21, 0x21, 0x3F, 0x00, 0x00, 0xFF, 0x00, (1 << bit) << 4];
        let mut chip = plain(&tone);
        chip.run(SAMPLE * 400);
        chip.take_outputs();
        let peak = *melody(&mut chip, 600).iter().max().unwrap();
        assert!((20.0 * (255.0 / peak as f64).log10() - db).abs() < 0.65);
    }
}

// ---------------------------------------------------------------- feedback

#[test]
fn test_table_iii_6_modulation_index() {
    // S1 Table III-6 (page 12): 0, pi/16, pi/8, pi/4, pi/2, pi, 2 pi, 4 pi. The
    // index is the largest phase swing the fed-back signal gives: a full-scale
    // modulator (two outputs of 2,042 summed) against a period of 1,024 steps.
    use std::f64::consts::PI;
    for fb in 0..8u8 {
        let printed = if fb == 0 {
            0.0
        } else {
            PI / 16.0 * 2f64.powi(fb as i32 - 1)
        };
        let tone = [0x21, 0x21, 0x00, fb, 0xF0, 0xF0, 0x00, 0x00];
        let mut chip = pitched(&tone, 0x100, 2);
        let mut still = pitched(&[0x21, 0x21, 0x00, 0, 0xF0, 0xF0, 0x00, 0x00], 0x100, 2);
        assert_eq!(
            attribute(&mut chip, |c| c.previous[0], 600)
                == attribute(&mut still, |c| c.previous[0], 600),
            fb == 0
        );
        if fb != 0 {
            let swing = (2.0 * 2042.0) / 2f64.powi(8 - fb as i32); // the most the chip adds, in 1/1024 periods
            assert!((swing * 2.0 * PI / 1024.0 - printed).abs() / printed < 0.01);
            // ...and the chip does add it: the first sample where the two differ is
            // the first where the shifted sum of the previous two outputs is not 0.
            let options = Tone {
                f_number: 0x100,
                block: 2,
                key: 0,
                ..Tone::default()
            };
            let mut chip = tone_chip(&tone, options.clone());
            let mut still = tone_chip(&[0x21, 0x21, 0x00, 0, 0xF0, 0xF0, 0, 0], options);
            for each in [&mut chip, &mut still] {
                note(each, 0, 0x100, 2);
            }
            let a = attribute(&mut chip, |c| c.previous[0], 400);
            let b = attribute(&mut still, |c| c.previous[0], 400);
            let first = (0..400).find(|&i| a[i] != b[i]).unwrap() as isize;
            let shift = 8 - fb as i32;
            // Python's b[first - 1] counts from the end when the index is negative, and
            // `first` is 0 here (see docs/status.md, "Suspected errors in the reference"):
            // kept as Python reads it.
            let at = |i: isize| b[i.rem_euclid(b.len() as isize) as usize];
            assert_ne!((at(first - 1) + at(first - 2)) >> shift, 0);
            assert!((2..first).all(|i| (at(i - 1) + at(i - 2)) >> shift == 0));
        }
    }
}

// ------------------------------------------------------- Table III-7: decay

// S1 Table III-7 (page 13), left half: effective rate -> ms from 0 dB to 48 dB.
#[rustfmt::skip]
const DECAY_MS: [(i32, f64); 60] = [
    (63, 1.27), (62, 1.27), (61, 1.27), (60, 1.27), (59, 1.47), (58, 1.71), (57, 2.05), (56, 2.55),
    (55, 2.94), (54, 3.42), (53, 4.10), (52, 5.11), (51, 5.87), (50, 6.84), (49, 8.21), (48, 10.22),
    (47, 11.75), (46, 13.68), (45, 16.41), (44, 20.44), (43, 23.49), (42, 27.36), (41, 32.83),
    (40, 40.87), (39, 46.99), (38, 54.71), (37, 65.65), (36, 81.74), (35, 93.97), (34, 109.42),
    (33, 131.31), (32, 163.49), (31, 187.95), (30, 218.84), (29, 262.61), (28, 326.98),
    (27, 375.90), (26, 437.69), (25, 525.22), (24, 653.95), (23, 751.79), (22, 875.37),
    (21, 1050.45), (20, 1307.91), (19, 1503.58), (18, 1750.75), (17, 2100.89), (16, 2615.82),
    (15, 3007.16), (14, 3501.49), (13, 4201.79), (12, 5231.64), (11, 6014.32), (10, 7002.98),
    (9, 8403.58), (8, 10463.30), (7, 12028.66), (6, 14006.00), (5, 16807.20), (4, 20926.60),
];

fn decay_ms(rate: i32) -> f64 {
    DECAY_MS.iter().find(|&&(r, _)| r == rate).unwrap().1
}

/// Samples from level 0, with the shared counter at 0, to level 127.
///
/// A percussive tone with SL 0 leaves decay at once and falls at RR (S1 page 12:
/// "Attenuation times of the release rate are the same as that of the decay
/// rate"), all the way to the bottom.
fn decay_samples(rate: i32) -> u64 {
    let tone = [0x00, 0x01, 0x3F, 0x00, 0x00, 0xF0, 0x00, (rate >> 2) as u8];
    let mut chip = struck(&tone, 0x40, 2 * (rate & 3));
    chip.run(SAMPLE); // damp ends and the instant attack puts the level at 0
    assert_eq!(chip.level[C[0]], 0);
    chip.counter = (1 << 24) - 1; // the next sample is counter 0
    chip.run(SAMPLE);
    let mut count = 0; // the first sample is time 0
    let stride = 1u64 << 0.max(13 - (rate >> 2)); // the level can only move this often
    while chip.level[C[0]] != 127 {
        chip.run(SAMPLE * stride);
        chip.take_outputs();
        count += stride;
    }
    count
}

#[test]
fn test_table_iii_7_decay_times() {
    // Spec 8.9. From counter 0 the model's 127th step falls one opportunity
    // earlier than the printed time (validation.md: "allowing the one early first
    // step"): the printed figures are whole opportunities, 127, 102, 85 and 73
    // pairs of them for rate & 3 = 0 to 3.
    let mut rates: Vec<i32> = DECAY_MS.iter().map(|&(r, _)| r).collect();
    rates.sort();
    for rate in rates {
        let opportunity = 1u64 << 0.max(13 - (rate >> 2));
        let printed = decay_ms(rate);
        let model = (decay_samples(rate) + opportunity) as f64 * SAMPLE_MS;
        if rate < 52 {
            // The table is printed to 0.01 ms and its longest rows differ from this
            // arithmetic by 0.02 ms in 20,000: two parts in a million.
            assert!(
                (model - printed).abs() < 0.011 + printed * 2e-6,
                "{model} {printed}"
            );
        } else {
            // The fastest rows: the table's rule of proportion against the chip's
            // measured 16-sample cycles (spec 8.7). Within a sample and a half.
            assert!(
                (model - printed).abs() < 1.5 * SAMPLE_MS,
                "{model} {printed}"
            );
        }
    }
}

#[test]
fn test_table_iii_7_rates_below_4_never_change() {
    // S1 page 13: 'When RATE is "0", the envelope will not change.' (R = 0.)
    assert!(decay_step_never(0));
}

fn decay_step_never(release: u8) -> bool {
    let tone = [0x00, 0x01, 0x3F, 0x00, 0x00, 0xF0, 0x00, release];
    let mut chip = pitched(&tone, 0x100, 7);
    levels(&mut chip, C[0], 9_000)[5..].iter().all(|&v| v == 0)
}

// ------------------------------------------------------ Table III-7: attack

// S1 Table III-7 (page 13), right half: effective rate -> ms from 48 dB to 0 dB.
#[rustfmt::skip]
const ATTACK_MS: [(i32, f64); 56] = [
    (59, 0.14), (58, 0.18), (57, 0.22), (56, 0.28), (55, 0.30), (54, 0.34), (53, 0.42), (52, 0.50),
    (51, 0.54), (50, 0.60), (49, 0.70), (48, 0.84), (47, 0.97), (46, 1.13), (45, 1.37), (44, 1.69),
    (43, 1.93), (42, 2.25), (41, 2.74), (40, 3.38), (39, 3.86), (38, 4.51), (37, 5.47), (36, 6.76),
    (35, 7.72), (34, 9.01), (33, 10.94), (32, 13.52), (31, 15.45), (30, 18.02), (29, 21.88),
    (28, 27.03), (27, 30.90), (26, 36.04), (25, 43.77), (24, 54.07), (23, 61.79), (22, 72.09),
    (21, 87.54), (20, 108.13), (19, 123.58), (18, 144.18), (17, 175.07), (16, 216.27),
    (15, 247.16), (14, 288.36), (13, 350.15), (12, 432.54), (11, 494.33), (10, 576.72),
    (9, 700.30), (8, 865.08), (7, 988.66), (6, 1153.43), (5, 1400.60), (4, 1730.15),
];

/// Samples of attack from level 127 to level 0, the shared counter being
/// `counter` at the first.
fn attack_samples(rate: i32, counter: i32) -> i32 {
    let tone = [
        0x00,
        0x21,
        0x3F,
        0x00,
        0x00,
        ((rate >> 2) << 4) as u8,
        0x00,
        0x0F,
    ];
    let mut chip = struck(&tone, 0x40, 2 * (rate & 3));
    chip.run(SAMPLE); // the sample in which damp ends (the level is already 127)
    assert_eq!(chip.level[C[0]], 127);
    chip.counter = (counter - 1).rem_euclid(1 << 24);
    let mut count = 0;
    while chip.level[C[0]] != 0 {
        chip.run(SAMPLE);
        count += 1;
    }
    count
}

#[test]
fn test_table_iii_7_attack_times() {
    // Spec 8.15: up to rate 47 the printed times are 21, 17, 14 and 12 times a
    // power of two samples. The measured attack (spec 8.10, 8.11) takes 42 steps,
    // four to an opportunity, which is those multiples to within one opportunity.
    // Spec 8.12: from rate 48 the level moves every sample, and the printed times
    // are the step counts of the measured formulas (42, 25, 14 ... 7 samples).
    let mut table = ATTACK_MS.to_vec();
    table.sort_by_key(|&(rate, _)| rate);
    for (rate, printed) in table {
        if rate < 48 {
            let model = attack_samples(rate, 0);
            let opportunity = (1 << (13 - (rate >> 2))) as f64;
            assert_eq!(
                (printed / SAMPLE_MS / opportunity).round_ties_even() as i32,
                [21, 17, 14, 12][(rate & 3) as usize]
            );
            assert!((model as f64 * SAMPLE_MS - printed).abs() <= opportunity * SAMPLE_MS + 0.006);
        } else {
            // How long a mixed rate takes depends on where the shared counter is
            // when the attack starts. The printed time lies among the times the model
            // can take.
            let model: Vec<i32> = (0..16)
                .map(|counter| attack_samples(rate, counter))
                .collect();
            let (low, high) = (*model.iter().min().unwrap(), *model.iter().max().unwrap());
            assert!(
                low as f64 - 1.5 <= printed / SAMPLE_MS && printed / SAMPLE_MS <= high as f64 + 1.5,
                "{model:?}"
            );
        }
    }
}

#[test]
fn test_table_iii_7_attack_rate_15_takes_no_time() {
    // S1 page 13: 0.00 ms for rates 60 to 63.
    let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x0F];
    let mut chip = struck(&tone, 0x100, 4);
    assert_eq!(levels(&mut chip, C[0], 3)[0], 0);
}

// ------------------------------------------------- the envelope figure III-1

#[test]
fn test_figure_iii_1_a_sustained_tone_holds_and_a_percussive_one_falls() {
    // S1 III-1-2 and Figure III-1 (pages 9, 10); III-1-5 (page 12): "With
    // percussive tone, it is the critical point of change from decay mode to
    // release mode."
    let mut held = plain(&[0x00, 0x21, 0x3F, 0x00, 0x00, 0xFC, 0x00, 0x28]);
    let mut falling = plain(&[0x00, 0x01, 0x3F, 0x00, 0x00, 0xFC, 0x00, 0x28]);
    assert_eq!(*levels(&mut held, C[0], 6000).last().unwrap(), 16); // SL 2: 6 dB, 16 steps
    assert!(*levels(&mut falling, C[0], 6000).last().unwrap() > 60);
}

#[test]
fn test_sus_on_makes_the_release_rate_5() {
    // S1 III-1-6 (page 14): 'When this bit is "1", the RR with the key off will
    // be 5.' Rate 5 x 4 = 20 is printed as 1307.91 ms (Figure III-1: "RS: 1.2 s").
    let tone = [0x00, 0x21, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x0F];
    let mut chip = pitched(&tone, 0x40, 0);
    chip.run(SAMPLE * 10);
    note_ks(&mut chip, 0, 0x40, 0, 0, 1);
    let mut took = 0;
    while chip.level[C[0]] != 127 {
        chip.run(SAMPLE * 64);
        chip.take_outputs();
        took += 64;
    }
    assert!(
        (took as f64 * SAMPLE_MS - decay_ms(20)).abs() < 25.0,
        "{took}"
    );
}

#[test]
fn test_figure_iii_1_damp_takes_10_ms() {
    // S1 Figure III-1 (page 10): "DP: 10 ms (0 - 48 dB)". Table III-7 has 10.22 ms
    // at rate 48, which is base rate 12 (measured: S4, see test_measured).
    assert_eq!(decay_ms(4 * YM2413.damp_rate), 10.22);
    assert_eq!(decay_ms(4 * YM2413.percussive_release_rate), 326.98); // "RR': 310 ms"
}

// ---------------------------------------------------------- the two LFOs

#[test]
fn test_tremolo_is_3_7_hz() {
    // S1 III-1-2 (page 10): "The frequency is 3.7 Hz".
    let mut chip = plain(&[0x21, 0xA1, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00]);
    // Python's `attribute(chip, "tremolo", 0, 0) or [chip.tremolo]` reads nothing.
    let mut seen = Vec::new();
    for _ in 0..3 * 13_440 {
        chip.run(SAMPLE);
        seen.push(chip.tremolo);
    }
    chip.take_outputs();
    let period = (1..20_000)
        .find(|&p| seen[..13_440] == seen[p..p + 13_440])
        .unwrap();
    assert_eq!(py_round(CLOCK_HZ as f64 / 72.0 / period as f64, 1), 3.7);
}

#[test]
fn test_vibrato_is_6_07_hz_not_the_printed_6_4() {
    // S1 III-1-2 (page 10) says 6.4 Hz. Measured: a period of 8,192 samples,
    // 6.07 Hz (S4 20151201.txt). The measurement is taken (contradiction C2), and
    // this test would fail on S1's figure on purpose.
    let mut chip = pitched(&[0x21, 0x61, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00], 0x1C0, 6);
    let mut steps = Vec::new();
    let mut last = chip.phase[C[0]];
    for _ in 0..3 * 8192 {
        chip.run(SAMPLE);
        steps.push((chip.phase[C[0]] - last).rem_euclid(1 << 19));
        last = chip.phase[C[0]];
    }
    chip.take_outputs();
    let period = (1..10_000)
        .find(|&p| steps[..8192] == steps[p..p + 8192])
        .unwrap();
    assert_eq!(py_round(CLOCK_HZ as f64 / 72.0 / period as f64, 2), 6.07);
    assert_ne!(py_round(CLOCK_HZ as f64 / 72.0 / period as f64, 1), 6.4);
}

// ----------------------------------------------------- slots and the DAC

fn heard_melody(trace: &[Vec<(i32, i32)>]) -> BTreeSet<usize> {
    let mut heard = BTreeSet::new();
    for sample in trace {
        for (slot, &(melody_word, _)) in sample.iter().enumerate() {
            if melody_word != 0 && melody_word != -1 {
                heard.insert(slot);
            }
        }
    }
    heard
}

#[test]
fn test_table_ii_3_channels_come_out_in_their_carrier_slots() {
    // S1 Table II-3 (page 5) and Figure III-3: channel n is output in the slot of
    // its carrier: slots 4, 5, 6, 10, 11, 12, 16, 17, 18 counting from 1.
    for channel in 0..9u8 {
        let mut chip = OPLL::new(YM2413);
        user_tone(&mut chip, &[0x21, 0x21, 0x3F, 0x00, 0xF0, 0xF0, 0x00, 0x00]);
        note(&mut chip, channel, 0x155, 5);
        let heard = heard_melody(&words(&mut chip, 300));
        assert_eq!(heard, BTreeSet::from([C[channel as usize]]));
    }
}

const RHYTHM_SETUP: [(u8, u8); 6] = [
    (0x16, 0x20),
    (0x17, 0x50),
    (0x18, 0xC0),
    (0x26, 0x05),
    (0x27, 0x05),
    (0x28, 0x01),
];

fn rhythm_chip(keys: u8) -> OPLL {
    let mut chip = OPLL::new(YM2413);
    for (register, value) in RHYTHM_SETUP {
        // S1 Table III-9 (page 16), right half
        write(&mut chip, register, value);
    }
    write(&mut chip, 0x0E, 0x20);
    write(&mut chip, 0x0E, 0x20 | keys);
    chip
}

// S1 Table III-9 (page 16), slots counted from 1: BD 13 and 16, SD 17, TOM 15,
// TOP-CYM 18, HH 14. The sound is heard from its last slot.
#[test]
fn test_table_iii_9_rhythm_slots_and_the_second_output() {
    // S1 III-4 (page 19): "the same percussion sounds are output twice".
    for (bit, slot) in [(4, 15), (3, 16), (2, 14), (1, 17), (0, 13)] {
        let mut chip = rhythm_chip(1 << bit);
        let copy = match slot {
            13 => 0,
            14 => 1,
            15 => 2,
            16 => 6,
            _ => 7,
        };
        let trace = words(&mut chip, 400);
        let mut heard = BTreeSet::new();
        for sample in &trace {
            for (index, &(_, rhythm_word)) in sample.iter().enumerate() {
                if rhythm_word != 0 && rhythm_word != -1 {
                    heard.insert(index);
                }
            }
        }
        assert_eq!(heard, BTreeSet::from([slot, copy]));
        for pair in trace.windows(2) {
            let (earlier, later) = (&pair[0], &pair[1]);
            assert_eq!(later[copy].1, earlier[slot].1);
        }
        assert!(trace
            .iter()
            .all(|sample| sample.iter().all(|&(melody_word, _)| melody_word == 0)));
    }
}

#[test]
fn test_rhythm_mode_leaves_six_melody_channels() {
    // S1 III-1-7 (page 16): "In this mode, the melody section is limited to six
    // sounds."
    let mut chip = rhythm_chip(0);
    for channel in 0..9u8 {
        write(&mut chip, 0x30 + channel, 0x10);
        note(&mut chip, channel, 0x155, 5);
    }
    let heard = heard_melody(&words(&mut chip, 300));
    assert_eq!(heard, C[..6].iter().copied().collect());
    assert_eq!(M[6], 12);
}

#[test]
fn test_half_wave_rectification() {
    // S1 III-1-3 (page 12): "The carrier wave is rectified to half wave."
    let mut tone = PLAIN_SINE;
    tone[3] = 0x10;
    let out = melody(&mut pitched(&tone, 256, 4), 1280);
    assert!(*out.iter().max().unwrap() == 255 && *out.iter().min().unwrap() == 0);
    assert!(out.iter().filter(|&&value| value == 0).count() >= 640);
}
