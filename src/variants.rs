//! The two parts this core models, as data.
//!
//! Section numbers are those of ym2413-python's `docs/spec.md`; S, C, U and T numbers are its
//! sources, contradictions and unknowns and the hand-off brief's technical defaults.
//! Each field says where its value comes from and how far it can be trusted:
//!
//! - *documented*: a manufacturer's or community document says so;
//! - *measured*: a capture from a real chip says so;
//! - *PROVISIONAL*: no source above an emulator settles it; the value is the
//!   hand-off brief's default, and a measurement would change the value, not the
//!   code.
//!
//! No logic anywhere asks which variant it is running: it reads these fields.
//!
//! The YM2413's fifteen melodic voices below are **not a dump of the chip**. They are
//! the table of emu2413 v1.6.4, an emulator, adjusted by hand by its author, and are
//! used as data under that project's licence:
//!
//! ```text
//!     The MIT License (MIT)
//!
//!     Copyright (c) 2001-2019 Mitsutaka Okazaki
//!
//!     Permission is hereby granted, free of charge, to any person obtaining a copy
//!     of this software and associated documentation files (the "Software"), to deal
//!     in the Software without restriction, including without limitation the rights
//!     to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//!     copies of the Software, and to permit persons to whom the Software is
//!     furnished to do so, subject to the following conditions:
//!
//!     The above copyright notice and this permission notice shall be included in all
//!     copies or substantial portions of the Software.
//!
//!     THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//!     IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//!     FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//!     AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//!     LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//!     OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//!     SOFTWARE.
//! ```
//!
//! A transcription of ym2413-python's `variants.py` (at f88dccf): same names, same order, same
//! comments. Where Rust forced a difference:
//!
//! - `OPLLVariant` is a frozen dataclass in Python; here it is a struct whose two values,
//!   `YM2413` and `DS1001`, are `const`s, so they cannot change. A new variant is
//!   `OPLLVariant { field: value, ..YM2413 }` (Python's `dataclasses.replace(YM2413, ...)`).
//!   Python's field defaults are written out in `OPLLVariant::new`.
//! - `name` is a `Cow<'static, str>` so that the `const`s borrow a literal while a variant
//!   restored from a save state (the `serde` feature) owns its name. It is the only field that is
//!   not `Copy`, so `OPLLVariant` is `Clone`.
//! - The `percussion` property is a method.

use std::borrow::Cow;

/// Eight register bytes, $00 to $07.
pub type Row = [u8; 8];

/// The YM2413's melodic voices 1 to 15, as register bytes $00 to $07.
/// PROVISIONAL, emulator tier, hand-tuned, NOT A DUMP (T1; spec 5.4, U1, C9).
/// Source: emu2413 v1.6.4 (Mitsutaka Okazaki, MIT; notice above), `emu2413.c`
/// at commit a1d5120, the first table, rows 1 to 15. That file does not say where
/// the rows came from; its commit titles ("Improve ROM instruments", "Tweak ROM
/// voice parameters", "Fix piano attack rate") show a table adjusted by hand over
/// time. No allowed source holds a read-out of the YM2413's instrument ROM.
pub const YM2413_VOICES: [Row; 15] = [
    [0x71, 0x61, 0x1E, 0x17, 0xD0, 0x78, 0x00, 0x17], // 1 Violin
    [0x13, 0x41, 0x1A, 0x0D, 0xD8, 0xF7, 0x23, 0x13], // 2 Guitar
    [0x13, 0x01, 0x99, 0x00, 0xF2, 0xC4, 0x21, 0x23], // 3 Piano
    [0x11, 0x61, 0x0E, 0x07, 0x8D, 0x64, 0x70, 0x27], // 4 Flute
    [0x32, 0x21, 0x1E, 0x06, 0xE1, 0x76, 0x01, 0x28], // 5 Clarinet
    [0x31, 0x22, 0x16, 0x05, 0xE0, 0x71, 0x00, 0x18], // 6 Oboe
    [0x21, 0x61, 0x1D, 0x07, 0x82, 0x81, 0x11, 0x07], // 7 Trumpet
    [0x33, 0x21, 0x2D, 0x13, 0xB0, 0x70, 0x00, 0x07], // 8 Organ
    [0x61, 0x61, 0x1B, 0x06, 0x64, 0x65, 0x10, 0x17], // 9 Horn
    [0x41, 0x61, 0x0B, 0x18, 0x85, 0xF0, 0x81, 0x07], // 10 Synthesizer
    [0x33, 0x01, 0x83, 0x11, 0xEA, 0xEF, 0x10, 0x04], // 11 Harpsichord
    [0x17, 0xC1, 0x24, 0x07, 0xF8, 0xF8, 0x22, 0x12], // 12 Vibraphone
    [0x61, 0x50, 0x0C, 0x05, 0xD2, 0xF5, 0x40, 0x42], // 13 Synthesizer Bass
    [0x01, 0x01, 0x55, 0x03, 0xE9, 0x90, 0x03, 0x02], // 14 Acoustic Bass
    [0x41, 0x41, 0x89, 0x03, 0xF1, 0xE4, 0xC0, 0x13], // 15 Electric Guitar
];

