//! The chip: registers, eighteen operators, and the sequence of output words.
//!
//! Written from ym2413-python's `docs/spec.md`, not from another program. Every rule below names
//! the spec statement it implements, or says CHOICE (nothing speaks to it and this
//! core had to pick) or PROVISIONAL (an unsettled item taken at the hand-off brief's
//! default: T numbers, `docs/handoff-brief.md`). `docs/provenance-audit.md` has
//! the chain behind each statement.
//!
//! The unit of time is the slot: four input clocks, one pass through the chip's one
//! operator circuit (spec 2.3). A sample is eighteen slots. Operators are numbered
//! by slot, 0 to 17 (spec 2.4): `tables::MODULATOR_SLOT` and `tables::CARRIER_SLOT`
//! say which channel each belongs to.
//!
//! All state is plain fields (ints, arrays of ints, one byte array), so that a
//! board's save-state walker can copy it out and back. `variant` is configuration
//! and never changes.
//!
//! A transcription of ym2413-python's `opll.py` (at f88dccf): same names, same order, same
//! comments. Where Rust forced a difference:
//!
//! - Python's integers are unbounded; here the state is `i32` (every value is far inside it: the
//!   phase is 19 bits, the counter 24, the noise register 23, a phase step at most 22 bits),
//!   the registers `u8`, as in Python's `bytearray`. Operator values keep Python's semantics:
//!   `>>` of a negative `i32` is arithmetic and `!` is Python's `~`.
//! - The per-operator lists are fixed arrays (`[i32; 18]`, `[i32; 9]`).
//! - `write` takes `u8`s: Python masks the value with `& 0xFF` and reads only bit 0 of the port,
//!   so no byte-wide caller sees a difference.
//! - Python's `reset()` keeps `vibrato_time` when it already exists and the variant says so
//!   (`hasattr`); here `new()` sets it to 0 before the first `reset()`, which is the same.
//! - `trace` is an `Option<Vec<Vec<(i32, i32)>>>`: `None` costs one branch a slot, and with
//!   `Some` it fills exactly as Python's list of lists does (a list set mid-sample stays empty
//!   until the next slot 0).
//! - `take_outputs` pairs the two lists Python zips with `strict=True`; they are always the same
//!   length here too.
//! - Booleans Python adds as integers (`melody_word < 0`, the cymbal bit) are cast to `i32`.

use crate::tables::{
    tables, CARRIER_SLOT, COPY_SLOT, ENVELOPE_PATTERNS, MODULATOR_SLOT, MULTIPLIER_X2,
    SLOT_BASS_DRUM, SLOT_CHANNEL, SLOT_CYMBAL, SLOT_HI_HAT, SLOT_IS_CARRIER, SLOT_SNARE, SLOT_TOM,
    TREMOLO, TREMOLO_PERIOD, VIBRATO_PERIOD,
};
use crate::variants::{OPLLVariant, Row};

/// Input clocks in one slot (spec 2.3: documented, S1; measured, S4).
pub const CLOCKS_PER_CYCLE: u64 = 4;

/// Slots in one output sample: 72 input clocks (spec 2.2, 2.3).
pub const CYCLES_PER_SAMPLE: usize = 18;

// Envelope states of a keyed operator (spec 8.16 to 8.21). "Release" is not a
// state: it is simply "the key is off" (spec 8.22).
pub const DAMP: i32 = 0;
pub const ATTACK: i32 = 1;
pub const DECAY: i32 = 2;
pub const SUSTAIN: i32 = 3;

/// The envelope level at and above which an operator is silent and at which the
/// damp phase ends: its top five bits set (spec 6.8, 8.17: measured, S4).
pub const QUIET: i32 = 124;

/// The largest envelope level and the largest total attenuation (spec 8.1, 6.7).
pub const BOTTOM: i32 = 127;

// Phases, in 1/1024 periods, that make the snare's and the cymbal's four output
// values out of the ordinary sine look-up: the positive peak, the negative peak,
// and the two zeros (spec 11.8, 11.9).
pub const PHASE_PLUS_ZERO: i32 = 0x000;
pub const PHASE_PLUS_PEAK: i32 = 0x100;
pub const PHASE_MINUS_ZERO: i32 = 0x200;
pub const PHASE_MINUS_PEAK: i32 = 0x300;

