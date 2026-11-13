//! The provisional defaults, one labelled test each.
//!
//! These are not judges. Each pins a value that no source above an emulator settles
//! (the hand-off brief's T1 to T15, and the spec's unknowns U20, U22, U23, U24), so
//! that changing one is a deliberate act: a measurement or a document that settles
//! it changes the value, this test, and the spec's entry together. Every docstring
//! starts with PROVISIONAL and names the item.
//!
//! A transcription of `tests/test_provisional.py` (at f88dccf). `rhythm_chip` and
//! `two_operator_reference` come from `common::measured`, as Python imports them from
//! `test_measured`.

// These tests pin constants on purpose (that is their job), so clippy's objection to an
// assertion on a constant does not apply.
#![allow(clippy::assertions_on_constants)]

mod common;

use common::measured::{rhythm_chip, rhythm_chip_at, two_operator_reference};
use common::*;
use ym2413::variants::{VIBRATO_OFFSETS, YM2413_VOICES};
use ym2413::{DS1001, OPLL, YM2413};

// ------------------------------------------------------------- data defaults

/// PROVISIONAL, T1 (U1, C9): emulator tier, not a dump. Row 1 (Violin) and
/// row 15 (Electric Guitar) as emu2413 v1.6.4 has them.
#[test]
fn test_t1_the_ym2413_voices_are_emu2413s_hand_tuned_table() {
    assert!(YM2413.voices == YM2413_VOICES && YM2413.voices.len() == 15);
    assert_eq!(
        YM2413.voices[0],
        [0x71, 0x61, 0x1E, 0x17, 0xD0, 0x78, 0x00, 0x17]
    );
    assert_eq!(
        YM2413.voices[14],
        [0x41, 0x41, 0x89, 0x03, 0xF1, 0xE4, 0xC0, 0x13]
    );
}

/// PROVISIONAL, T2 (C4): $68, the only measured value; S6 and ymfm say $48
/// for the YM2413.
#[test]
fn test_t2_the_snare_byte_is_68_on_both_variants() {
    assert!(YM2413.snare_last_byte == DS1001.snare_last_byte && DS1001.snare_last_byte == 0x68);
}

/// PROVISIONAL, T3 (U4): emulator tier. Peak = the F-number's top three bits,
/// half-way = peak >> 1, mirrored negative. Only the top row was measured.
#[test]
fn test_t3_vibrato_below_the_top_row_follows_the_top_rows_rule() {
    assert_eq!(VIBRATO_OFFSETS[7], [0, 3, 7, 3, 0, -3, -7, -3]);
    assert_eq!(VIBRATO_OFFSETS[4], [0, 2, 4, 2, 0, -2, -4, -2]);
    assert_eq!(VIBRATO_OFFSETS[1], [0, 0, 1, 0, 0, 0, -1, 0]);
    assert_eq!(VIBRATO_OFFSETS[0], [0; 8]);
    assert!(
        YM2413.vibrato_offsets == DS1001.vibrato_offsets
            && DS1001.vibrato_offsets == VIBRATO_OFFSETS
    );
}

/// PROVISIONAL, T4 (U2): the four bits S6 gives for the VRC7, on the YM2413
/// too (their effects are in test_community).
#[test]
fn test_t4_the_ym2413s_test_register_is_the_vrc7s() {
    assert!(YM2413.test_bits == DS1001.test_bits && DS1001.test_bits == 0x0F);
}

// -------------------------------------------------------------------- reset

/// PROVISIONAL, T5 (U3): every envelope at 127 with its key off; phases, the
/// shared counter and both LFO positions 0; the noise register 1. The VRC7's
/// reset leaves the vibrato position alone (that part tier 3, S6).
#[test]
fn test_t5_the_state_after_reset() {
    for variant in [YM2413, DS1001] {
        let is_ym2413 = variant == YM2413;
        let mut chip = tone_chip(
            &PLAIN_SINE,
            Tone {
                variant,
                ..Tone::default()
            },
        );
        chip.run(SAMPLE * 3000);
        chip.take_outputs();
        let vibrato = chip.vibrato_time;
        chip.reset();
        assert!(chip.level == [127; 18] && chip.key == [0; 18]);
        assert_eq!(chip.phase, [0; 18]);
        assert!(chip.counter == 0 && chip.tremolo_time == 0);
        assert_eq!(chip.noise, 1);
        assert_eq!(chip.vibrato_time, if is_ym2413 { 0 } else { vibrato });
    }
}

// ---------------------------------------------------------------- the output

