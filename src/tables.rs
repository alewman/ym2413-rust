//! The chip's fixed tables, each with its source and standing.
//!
//! Section numbers ("spec 6.3") are those of ym2413-python's `docs/spec.md`; S numbers are the
//! sources of its `docs/phase0.md`; its `docs/provenance-audit.md` has each statement's chain.
//! Nothing here differs between the YM2413 and the DS1001: what differs is in `variants.rs`.
//!
//! A transcription of ym2413-python's `tables.py` (at f88dccf): same names, same order, same
//! comments. Where Rust forced a difference:
//!
//! - Python builds `LOG_SIN`, `EXP` and `KEY_SCALE_STEPS` at import from floating-point
//!   formulas. Rust cannot evaluate those in a `const`, so they are built from the same formulas,
//!   in the same order of operations, once on first use (`tables()`, a `OnceLock`). Python's
//!   `round()` rounds halves to even: `f64::round_ties_even` here. The tables are compared with
//!   the reference's, value for value, by rung 2 (`scripts/rung2.sh`).
//! - `TREMOLO` and `SLOT_IS_CARRIER` need only integers and are `const` arrays built by
//!   `const fn`s.
//! - `COPY_SLOT` is a Python dict; here it is an 18-long array of `Option`s indexed by slot
//!   (`slot in COPY_SLOT` is `COPY_SLOT[slot].is_some()`).

use std::sync::OnceLock;

/// The tables Python computes at import with floating point (see the module header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    /// `LOG_SIN`
    pub log_sin: [i32; 256],
    /// `EXP`
    pub exp: [i32; 256],
    /// `KEY_SCALE_STEPS`
    pub key_scale_steps: [[[i32; 16]; 8]; 4],
}

static TABLES: OnceLock<Tables> = OnceLock::new();

/// The computed tables, built on first use.
#[inline]
pub fn tables() -> &'static Tables {
    TABLES.get_or_init(|| Tables {
        log_sin: log_sin(),
        exp: exp(),
        key_scale_steps: key_scale_steps(),
    })
}

/// The logarithmic sine ROM: a quarter period in 256 steps, each the negated
/// base-2 logarithm of the sine in 1/256 units, taken at the middle of the step.
/// Spec 6.3. The formula is Matthew Gambrell's and Olli Niemitalo's, who read the
/// ROMs of a YM3812 and a YMF262 bit by bit under a microscope ("OPLx
/// decapsulated", 2008: S11); Wouter Vermaelen (S4, `tab.txt`) found the OPLL's
/// ROM alike on the die and predicted captured waveforms from it exactly for
/// volumes 1 to 15. Measured for those chips; measured indirectly for this one.
fn log_sin() -> [i32; 256] {
    let mut table = [0; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = (-((i as f64 + 0.5) * std::f64::consts::PI / 512.0)
            .sin()
            .log2()
            * 256.0)
            .round_ties_even() as i32;
    }
    table
}

/// The exponential ROM: 256 steps of 2^(i/256), scaled by 1,024, with the bit
/// that is always set left out. Spec 6.4; same sources and standing as LOG_SIN.
fn exp() -> [i32; 256] {
    let mut table = [0; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = ((2f64.powf(i as f64 / 256.0) - 1.0) * 1024.0).round_ties_even() as i32;
    }
    table
}