/// How far the envelope level rises this sample at an effective rate.
///
/// `counter` is the chip's shared sample counter (spec 8.4).
pub fn decay_step(rate: i32, counter: i32) -> i32 {
    if rate < 4 {
        return 0; // spec 8.6: documented (S1 Table III-7 note), measured
    }
    if rate < 52 {
        // Spec 8.5 (measured): an opportunity when the counter's low s bits are
        // zero; the next three bits pick a place in the rate's pattern.
        let s = 13 - (rate >> 2);
        if counter & ((1 << s) - 1) != 0 {
            return 0;
        }
        return ENVELOPE_PATTERNS[(rate & 3) as usize][((counter >> s) & 7) as usize];
    }
    if rate >= 60 {
        return 2; // spec 8.8: measured
    }
    // Spec 8.7 (measured): a 16-sample cycle in which 0, 4, 8 or 12 samples go at
    // the faster of two speeds. CHOICE (U23): the faster samples are those where
    // counter bits 2 and 3 pick a 1 from the even places of the rate's pattern.
    let faster = ENVELOPE_PATTERNS[(rate & 3) as usize][((counter >> 1) & 6) as usize];
    if rate < 56 {
        return if faster != 0 { 1 } else { counter & 1 };
    }
    if faster != 0 {
        2
    } else {
        1
    }
}

/// The envelope level after one sample of attack at an effective rate.
pub fn attack_step(rate: i32, counter: i32, level: i32) -> i32 {
    if !(4..60).contains(&rate) {
        // Spec 8.13 (measured): with the rate at 0, or at the instant setting once
        // an attack is under way, the attack stalls.
        return level;
    }
    if rate < 48 {
        // Spec 8.11 (measured): as decay, but the counter's lowest two bits are
        // ignored, so an opportunity lasts four samples; spec 8.10: each applies
        // x -> x - (x >> 4) - 1.
        let s = 13 - (rate >> 2);
        if (counter >> 2) & ((1 << (s - 2)) - 1) != 0 {
            return level;
        }
        if ENVELOPE_PATTERNS[(rate & 3) as usize][((counter >> s) & 7) as usize] == 0 {
            return level;
        }
        return level - (level >> 4) - 1;
    }
    // Spec 8.12 (measured): every sample; the shift is 4, 3, 2 for rates 48, 52,
    // 56, and one less on 4, 8 or 12 samples of 16 for the rates between.
    // CHOICE (U23): which samples, as in decay_step.
    let mut shift = 4 - ((rate - 48) >> 2);
    if ENVELOPE_PATTERNS[(rate & 3) as usize][((counter >> 1) & 6) as usize] != 0 {
        shift -= 1;
    }
    level - (level >> shift) - 1
}

/// A YM2413 or a DS1001, chosen by `variant` (ym2413-python's `docs/api.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OPLL {
    pub variant: OPLLVariant,

    // Where in the 72-clock sample the chip is. CHOICE (U9): a sample is taken
    // to begin with slot 0; no source says where it begins as seen from the
    // pins. Not touched by reset().
    /// input clocks into the current slot, 0 to 3
    pub clock_phase: u64,
    /// the next slot to compute, 0 to 17
    pub slot: usize,

    // Finished samples, and the sums of the sample in progress (T6).
    pub melody: Vec<i32>,
    pub rhythm: Vec<i32>,
    pub melody_sum: i32,
    pub rhythm_sum: i32,

    /// For the validation harness: set it to `Some(vec![])` and each sample appends
    /// its eighteen (melody word, rhythm word) pairs, the words in the chip's
    /// own form (one's complement: -1 is "minus zero"). `None` costs nothing.
    pub trace: Option<Vec<Vec<(i32, i32)>>>,

    // Set by reset().
    pub address: u8,
    #[cfg_attr(feature = "serde", serde(with = "bytes64"))]
    pub registers: [u8; 0x40],
    pub test: u8,

    // One entry per operator, indexed by slot.
    /// 19 bits: 10 index the sine, 9 are fraction (spec 7.2)
    pub phase: [i32; 18],
    /// envelope level, 0 loudest (spec 8.1)
    pub level: [i32; 18],
    pub state: [i32; 18],
    pub key: [i32; 18],

    // The two previous outputs of each channel's modulator (spec 6.12).
    pub previous: [i32; 9],
    pub before_previous: [i32; 9],

    /// the shared sample counter (spec 8.4)
    pub counter: i32,
    /// samples into the tremolo cycle (spec 10.2)
    pub tremolo_time: i32,
    /// samples into the vibrato cycle (spec 7.7)
    pub vibrato_time: i32,
    /// this sample's tremolo level, 0 to 13
    pub tremolo: i32,
    /// 23 stages (spec 11.7)
    pub noise: i32,

    // Each percussion word as last computed, for its second output (spec 11.12).
    pub held: [i32; 18],

    // What each operator's instrument says, decoded from the registers and the
    // ROM whenever a write changes them. Derived, and saved like the rest.
    pub tremolo_on: [i32; 18],
    pub vibrato_on: [i32; 18],
    /// EG-TYP
    pub sustained: [i32; 18],
    /// KSR
    pub rate_scaling: [i32; 18],
    /// twice the multiplier
    pub multiplier: [i32; 18],
    /// the KSL code
    pub key_scale: [i32; 18],
    /// attenuation from TL or the volume, in envelope steps
    pub base: [i32; 18],
    pub half_sine: [i32; 18],
    pub attack_rate: [i32; 18],
    pub decay_rate: [i32; 18],
    pub sustain_level: [i32; 18],
    pub release_rate: [i32; 18],
    /// whether key-off puts the operator into release
    pub releases: [i32; 18],
    pub feedback: [i32; 9],
    pub rhythm_mode: i32,
}