/// PROVISIONAL, T6: a sample's rhythm number is the sum of the rhythm pin's
/// eighteen words, so each percussion word is in it twice (spec 11.12): once
/// fresh and once as the copy in the next sample.
#[test]
fn test_t6_each_percussion_word_counts_twice_in_the_rhythm_sum() {
    let mut chip = rhythm_chip_at(0x04, 0x150, 2, 0x100, 4); // the tom alone
    chip.run(SAMPLE * 50);
    chip.take_outputs();
    chip.trace = Some(Vec::new());
    chip.run(SAMPLE * 200);
    let trace = chip.trace.take().unwrap();
    let sums: Vec<i32> = chip.take_outputs().into_iter().map(|pair| pair.1).collect();
    assert_eq!(trace.len(), sums.len()); // Python's zip(..., strict=True)
    for i in 1..trace.len() {
        let (sample, total, previous) = (&trace[i], sums[i], &trace[i - 1]);
        let (tom, copy) = (sample[14].1, sample[1].1);
        assert_eq!(copy, previous[14].1);
        assert_eq!(total, (tom + (tom < 0) as i32) + (copy + (copy < 0) as i32));
    }
}

/// PROVISIONAL, T7 (U9): a data write acts from the next slot boundary, so a
/// write two clocks into slot 3 changes slot 3's word in that same sample.
#[test]
fn test_t7_a_write_acts_from_the_slot_it_falls_in() {
    let (mut quiet, mut loud) = (pitched(&PLAIN_SINE, 256, 1), pitched(&PLAIN_SINE, 256, 1));
    loud.run(SAMPLE * 10);
    quiet.run(SAMPLE * 10);
    for chip in [&mut quiet, &mut loud] {
        chip.take_outputs();
        chip.trace = Some(Vec::new());
        chip.write(0, 0x30);
        chip.run(4 * 3 + 2); // slots 0 to 2 done, two clocks into slot 3
    }
    quiet.write(1, 0x0F); // volume 15
    for chip in [&mut quiet, &mut loud] {
        chip.run(SAMPLE - (4 * 3 + 2));
    }
    let (quiet_word, loud_word) = (
        quiet.trace.as_ref().unwrap()[0][C[0]].0,
        loud.trace.as_ref().unwrap()[0][C[0]].0,
    );
    assert_ne!(quiet_word, loud_word);
    assert!(quiet_word.abs() <= 2);
}

/// PROVISIONAL, T7 (spec 4.4): $08-$0D and $40 up change nothing.
#[test]
fn test_t7_unmapped_addresses_are_ignored() {
    let mut chip = OPLL::new(YM2413);
    for address in (0x08..0x0E).chain([0x40, 0x7F, 0xFF]) {
        write(&mut chip, address, 0xFF);
    }
    assert!(chip.registers.iter().all(|&b| b == 0));
}

// --------------------------------------------------------------- percussion

/// The cymbal bit as the tests below compute it, from the two ten-bit phases.
fn bit_of(m7: i32, c8: i32) -> bool {
    ((c8 >> 5) & 1 == (c8 >> 3) & 1)
        && ((m7 >> 7) & 1 == (m7 >> 2) & 1)
        && ((c8 >> 5) & 1 == (m7 >> 3) & 1)
}

/// PROVISIONAL, T8 (U5): emulator tier. (c85 = c83) and (m77 = m72) and
/// (c85 = m73), on bits of the ten-bit phases of C8 and M7; positive peak when
/// true. Read as each sound's phases stand at its own slot.
#[test]
fn test_t8_the_cymbals_bit_is_emu2413s_form() {
    let mut chip = rhythm_chip_at(0x02, 0x123, 4, 0x1A7, 5);
    chip.run(SAMPLE * 200);
    chip.take_outputs();
    for _ in 0..500 {
        chip.trace = Some(Vec::new());
        chip.run(SAMPLE);
        let word = chip.trace.take().unwrap()[0][17].1;
        chip.take_outputs();
        let (m7, c8) = (chip.phase[13] >> 9, chip.phase[17] >> 9);
        let bit = bit_of(m7, c8);
        assert!((word >= 0) == bit || word == 0 || word == -1);
    }
}

/// PROVISIONAL, T9 (U7): in rhythm mode each sound is keyed by its $0E bit OR
/// its channel's key bit; $28's key starts both the tom and the top cymbal.
#[test]
fn test_t9_a_channel_key_bit_keys_its_rhythm_sounds() {
    let mut chip = rhythm_chip(0);
    note_k(&mut chip, 8, 0x1C0, 0, 1);
    chip.run(SAMPLE * 50);
    chip.take_outputs();
    assert!(chip.key[14] == 1 && chip.key[17] == 1);
    assert!(chip.key[13] == 0 && chip.key[16] == 0);
}