/// The DS1001's voices 1 to 15: the chip's own ROM. Documented from a read-out
/// (spec 5.5; docs/restart.md rule 11). Chain: the NESdev wiki, "VRC7 audio",
/// section "Internal patch set" ("dumped using the VRC7 rewired into a debug
/// mode"), which cites the page "vendor:yamaha:opl2" on siliconpr0n.org (not
/// readable from here: it refuses automated fetches). The wiki does not name who
/// made the read-out; emu2413's source credits the same bytes as "VRC7 presets
/// ... by Nuke.YKT". The bytes are taken from the wiki's table, and
/// `tests/community.rs` compares them with the pinned wiki file.
pub const DS1001_VOICES: [Row; 15] = [
    [0x03, 0x21, 0x05, 0x06, 0xE8, 0x81, 0x42, 0x27], // 1 Buzzy Bell
    [0x13, 0x41, 0x14, 0x0D, 0xD8, 0xF6, 0x23, 0x12], // 2 Guitar
    [0x11, 0x11, 0x08, 0x08, 0xFA, 0xB2, 0x20, 0x12], // 3 Wurly
    [0x31, 0x61, 0x0C, 0x07, 0xA8, 0x64, 0x61, 0x27], // 4 Flute
    [0x32, 0x21, 0x1E, 0x06, 0xE1, 0x76, 0x01, 0x28], // 5 Clarinet
    [0x02, 0x01, 0x06, 0x00, 0xA3, 0xE2, 0xF4, 0xF4], // 6 Synth
    [0x21, 0x61, 0x1D, 0x07, 0x82, 0x81, 0x11, 0x07], // 7 Trumpet
    [0x23, 0x21, 0x22, 0x17, 0xA2, 0x72, 0x01, 0x17], // 8 Organ
    [0x35, 0x11, 0x25, 0x00, 0x40, 0x73, 0x72, 0x01], // 9 Bells
    [0xB5, 0x01, 0x0F, 0x0F, 0xA8, 0xA5, 0x51, 0x02], // 10 Vibes
    [0x17, 0xC1, 0x24, 0x07, 0xF8, 0xF8, 0x22, 0x12], // 11 Vibraphone
    [0x71, 0x23, 0x11, 0x06, 0x65, 0x74, 0x18, 0x16], // 12 Tutti
    [0x01, 0x02, 0xD3, 0x05, 0xC9, 0x95, 0x03, 0x02], // 13 Fretless
    [0x61, 0x63, 0x0C, 0x00, 0x94, 0xC0, 0x33, 0xF6], // 14 Synth Bass
    [0x21, 0x72, 0x0D, 0x00, 0xC1, 0xD5, 0x56, 0x06], // 15 Sweep
];

/// The three ROM rows of the percussion sounds, for channels 6, 7 and 8.
///
/// Spec 5.6. Measured as fits on a real YM2413 (S4, `rhythm.md`): the bass
/// drum's eight bytes "gave the best result" among many tried; for the other four
/// the bytes that matter were fitted to the captured envelopes ("pretty good
/// match"). In the second row the even bytes are the hi-hat's (modulator slot)
/// and the odd bytes the snare's; in the third, the tom's and the top cymbal's.
/// Bytes $02 and $03 of those rows do not matter to S4's fits and are 0, as in
/// the read-out of the DS1001's ROM (S6), whose three rows equal these.
pub const fn percussion_rows(snare_last_byte: u8) -> [Row; 3] {
    [
        [0x01, 0x01, 0x18, 0x0F, 0xDF, 0xF8, 0x6A, 0x6D],
        [0x01, 0x01, 0x00, 0x00, 0xC8, 0xD8, 0xA7, snare_last_byte],
        [0x05, 0x01, 0x00, 0x00, 0xF8, 0xAA, 0x59, 0x55],
    ]
}

const fn vibrato_offsets() -> [[i32; 8]; 8] {
    let mut table = [[0; 8]; 8];
    let mut top = 0;
    while top < 8 {
        table[top as usize] = [0, top >> 1, top, top >> 1, 0, -(top >> 1), -top, -(top >> 1)];
        top += 1;
    }
    table
}

/// Vibrato: the offset added to twice the F-number, by the F-number's top three
/// bits (rows) and the vibrato position (columns). Spec 7.8.
/// The last row is measured, approximately (S4 `20151201.txt`: "I only verified
/// the last row", by aligning generated sines with a capture).
/// The other seven rows are PROVISIONAL, emulator tier (T3, U4): emu2413's
/// `pm_table` (MIT), which its own comment calls a "rough approximation of 14
/// cents depth". The rule that generates all eight rows: the peak is the top three
/// bits, the half-way value is the peak shifted right once.
pub const VIBRATO_OFFSETS: [[i32; 8]; 8] = vibrato_offsets();