impl OPLL {
    pub fn new(variant: OPLLVariant) -> Self {
        let mut chip = OPLL {
            variant,
            // Where in the 72-clock sample the chip is. CHOICE (U9): a sample is taken
            // to begin with slot 0; no source says where it begins as seen from the
            // pins. Not touched by reset().
            clock_phase: 0,
            slot: 0,
            // Finished samples, and the sums of the sample in progress (T6).
            melody: Vec::new(),
            rhythm: Vec::new(),
            melody_sum: 0,
            rhythm_sum: 0,
            trace: None,
            // Everything below is set by reset().
            address: 0,
            registers: [0; 0x40],
            test: 0,
            phase: [0; 18],
            level: [0; 18],
            state: [0; 18],
            key: [0; 18],
            previous: [0; 9],
            before_previous: [0; 9],
            counter: 0,
            tremolo_time: 0,
            vibrato_time: 0,
            tremolo: 0,
            noise: 0,
            held: [0; 18],
            tremolo_on: [0; 18],
            vibrato_on: [0; 18],
            sustained: [0; 18],
            rate_scaling: [0; 18],
            multiplier: [0; 18],
            key_scale: [0; 18],
            base: [0; 18],
            half_sine: [0; 18],
            attack_rate: [0; 18],
            decay_rate: [0; 18],
            sustain_level: [0; 18],
            release_rate: [0; 18],
            releases: [0; 18],
            feedback: [0; 9],
            rhythm_mode: 0,
        };
        chip.reset();
        chip
    }

    // ------------------------------------------------------------------ reset

    /// The /IC pin, or the VRC7's sound-reset bit.
    ///
    /// Spec 14.1 (documented, S1): every register is cleared. Everything else
    /// here is PROVISIONAL (T5, U3): no source says what the envelopes, phases,
    /// counters and noise register hold afterwards.
    pub fn reset(&mut self) {
        self.address = 0;
        self.registers = [0; 0x40];
        self.test = 0;

        // One entry per operator, indexed by slot.
        self.phase = [0; 18]; // 19 bits: 10 index the sine, 9 are fraction (spec 7.2)
        self.level = [BOTTOM; 18]; // envelope level, 0 loudest (spec 8.1)
        self.state = [SUSTAIN; 18];
        self.key = [0; 18];

        // The two previous outputs of each channel's modulator (spec 6.12).
        self.previous = [0; 9];
        self.before_previous = [0; 9];

        self.counter = 0; // the shared sample counter (spec 8.4)
        self.tremolo_time = 0; // samples into the tremolo cycle (spec 10.2)
        if self.variant.reset_clears_vibrato {
            self.vibrato_time = 0; // samples into the vibrato cycle (spec 7.7)
        }
        self.tremolo = 0; // this sample's tremolo level, 0 to 13
        self.noise = self.variant.reset_noise; // 23 stages (spec 11.7)

        // Each percussion word as last computed, for its second output (spec 11.12).
        self.held = [0; 18];

        // What each operator's instrument says, decoded from the registers and the
        // ROM whenever a write changes them. Derived, and saved like the rest.
        self.tremolo_on = [0; 18];
        self.vibrato_on = [0; 18];
        self.sustained = [0; 18]; // EG-TYP
        self.rate_scaling = [0; 18]; // KSR
        self.multiplier = [1; 18]; // twice the multiplier
        self.key_scale = [0; 18]; // the KSL code
        self.base = [0; 18]; // attenuation from TL or the volume, in envelope steps
        self.half_sine = [0; 18];
        self.attack_rate = [0; 18];
        self.decay_rate = [0; 18];
        self.sustain_level = [0; 18];
        self.release_rate = [0; 18];
        self.releases = [0; 18]; // whether key-off puts the operator into release
        self.feedback = [0; 9];
        self.rhythm_mode = 0;
        self.decode_everything();
    }

    // ----------------------------------------------------------------- writes