/// Twice the frequency multiplier for each MULTI value (so that the 1/2 of value
/// 0 is a whole number). Documented: spec 7.4, Yamaha's application manual (S1),
/// Table III-1. Measured as doubled values in the phase step: spec 7.3 (S4).
pub const MULTIPLIER_X2: [i32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

/// Attenuation in dB at 3 dB per octave in the top octave, by the F-number's top
/// four bits. Documented: spec 9.2, S1 Table III-5, bottom row, read from the
/// page image.
#[rustfmt::skip]
pub const KEY_SCALE_DB: [f64; 16] = [
    0.000, 9.000, 12.000, 13.875, 15.000, 16.125, 16.875, 17.625,
    18.000, 18.750, 19.125, 19.500, 19.875, 20.250, 20.625, 21.000,
];

/// An envelope step in dB. Measured: spec 8.1 (S4: 128 steps in 48 dB). S1 prints
/// "0,325 dB" (contradiction C1).
pub const ENVELOPE_STEP_DB: f64 = 0.375;

/// KEY_SCALE_STEPS[code][block][F-number's top four bits], in envelope steps.
///
/// Spec 9.2 to 9.4. The 6 dB setting is twice S1's row less 6 dB per octave below
/// the top, floored at zero (S1's notes to Table III-5); the 3 dB setting is that
/// halved and the 1.5 dB setting quartered, dropping fractions (spec 9.3:
/// measured on a sparse sample by S4, `ksl.txt`). Code 1 is 1.5 dB, code 2 is
/// 3 dB, code 3 is 6 dB: measured (S4), against one reading of S1's Table III-4
/// (contradiction C3).
fn key_scale_steps() -> [[[i32; 16]; 8]; 4] {
    let mut six_db = [[0i32; 16]; 8];
    for (block, row) in six_db.iter_mut().enumerate() {
        for (entry, db) in row.iter_mut().zip(KEY_SCALE_DB) {
            *entry = 0.max(
                (2.0 * db / ENVELOPE_STEP_DB).round_ties_even() as i32 - 16 * (7 - block as i32),
            );
        }
    }
    let shifts = [None, Some(2), Some(1), Some(0)];
    let mut table = [[[0i32; 16]; 8]; 4];
    for (code, shift) in shifts.iter().enumerate() {
        for block in 0..8 {
            for (top, &steps) in six_db[block].iter().enumerate() {
                table[code][block][top] = match shift {
                    None => 0,
                    Some(shift) => steps >> shift,
                };
            }
        }
    }
    table
}

/// For effective rates 4 to 51: where in each run of eight opportunities the
/// envelope steps, by the rate's low two bits. Measured: spec 8.5. S4
/// (`adsr-transitions.txt`) fixed each pattern and its rotation by listing the
/// first-segment lengths that occur on the chip and ruling out every other
/// candidate. S4 first met the scheme in a GPL emulator and says so; the
/// measurement is the source here (docs/restart.md, rule 10).
pub const ENVELOPE_PATTERNS: [[i32; 8]; 4] = [
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
];

/// Samples in one tremolo cycle, and the measured lengths that make it up
/// (spec 10.2, S4 `am.txt`): 960 samples at 0, 512 at each of 1 to 12 going up
/// and again coming down, 192 at 13.
pub const TREMOLO_PERIOD: i32 = 13_440;

/// Samples in one vibrato cycle: eight positions of 1,024 samples. Measured:
/// spec 7.7 (S4 `20151201.txt`). S1 says 6.4 Hz, which would be 7,768
/// (contradiction C2).
pub const VIBRATO_PERIOD: i32 = 8_192;

/// Envelope steps the tremolo adds, 0 to 13, at a place in its cycle.
///
/// Spec 10.1 and 10.2: measured (S4). CHOICE: the cycle is taken to begin where
/// the level becomes 0; nothing says how it lies against anything else (U15).
pub const fn tremolo_level(sample_in_cycle: i32) -> i32 {
    let step = sample_in_cycle >> 6; // every measured length is a multiple of 64
    if step < 15 {
        return 0;
    }
    if step < 111 {
        return 1 + ((step - 15) >> 3);
    }
    if step < 114 {
        return 13;
    }
    12 - ((step - 114) >> 3)
}

const fn tremolo_table() -> [i32; (TREMOLO_PERIOD >> 6) as usize] {
    let mut table = [0; (TREMOLO_PERIOD >> 6) as usize];
    let mut step = 0;
    while step < table.len() {
        table[step] = tremolo_level((step as i32) << 6);
        step += 1;
    }
    table
}

/// TREMOLO[step]: the 210 steps of 64 samples, from the rule above.
pub const TREMOLO: [i32; (TREMOLO_PERIOD >> 6) as usize] = tremolo_table();

/// Which slot is which operator: the modulators and carriers of channels 0 to 8.
/// Documented: spec 2.4 (S1 Table II-3); measured (S4 `rhythm.md`).
pub const MODULATOR_SLOT: [usize; 9] = [0, 1, 2, 6, 7, 8, 12, 13, 14];
pub const CARRIER_SLOT: [usize; 9] = [3, 4, 5, 9, 10, 11, 15, 16, 17];
pub const SLOT_CHANNEL: [usize; 18] = [0, 1, 2, 0, 1, 2, 3, 4, 5, 3, 4, 5, 6, 7, 8, 6, 7, 8];

const fn slot_is_carrier() -> [bool; 18] {
    let mut table = [false; 18];
    let mut channel = 0;
    while channel < 9 {
        table[CARRIER_SLOT[channel]] = true;
        channel += 1;
    }
    table
}

pub const SLOT_IS_CARRIER: [bool; 18] = slot_is_carrier();

/// Rhythm mode: the slots of the five sounds (spec 11.2: S1 Table III-9, S4).
pub const SLOT_BASS_MODULATOR: usize = 12;
pub const SLOT_HI_HAT: usize = 13;
pub const SLOT_TOM: usize = 14;
pub const SLOT_BASS_DRUM: usize = 15;
pub const SLOT_SNARE: usize = 16;
pub const SLOT_CYMBAL: usize = 17;

/// Rhythm mode: the slot of the next sample in which each sound's word is output
/// a second time (spec 11.12, 12.2: S1 Figure III-3; measured by S4).
#[rustfmt::skip]
pub const COPY_SLOT: [Option<usize>; 18] = [
    Some(SLOT_HI_HAT), Some(SLOT_TOM), Some(SLOT_BASS_DRUM), None, None, None,
    Some(SLOT_SNARE), Some(SLOT_CYMBAL), None, None, None, None,
    None, None, None, None, None, None,
];