/// What differs between the parts, and every unsettled value a measurement
/// might change. Configuration: nothing here changes at run time.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OPLLVariant {
    /// A label for messages and reports. Nothing compares against it.
    pub name: Cow<'static, str>,

    /// Melodic voices 1 to 15 as register bytes $00 to $07. See YM2413_VOICES and
    /// DS1001_VOICES for the very different standing of the two sets.
    pub voices: [Row; 15],

    /// Channels that make sound: 9 on the YM2413 (documented: spec 2.3, S1), 6 on
    /// the DS1001 (documented, tier 3: spec 1, S6).
    pub channels: usize,

    /// Whether the percussion output (the RO pin) exists. Documented (spec 12.1,
    /// S1); the DS1001 has none (spec 11.14, S6: "The VRC7 has no rhythm DAC").
    pub rhythm_output: bool,

    /// Whether rhythm mode is on whatever $0E says. Documented, tier 3, for the
    /// DS1001 (S6: the bit "is treated as though it were always enabled").
    /// With `channels` at 6 and no rhythm output this means channels 6 to 8 are
    /// never heard: their registers are accepted and their operators are not run.
    /// PROVISIONAL (T15, U16).
    pub rhythm_forced: bool,

    /// Byte $07 of the snare's ROM row: its sustain level and release rate. $68 is
    /// the only measured value (S4's fit on a YM2413; the DS1001's read-out has
    /// $68 too). The NESdev wiki and ymfm say the YM2413 has $48 (contradiction
    /// C4). PROVISIONAL on the YM2413 (T2).
    pub snare_last_byte: u8,

    /// Vibrato offsets (see VIBRATO_OFFSETS): top row measured, the rest
    /// PROVISIONAL, emulator tier (T3).
    pub vibrato_offsets: [[i32; 8]; 8],

    /// The bits of the test register $0F that do something. Documented at tier 3
    /// for the VRC7 (spec 13.2, S6: "responds to the low 4 bits"). For the YM2413
    /// Yamaha says only that the register exists: PROVISIONAL (T4, U2).
    pub test_bits: u8,

    /// The hi-hat's phase, in 1/1024 periods, for (cymbal bit, noise bit) =
    /// (0, 0), (0, 1), (1, 0), (1, 1). PROVISIONAL, emulator tier (T8, U22;
    /// spec 11.10): emu2413's four constants (MIT). S4's measured values are
    /// printed only in a code block, which was not read. emu2413 pairs the noise
    /// values the other way round, with a snare whose noise is also the other way
    /// round from S4's measured table; the pairing here is emu2413's read through
    /// that one inversion.
    pub hi_hat_phases: [i32; 4],

    /// The fixed base rates of the damp phase, of release with the SUS bit set,
    /// and of a percussive tone's release without it. Measured: spec 8.16 and 8.22
    /// (S4 `envelope.txt`); S1's Figure III-1 gives them as times (10 ms,
    /// 1.2 s, 310 ms).
    pub damp_rate: i32,
    pub sustained_release_rate: i32,
    pub percussive_release_rate: i32,

    /// Whether `reset()` clears the vibrato position. On the VRC7 the reset bit
    /// clears the registers "including tremolo LFO state, but not including
    /// vibrato LFO state" (documented, tier 3: spec 14.2, S6). For the YM2413's
    /// /IC pin nothing is known beyond "all registers are cleared": PROVISIONAL
    /// (T5, U3).
    pub reset_clears_vibrato: bool,

    /// The noise register after reset. Any value but 0 would do (an all-zero
    /// register never leaves zero: spec 14.3). PROVISIONAL (T5, U3).
    pub reset_noise: i32,
}

impl OPLLVariant {
    /// A variant with Python's field defaults: the name and the voices are required.
    pub const fn new(name: &'static str, voices: [Row; 15]) -> Self {
        OPLLVariant {
            name: Cow::Borrowed(name),
            voices,
            channels: 9,
            rhythm_output: true,
            rhythm_forced: false,
            snare_last_byte: 0x68,
            vibrato_offsets: VIBRATO_OFFSETS,
            test_bits: 0x0F,
            hi_hat_phases: [0x2D0, 0x234, 0x034, 0x0D0],
            damp_rate: 12,
            sustained_release_rate: 5,
            percussive_release_rate: 7,
            reset_clears_vibrato: true,
            reset_noise: 1,
        }
    }

    /// The ROM rows of channels 6, 7 and 8 in rhythm mode.
    pub const fn percussion(&self) -> [Row; 3] {
        percussion_rows(self.snare_last_byte)
    }
}

/// Yamaha's YM2413 (OPLL): the Mark III FM Sound Unit, the Japanese Master System.
pub const YM2413: OPLLVariant = OPLLVariant::new("YM2413", YM2413_VOICES);

/// The FM section of Konami's VRC7 (the Famicom's *Lagrange Point*).
pub const DS1001: OPLLVariant = {
    let mut variant = OPLLVariant::new("DS1001", DS1001_VOICES);
    variant.channels = 6;
    variant.rhythm_output = false;
    variant.rhythm_forced = true;
    variant.reset_clears_vibrato = false;
    variant
};