    /// A write with A0 = `port`: 0 latches an address, 1 stores data there.
    ///
    /// Spec 3.1, 3.2 (documented, S1): the address stays latched. PROVISIONAL
    /// (T7, U9): a data write acts from the next slot boundary (see `run`);
    /// the waits the manual asks for are not modelled.
    pub fn write(&mut self, port: u8, value: u8) {
        if port & 1 == 0 {
            self.address = value;
            return;
        }
        let mut address = self.address as usize;
        if address >= 0x40 {
            return; // PROVISIONAL (T7; spec 4.4): no source; ignored
        }
        let mut low = address & 0x0F;
        if address >= 0x10 && low >= 9 {
            // Spec 4.3 (measured, S4 ksl.txt): $19-$1F, $29-$2F and $39-$3F act as
            // $10-$16, $20-$26 and $30-$36.
            address -= 9;
            low -= 9;
        }
        if address < 0x08 {
            self.registers[address] = value; // the user tone (spec 4, 5.1)
            for channel in 0..9 {
                self.decode_channel(channel);
            }
        } else if address == 0x0E {
            self.registers[address] = value & 0x3F;
            self.decode_everything();
        } else if address == 0x0F {
            // Spec 13: documented for the VRC7 at tier 3 (S6); PROVISIONAL on the
            // YM2413 (T4).
            self.registers[address] = value;
            self.test = value & self.variant.test_bits;
        } else if address < 0x10 {
            // PROVISIONAL (T7; spec 4.4): $08-$0D, no source; ignored
        } else if address < 0x20 {
            self.registers[address] = value; // F-number bits 7-0
        } else if address < 0x30 {
            self.registers[address] = value & 0x3F; // spec 4.2: bits 7-6 unused
            self.decode_keys();
        } else {
            self.registers[address] = value;
            self.decode_channel(low);
        }
    }

    fn decode_everything(&mut self) {
        // Spec 11.1 (documented): bit 5 of $0E is rhythm mode. PROVISIONAL (T10,
        // U8): switching it changes only which settings and which keys the six
        // operators of channels 6 to 8 use; their envelopes and phases carry on.
        self.rhythm_mode = if self.variant.rhythm_forced {
            1
        } else {
            ((self.registers[0x0E] >> 5) & 1) as i32
        };
        for channel in 0..9 {
            self.decode_channel(channel);
        }
        self.decode_keys();
    }

    /// Work out both operators' settings for a channel (spec 4, 5, 11.11).
    fn decode_channel(&mut self, channel: usize) {
        let registers = &self.registers;
        let (modulator, carrier) = (MODULATOR_SLOT[channel], CARRIER_SLOT[channel]);
        let percussion = self.rhythm_mode != 0 && channel >= 6;
        let voice = (registers[0x30 + channel] >> 4) as i32;
        let volume = (registers[0x30 + channel] & 0x0F) as i32;
        let row: Row = if percussion {
            self.variant.percussion()[channel - 6] // spec 5.3, 5.6
        } else if voice == 0 {
            // spec 5.1: the user tone in $00-$07
            let mut user = [0; 8];
            user.copy_from_slice(&registers[..8]);
            user
        } else {
            self.variant.voices[(voice - 1) as usize]
        };

        for (operator, which) in [(modulator, 0), (carrier, 1)] {
            let flags = row[which] as i32;
            self.tremolo_on[operator] = flags >> 7;
            self.vibrato_on[operator] = (flags >> 6) & 1;
            self.sustained[operator] = (flags >> 5) & 1;
            self.rate_scaling[operator] = (flags >> 4) & 1;
            self.multiplier[operator] = MULTIPLIER_X2[(flags & 0x0F) as usize];
            self.key_scale[operator] = (row[2 + which] >> 6) as i32;
            self.attack_rate[operator] = (row[4 + which] >> 4) as i32;
            self.decay_rate[operator] = (row[4 + which] & 0x0F) as i32;
            self.sustain_level[operator] = (row[6 + which] >> 4) as i32;
            self.release_rate[operator] = (row[6 + which] & 0x0F) as i32;
        }
        self.half_sine[modulator] = ((row[3] >> 3) & 1) as i32; // DM
        self.half_sine[carrier] = ((row[3] >> 4) & 1) as i32; // DC
        self.feedback[channel] = (row[3] & 0x07) as i32;

        // Spec 6.7: a modulator's base attenuation is its total level, 0.75 dB a
        // step, two envelope steps; a carrier's is the volume, 3 dB a step, eight.
        self.base[modulator] = 2 * (row[2] & 0x3F) as i32;
        self.base[carrier] = 8 * volume;
        // Spec 8.23 (measured): a modulator does not change rate at key-off.
        self.releases[modulator] = 0;
        self.releases[carrier] = 1;
        if percussion && channel >= 7 {
            // The hi-hat and the tom are single operators on modulator slots.
            // They "always act like a carrier" (S4 rhythm.md, prose). Their volume
            // is the high nibble of $37 and $38 (spec 4, S1's second map).
            // PROVISIONAL (T12, U11): eight envelope steps a volume step, and no
            // total level.
            self.base[modulator] = 8 * voice;
            self.releases[modulator] = 1;
        }
    }