/// PROVISIONAL, T10 (U8): only the instrument rows and routing change.
#[test]
fn test_t10_switching_rhythm_mode_keeps_envelopes_and_phases() {
    let mut chip = OPLL::new(YM2413);
    user_tone(&mut chip, &PLAIN_SINE);
    note(&mut chip, 7, 0x150, 3);
    chip.run(SAMPLE * 500);
    chip.take_outputs();
    let before = (chip.level[12..18].to_vec(), chip.state[12..18].to_vec());
    chip.write(0, 0x0E);
    chip.write(1, 0x20);
    assert_eq!(
        (chip.level[12..18].to_vec(), chip.state[12..18].to_vec()),
        before
    );
}

/// PROVISIONAL, T11 (C7, U21): the carrier is moved by the modulator's output
/// of the sample before. The same-sample reading gives other words.
#[test]
fn test_t11_the_carrier_hears_the_modulators_previous_sample() {
    let tone = [0x20, 0x20, 8, 0x00, 0xF0, 0xF0, 0x00, 0x00];
    let got: Vec<i32> = words(&mut struck(&tone, 0x40, 0), 2000)
        .iter()
        .map(|sample| sample[C[0]].0)
        .collect();
    assert_eq!(got, two_operator_reference(8, 0, 2000, 32));
    let (mut same_sample, mut phase) = (Vec::new(), 0);
    for _ in 0..2000 {
        phase += 32;
        let modulator = reference_value(phase >> 9, 16, false);
        same_sample.push(reference_value((phase >> 9) + 2 * modulator, 0, false) >> 3);
    }
    assert_ne!(got, same_sample);
}

/// PROVISIONAL, T12 (U11): base attenuation 8 x the sound's own nibble, no
/// total level, on the two single-operator sounds on modulator slots.
#[test]
fn test_t12_hi_hat_and_tom_levels_are_eight_steps_a_volume_step_no_tl() {
    let mut chip = rhythm_chip(0);
    write(&mut chip, 0x37, 0x50); // hi-hat 5, snare 0
    write(&mut chip, 0x38, 0x30); // tom 3, cymbal 0
    assert!(chip.base[13] == 40 && chip.base[14] == 24);
    write(&mut chip, 0x02, 0x3F); // the user tone's total level plays no part
    assert!(chip.base[13] == 40 && chip.base[14] == 24);
}

/// PROVISIONAL, T13 (U12, U14): the attack formula as it stands from 125
/// (125 - 7 - 1 = 117); a +2 step from 126 stops at 127.
#[test]
fn test_t13_attack_from_125_and_a_step_past_the_bottom() {
    let mut chip = struck(&[0x00, 0x21, 0x3F, 0x00, 0x00, 0xC0, 0x00, 0x0F], 0x40, 0);
    chip.run(SAMPLE);
    chip.take_outputs();
    chip.state[C[0]] = 1;
    chip.level[C[0]] = 125;
    assert_eq!(levels(&mut chip, C[0], 1), [117]); // rate 48: every sample, n = 4
    let mut chip = pitched(&[0x00, 0x31, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x0F], 0x100, 7);
    note_k(&mut chip, 0, 0x100, 7, 0); // release at RR 15 with KSR: rate 63, +2
    chip.level[C[0]] = 126;
    assert_eq!(levels(&mut chip, C[0], 3), [127, 127, 127]);
}

/// PROVISIONAL, T14 (U13): a modulator enters damp at key-on, as a carrier
/// does.
#[test]
fn test_t14_modulators_are_damped_at_key_on() {
    let tone = [0x21, 0x21, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x0F];
    let mut chip = plain(&tone);
    chip.run(SAMPLE * 100);
    note_k(&mut chip, 0, 0x100, 4, 0);
    chip.run(SAMPLE * 100);
    chip.take_outputs();
    assert_eq!(chip.level[M[0]], 0); // held through key-off (spec 8.23, measured)
    press(&mut chip, 0, 0x100, 4);
    chip.run(SAMPLE * 3);
    chip.take_outputs();
    assert!(chip.state[M[0]] == 0 && 0 < chip.level[M[0]] && chip.level[M[0]] < 124);
}