    /// Each operator's key, and the start of a note where one goes on.
    fn decode_keys(&mut self) {
        let registers = &self.registers;
        let rhythm = if self.rhythm_mode != 0 {
            registers[0x0E] as i32
        } else {
            0
        };
        for channel in 0..9 {
            let on = ((registers[0x20 + channel] >> 4) & 1) as i32;
            let (mut modulator_on, mut carrier_on) = (on, on);
            if rhythm != 0 && channel >= 6 {
                // PROVISIONAL (T9, U7; spec 11.4, measured loosely): in rhythm
                // mode each sound is keyed by its $0E bit OR its channel's key bit.
                if channel == 6 {
                    // bass drum
                    modulator_on = on | ((rhythm >> 4) & 1);
                    carrier_on = modulator_on;
                } else if channel == 7 {
                    modulator_on = on | (rhythm & 1); // hi-hat
                    carrier_on = on | ((rhythm >> 3) & 1); // snare
                } else {
                    modulator_on = on | ((rhythm >> 2) & 1); // tom
                    carrier_on = on | ((rhythm >> 1) & 1); // top cymbal
                }
            }
            for (operator, now) in [
                (MODULATOR_SLOT[channel], modulator_on),
                (CARRIER_SLOT[channel], carrier_on),
            ] {
                if now != 0 && self.key[operator] == 0 {
                    // Spec 8.16 (measured): key off to key on enters damp.
                    // PROVISIONAL (T14, U13): modulators and percussion too.
                    self.state[operator] = DAMP;
                }
                self.key[operator] = now;
            }
        }
    }

    // ------------------------------------------------------------------- time

    /// Advance by input clocks. `run(a); run(b)` equals `run(a + b)`.
    ///
    /// PROVISIONAL (T7, U9): a slot is computed as its last clock passes, so a
    /// write lands at the next slot boundary: it acts on the slot it falls in and
    /// on every later one. A sample is complete on every 72nd clock.
    pub fn run(&mut self, clocks: u64) {
        let total = self.clock_phase + clocks;
        let slots = total / CLOCKS_PER_CYCLE;
        self.clock_phase = total % CLOCKS_PER_CYCLE;
        for _ in 0..slots {
            self.run_slot();
        }
    }

    /// The samples finished since the last take, as (melody, rhythm) pairs.
    ///
    /// CHOICE (T6; spec section 12): the chip's fact is a train of nine-bit
    /// words, one per slot, on two pins. Each pair here is the sum of one
    /// sample's words on the melody pin and on the rhythm pin (each percussion
    /// word appears twice, as the chip outputs it: spec 11.12). A word counts as
    /// its signed size, -255 to +255; the two zeros both count as 0.
    pub fn take_outputs(&mut self) -> Vec<(i32, i32)> {
        let melody = std::mem::take(&mut self.melody);
        let rhythm = std::mem::take(&mut self.rhythm);
        melody.into_iter().zip(rhythm).collect()
    }

    /// The same samples as one number each: melody plus rhythm (T6).
    pub fn take_samples(&mut self) -> Vec<i32> {
        self.take_outputs()
            .into_iter()
            .map(|(melody, rhythm)| melody + rhythm)
            .collect()
    }

    // ------------------------------------------------------------- one sample

    /// What happens once a sample: the shared counter and the two LFOs.
    fn begin_sample(&mut self) {
        let test = self.test;
        self.counter = (self.counter + 1) & 0xFFFFFF; // spec 8.4; width is U19
        if test & 0x02 != 0 {
            // Spec 13.2 (S6, tier 3): bit 1 holds both LFOs at zero.
            self.tremolo_time = 0;
            self.vibrato_time = 0;
        } else {
            // Spec 13.2: bit 3 makes the LFOs move every sample: the tremolo 64
            // times faster and the vibrato 1,024 times faster.
            // CHOICE (U15, U19): the LFOs are counted apart from the envelope
            // counter, since the VRC7's reset clears one and not the other.
            let fast = test & 0x08 != 0;
            self.tremolo_time = (self.tremolo_time + if fast { 64 } else { 1 }) % TREMOLO_PERIOD;
            self.vibrato_time = (self.vibrato_time + if fast { 1024 } else { 1 }) % VIBRATO_PERIOD;
        }
        self.tremolo = TREMOLO[(self.tremolo_time >> 6) as usize]; // spec 10.2 (measured)
    }

    fn run_slot(&mut self) {
        let mut slot = self.slot;
        if slot == 0 {
            self.begin_sample();
        }
        let channel = SLOT_CHANNEL[slot];
        let percussion = self.rhythm_mode != 0 && channel >= 6;
        let mut melody_word = 0;
        let mut rhythm_word = 0;

        if self.rhythm_mode != 0 && self.variant.rhythm_output {
            if let Some(copy) = COPY_SLOT[slot] {
                // Spec 11.12 (documented and measured): each percussion word is
                // output a second time, unchanged, in a slot of the next sample.
                rhythm_word = self.held[copy];
            }
        }

        if channel < self.variant.channels || (percussion && self.variant.rhythm_output) {
            self.step_envelope(slot, channel);
            self.step_phase(slot, channel);
            if !percussion {
                let word = self.melodic_word(slot, channel);
                if SLOT_IS_CARRIER[slot] {
                    melody_word = word;
                }
            } else {
                let word = self.percussion_word(slot);
                if slot != 12 {
                    // the bass drum's modulator is not heard
                    self.held[slot] = word;
                    rhythm_word = word;
                }
            }
        }

        // Spec 11.7 (measured): the noise register is a 23-stage shift register
        // with polynomial x^23 + x^9 + 1, stepped once per slot.
        let noise = self.noise;
        self.noise = (noise >> 1) | (((noise ^ (noise >> 14)) & 1) << 22);

        // Spec 12.3: the words are not added inside the chip. CHOICE (T6): the
        // core hands out their sum over a sample; a negative word ~m counts -m.
        self.melody_sum += melody_word + (melody_word < 0) as i32;
        self.rhythm_sum += rhythm_word + (rhythm_word < 0) as i32;
        if let Some(trace) = &mut self.trace {
            if slot == 0 {
                trace.push(Vec::with_capacity(CYCLES_PER_SAMPLE));
            }
            if let Some(sample) = trace.last_mut() {
                sample.push((melody_word, rhythm_word));
            }
        }

        slot += 1;
        if slot == CYCLES_PER_SAMPLE {
            slot = 0;
            self.melody.push(self.melody_sum);
            self.rhythm.push(self.rhythm_sum);
            self.melody_sum = 0;
            self.rhythm_sum = 0;
        }
        self.slot = slot;
    }

    // --------------------------------------------------------------- envelope

    /// Spec 8.2, 8.3 (documented, S1 III-1-2 and Table III-2; measured).
    fn effective_rate(&self, operator: usize, channel: usize, rate: i32) -> i32 {
        if rate == 0 {
            return 0;
        }
        let high = self.registers[0x20 + channel] as i32;
        let block_and_top = high & 0x0F; // BLOCK and F-number bit 8
        let scaling = if self.rate_scaling[operator] != 0 {
            block_and_top
        } else {
            block_and_top >> 2
        };
        63.min(4 * rate + scaling)
    }

    fn step_envelope(&mut self, operator: usize, channel: usize) {
        let level = self.level[operator];
        let counter = self.counter;

        if !SLOT_IS_CARRIER[operator] && !(self.rhythm_mode != 0 && channel >= 7) {
            // Spec 7.10: the carrier's damp ends this sample (keyed, in damp, at
            // 124 or more), so this channel's phases restart together.
            let carrier = CARRIER_SLOT[channel];
            if self.key[carrier] != 0 && self.state[carrier] == DAMP && self.level[carrier] >= QUIET
            {
                self.phase[operator] = 0;
            }
        }

        if self.key[operator] == 0 && self.releases[operator] != 0 {
            // Spec 8.22 (measured and documented): release is "the key is off",
            // whatever state the operator was in. Spec 8.24: it runs to 127.
            let mut rate = if (self.registers[0x20 + channel] >> 5) & 1 != 0 {
                self.variant.sustained_release_rate // the SUS bit: rate 5
            } else if self.sustained[operator] != 0 {
                self.release_rate[operator]
            } else {
                self.variant.percussive_release_rate // rate 7
            };
            rate = self.effective_rate(operator, channel, rate);
            // PROVISIONAL (T13, U14): a step past the bottom stops at 127.
            self.level[operator] = BOTTOM.min(level + decay_step(rate, counter));
            return;
        }

        let mut state = self.state[operator];
        if state == DAMP {
            if level >= QUIET {
                // Spec 8.17 (measured): damp ends at 124, not 127. Spec 7.10
                // (measured): "both of a channel's phase counters are set to zero
                // when the envelope leaves the damp state". The envelope S4 saw
                // is the carrier's, so the carrier's damp end resets both; a
                // modulator's own damp end resets nothing, since a modulator held
                // through key-off (spec 8.23) damps from a low level and would
                // otherwise restart out of step with its carrier, against S4's
                // "each time the same note is played, the wave looks exactly the
                // same". CHOICE (U20): the reset belongs to the sample in which
                // the carrier's damp ends, before either operator steps; the
                // modulator's slot comes first, so it resets its own phase when
                // it sees the carrier about to end damp (see the top of this
                // method). The single-operator percussion sounds (hi-hat, tom,
                // snare, cymbal) have their own keys and each resets only its
                // own phase.
                if SLOT_IS_CARRIER[operator] || (self.rhythm_mode != 0 && channel >= 7) {
                    self.phase[operator] = 0;
                }
                let rate = self.effective_rate(operator, channel, self.attack_rate[operator]);
                if rate >= 60 {
                    // Spec 8.13 (measured): the instant attack is decided here.
                    self.level[operator] = 0;
                    self.state[operator] = DECAY;
                } else {
                    self.state[operator] = ATTACK;
                }
                return;
            }
            // Spec 8.16 (measured): damp is a decay at fixed rate 12, key-scaled.
            let rate = self.effective_rate(operator, channel, self.variant.damp_rate);
            self.level[operator] = BOTTOM.min(level + decay_step(rate, counter));
            return;
        }

        if state == ATTACK {
            if level == 0 {
                state = DECAY; // spec 8.18
                self.state[operator] = state;
            } else {
                let rate = self.effective_rate(operator, channel, self.attack_rate[operator]);
                self.level[operator] = attack_step(rate, counter, level);
                return;
            }
        }

        if state == DECAY {
            // Spec 8.19 (measured): decay ends when the level's top four bits
            // equal SL: equality, not "at or beyond".
            if level >> 3 == self.sustain_level[operator] {
                self.state[operator] = SUSTAIN;
            } else {
                let rate = self.effective_rate(operator, channel, self.decay_rate[operator]);
                self.level[operator] = BOTTOM.min(level + decay_step(rate, counter));
                return;
            }
        }

        // Spec 8.20 (measured): sustain is one state with two rates. A sustained
        // tone holds; a percussive one goes on at RR. Spec 8.21: SL plays no part.
        if self.sustained[operator] == 0 {
            let rate = self.effective_rate(operator, channel, self.release_rate[operator]);
            self.level[operator] = BOTTOM.min(level + decay_step(rate, counter));
        }
    }