/// PROVISIONAL, T15 (U16): registers accepted, nothing output.
#[test]
fn test_t15_the_ds1001s_channels_6_to_8_take_writes_and_stay_silent() {
    let mut chip = OPLL::new(DS1001);
    user_tone(&mut chip, &PLAIN_SINE);
    note(&mut chip, 7, 0x100, 4);
    assert!(chip.registers[0x17] == 0x00 && chip.registers[0x27] == 0x19);
    assert!(melody(&mut chip, 1000).iter().all(|&v| v == 0));
}

// ----------------------------------------------------------------- unknowns

/// PROVISIONAL, U20: the phases restart, the level of an instant attack goes
/// to 0, and the sample is computed, all in the sample in which damp ends.
#[test]
fn test_u20_the_damp_end_sample_already_sounds() {
    let mut chip = struck(&[0x00, 0x24, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x00], 256, 7);
    assert_eq!(melody(&mut chip, 1), [255]); // a quarter-rate tone: first sample on the peak
}

/// PROVISIONAL, U22: emulator tier. Phases $2D0, $234, $034, $0D0 for
/// (cymbal bit, noise) = (0, 0), (0, 1), (1, 0), (1, 1).
#[test]
fn test_u22_the_hi_hats_four_phases() {
    assert_eq!(YM2413.hi_hat_phases, [0x2D0, 0x234, 0x034, 0x0D0]);
    let mut chip = rhythm_chip_at(0x01, 0x123, 4, 0x1A7, 5);
    chip.run(SAMPLE * 300);
    chip.take_outputs();
    let mut seen = std::collections::BTreeSet::new();
    let (mut noise, mut m7, mut c8, mut level) = (0, 0, 0, 0);
    for _ in 0..400 {
        chip.trace = Some(Vec::new());
        for slot in 0..18 {
            if slot == 13 {
                noise = chip.noise & 1;
            }
            chip.run(4);
            if slot == 13 {
                (m7, c8) = (chip.phase[13] >> 9, chip.phase[17] >> 9);
                level = chip.level[13];
            }
        }
        let word = chip.trace.take().unwrap()[0][13].1;
        chip.take_outputs();
        let bit = bit_of(m7, c8) as i32;
        let phase = YM2413.hi_hat_phases[(2 * bit + noise) as usize];
        assert_eq!(
            word,
            if level < 124 {
                reference_value(phase, chip.base[13] + level, false) >> 3
            } else {
                0
            }
        );
        seen.insert((bit, noise));
    }
    assert!(seen.len() >= 3);
}

/// PROVISIONAL, U23: for decay rates 52 to 59, counter bits 2 and 3 pick an
/// even place of the rate's pattern; at rate 53 (pattern 0 1 0 1 1 1 0 1, even
/// places 0 0 1 0) samples 8 to 11 of 16 go at the faster speed (one level),
/// the others at half speed.
#[test]
fn test_u23_counter_bits_2_and_3_choose_the_faster_samples() {
    let mut chip = struck(&[0x00, 0x01, 0x3F, 0x00, 0x00, 0xF0, 0x00, 0x0D], 0x40, 2);
    chip.run(SAMPLE);
    chip.take_outputs();
    chip.counter = 0;
    let trace = [vec![chip.level[C[0]]], levels(&mut chip, C[0], 64)].concat();
    for (count, w) in (1..).zip(trace.windows(2)) {
        let (before, after) = (w[0], w[1]);
        let expected = if (count >> 2) & 3 == 2 { 1 } else { count & 1 };
        assert_eq!(after - before, expected, "{count}");
    }
}

/// PROVISIONAL, U24: a negative modulator value enters the carrier's phase as
/// twice its one's complement form, and feedback shifts the sum arithmetically.
/// A sign-and-size reading gives other words.
#[test]
fn test_u24_negative_modulator_values_are_ones_complement_then_shifted() {
    let tone = [0x20, 0x20, 0, 5, 0xF0, 0xF0, 0x00, 0x00];
    let got: Vec<i32> = words(&mut struck(&tone, 0x40, 0), 4000)
        .iter()
        .map(|sample| sample[C[0]].0)
        .collect();
    assert_eq!(got, two_operator_reference(0, 5, 4000, 32));
    let (mut out, mut previous, mut before, mut phase) = (Vec::new(), 0, 0, 0);
    for _ in 0..4000 {
        phase += 32;
        let fed = (previous + before) >> 3;
        let value = reference_value((phase >> 9) + fed, 0, false);
        before = previous;
        previous = if value >= 0 { value } else { value + 1 }; // sign and size
        out.push(reference_value((phase >> 9) + 2 * before, 0, false) >> 3);
    }
    assert_ne!(got, out);
}