    // ------------------------------------------------------------------ phase

    fn step_phase(&mut self, operator: usize, channel: usize) {
        if self.test & 0x04 != 0 {
            self.phase[operator] = 0; // spec 13.2 (S6): bit 2 holds every phase at zero
            return;
        }
        let registers = &self.registers;
        let high = registers[0x20 + channel] as i32;
        let f_number = registers[0x10 + channel] as i32 | ((high & 1) << 8);
        let block = (high >> 1) & 7;
        let multiplier = self.multiplier[operator];
        let step = if self.vibrato_on[operator] != 0 {
            // Spec 7.8: measured approximately for the top pitch range,
            // PROVISIONAL below it (T3, U4).
            let offset = self.variant.vibrato_offsets[(f_number >> 6) as usize]
                [(self.vibrato_time >> 10) as usize];
            (((2 * f_number + offset) * multiplier) << block) >> 2
        } else {
            // Spec 7.3 (measured): ((F-number x twice the multiplier) << block) >> 1.
            ((f_number * multiplier) << block) >> 1
        };
        // Spec 7.2, 7.3: 19 bits; what overflows is lost. Spec 7.6: a step of 0
        // leaves the phase where it is.
        self.phase[operator] = (self.phase[operator] + step) & 0x7FFFF;
    }

    // ----------------------------------------------------------------- output

    /// One pass through the operator circuit (spec section 6).
    ///
    /// `phase` is in 1/1024 periods. The result is the eleven-bit value v of
    /// spec 6.5 in one's complement: -1 is "minus zero" (spec 6.6).
    fn operate(&self, operator: usize, channel: usize, phase: i32) -> i32 {
        let mut level = self.level[operator];
        if self.test & 0x01 != 0 {
            level = 0; // spec 13.2 (S6): bit 0 replaces the envelope's output by 0
        }
        if level >= QUIET {
            return 0; // spec 6.8 (measured): a quiet operator outputs +0
        }
        let tables = tables();

        // Spec 6.2 (measured): a quarter is stored; the second quarter reads it
        // backwards and the second half changes the sign.
        let mut index = phase & 0xFF;
        if phase & 0x100 != 0 {
            index ^= 0xFF;
        }
        let negative = phase & 0x200 != 0;

        // Spec 6.7 (measured): attenuation in envelope steps, clipped at 127.
        let high = self.registers[0x20 + channel] as i32;
        let f_number_top = ((high & 1) << 3) | (self.registers[0x10 + channel] >> 5) as i32;
        let mut attenuation = self.base[operator]
            + tables.key_scale_steps[self.key_scale[operator] as usize][((high >> 1) & 7) as usize]
                [f_number_top as usize]
            + level;
        if self.tremolo_on[operator] != 0 {
            attenuation += self.tremolo; // spec 10.1 (measured)
        }
        attenuation = attenuation.min(BOTTOM);

        // Spec 6.5 (measured): each envelope step is 16 units of the logarithm.
        let x = tables.log_sin[index as usize] + (attenuation << 4);
        let mut value = (tables.exp[((x & 0xFF) ^ 0xFF) as usize] + 1024) >> (x >> 8);
        if negative {
            if self.half_sine[operator] != 0 {
                value = 0; // spec 6.9 (measured): size zero, the sign still set
            }
            return !value;
        }
        value
    }

    fn melodic_word(&mut self, operator: usize, channel: usize) -> i32 {
        let mut phase = self.phase[operator] >> 9;
        if !SLOT_IS_CARRIER[operator] {
            let feedback = self.feedback[channel];
            if feedback != 0 {
                // Spec 6.12: the sum of the modulator's two previous outputs
                // (measured, S4), shifted right by 8 - FB (documented: S1 Table
                // III-6). PROVISIONAL (U24): the shift of a negative sum rounds
                // down.
                phase += (self.previous[channel] + self.before_previous[channel]) >> (8 - feedback);
            }
            let value = self.operate(operator, channel, phase & 0x3FF);
            self.before_previous[channel] = self.previous[channel];
            self.previous[channel] = value;
            return 0;
        }
        // Spec 6.11 (measured): the carrier's phase is moved by twice the
        // modulator's output. PROVISIONAL (T11; spec 6.13, U21): the output used
        // is the previous sample's, which gives the one-sample lag S4 measured.
        // By this slot the modulator has already run this sample, so that output
        // is the older of the two remembered.
        phase += 2 * self.before_previous[channel];
        // Spec 6.10 (measured): a carrier's word is the value shifted right by 3:
        // nine bits, -255 to +255 with both zeros.
        self.operate(operator, channel, phase & 0x3FF) >> 3
    }

    /// The one bit the top cymbal and the hi-hat share (spec 11.9).
    ///
    /// Measured (S4) as a function of five phase bits, two of C8's and three of
    /// M7's, up to sixteen forms that fit the captures equally. PROVISIONAL,
    /// emulator tier (T8, U5): emu2413's form (MIT), the first of the sixteen,
    /// on bits of the ten-bit phase. CHOICE: each sound reads the two phases as
    /// they stand at its own slot.
    fn cymbal_bit(&self) -> i32 {
        let m7 = self.phase[SLOT_HI_HAT] >> 9;
        let c8 = self.phase[SLOT_CYMBAL] >> 9;
        let c5 = (c8 >> 5) & 1;
        ((c5 == (c8 >> 3) & 1) & ((m7 >> 7) & 1 == (m7 >> 2) & 1) & (c5 == (m7 >> 3) & 1)) as i32
    }

    /// Channels 6 to 8 in rhythm mode (spec section 11).
    fn percussion_word(&mut self, operator: usize) -> i32 {
        if operator == SLOT_BASS_DRUM || operator == 12 {
            // Spec 11.5 (measured): the bass drum is an ordinary two-operator voice.
            return self.melodic_word(operator, 6);
        }
        let channel = SLOT_CHANNEL[operator];
        let noise = self.noise & 1;
        let phase = if operator == SLOT_TOM {
            // Spec 11.6 (measured): a plain sine at channel 8's pitch.
            self.phase[operator] >> 9
        } else if operator == SLOT_SNARE {
            // Spec 11.8 (measured): one phase bit and the noise bit choose among
            // four values. PROVISIONAL (T8, U5, U6): the bit is bit 8 of the
            // snare's own ten-bit phase, and the table is the first of S4's two.
            if (self.phase[operator] >> 17) & 1 != 0 {
                if noise != 0 {
                    PHASE_MINUS_ZERO
                } else {
                    PHASE_MINUS_PEAK
                }
            } else if noise != 0 {
                PHASE_PLUS_PEAK
            } else {
                PHASE_PLUS_ZERO
            }
        } else if operator == SLOT_CYMBAL {
            // Spec 11.9: a positive or negative peak (measured). PROVISIONAL,
            // emulator tier: which is which (emu2413).
            if self.cymbal_bit() != 0 {
                PHASE_PLUS_PEAK
            } else {
                PHASE_MINUS_PEAK
            }
        } else {
            // Spec 11.10, PROVISIONAL, emulator tier (U22): the hi-hat.
            self.variant.hi_hat_phases[(2 * self.cymbal_bit() + noise) as usize]
        };
        self.operate(operator, channel, phase & 0x3FF) >> 3
    }
}

/// serde derives arrays up to 32 long; the register file is 64 bytes.
#[cfg(feature = "serde")]
mod bytes64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 0x40], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 0x40], D::Error> {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("registers: 64 bytes"))
    }
}
