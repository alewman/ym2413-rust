//! Rung 3: community documents whose accounts rest on hardware (tier 3).
//!
//! S6 is the NESdev wiki's "VRC7 audio" page, pinned in famicom-python at
//! `reference/nesdev-wiki/VRC7_audio.wiki`; S9 is SMS Power's "YM2413" page, pinned
//! in sms-python at `reference/smspower-wiki/YM2413.pmwiki`. Tests that parse a
//! pinned file skip when the sibling repository is absent. Every test here is tier
//! 3: it proves agreement with the community's account, not that account. Where the
//! account is about the VRC7 and the core applies it to the YM2413 too, the YM2413
//! case is PROVISIONAL (T4) and says so.
//!
//! A transcription of `tests/test_community.py` (at f88dccf); parametrized cases are loops.
//! The crate has no dependencies, so Python's two regular expressions are written out as small
//! parsers (`row`, `sms_power_has`) that accept what the patterns accept. A skipped test prints
//! why and returns (Rust's harness has no skip). The strict expected failure is
//! `#[should_panic]`: it fails if the assertion ever holds.

mod common;

use std::path::PathBuf;

use common::*;
use ym2413::variants::Row;
use ym2413::{OPLLVariant, DS1001, OPLL, YM2413};

fn wiki() -> PathBuf {
    root().join("../famicom-python/reference/nesdev-wiki/VRC7_audio.wiki")
}

fn sms_power() -> PathBuf {
    root().join("../sms-python/reference/smspower-wiki/YM2413.pmwiki")
}

/// One row of the wiki's tables: "  1 | 03 21 05 06 E8 81 42 27 | Buzzy Bell" or,
/// for the drums, "    | 01 01 18 0F DF F8 6A 6D | Bass Drum". Python's pattern:
/// `^\s*([0-9A-F]?)\s*\|\s*((?:[0-9A-F]{2}\s+){7}[0-9A-F]{2})\s*\|\s*(.+?)\s*$`.
fn row(line: &str) -> Option<(Option<u8>, Row)> {
    let mut parts = line.splitn(3, '|');
    let (head, bytes, name) = (parts.next()?, parts.next()?, parts.next()?);
    let head = head.trim();
    let number = match head.len() {
        0 => None,
        1 if head
            .chars()
            .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)) =>
        {
            Some(u8::from_str_radix(head, 16).ok()?)
        }
        _ => return None,
    };
    let tokens: Vec<&str> = bytes.split_whitespace().collect();
    if tokens.len() != 8
        || !tokens.iter().all(|token| {
            token.len() == 2
                && token
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
        })
    {
        return None;
    }
    if name.is_empty() {
        return None; // `(.+?)` needs a character after the second bar
    }
    let mut out = [0u8; 8];
    for (byte, token) in out.iter_mut().zip(tokens) {
        *byte = u8::from_str_radix(token, 16).ok()?;
    }
    Some((number, out))
}

type WikiRows = (Vec<(u8, Row)>, Vec<Row>);

fn wiki_rows() -> Option<WikiRows> {
    let path = wiki();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "SKIPPED: {} is absent (famicom-python not beside this repository)",
            path.display()
        );
        return None;
    };
    let mut voices: Vec<(u8, Row)> = Vec::new();
    let mut drums: Vec<Row> = Vec::new();
    for line in text.lines() {
        let Some((number, row)) = row(line) else {
            continue;
        };
        if row == [0, 1, 2, 3, 4, 5, 6, 7] {
            continue; // the column heads "00 01 ... 07"
        }
        match number {
            Some(number) => {
                // a dict in Python: a later row with the same number replaces the earlier
                voices.retain(|&(n, _)| n != number);
                voices.push((number, row));
            }
            None => drums.push(row),
        }
    }
    Some((voices, drums))
}

// ------------------------------------------------------ the DS1001's ROM (S6)

/// Tier 3. S6, "Internal patch set": "Exact values for the fixed patch were
/// dumped using the VRC7 rewired into a debug mode". Spec 5.5.
#[test]
fn test_the_ds1001_voices_are_the_wikis_read_out() {
    let Some((voices, _)) = wiki_rows() else {
        return;
    };
    let mut numbers: Vec<u8> = voices.iter().map(|&(n, _)| n).collect();
    numbers.sort();
    assert_eq!(numbers, (1..16).collect::<Vec<u8>>());
    let by_number: Vec<Row> = (1..16)
        .map(|n| voices.iter().find(|&&(m, _)| m == n).unwrap().1)
        .collect();
    assert_eq!(DS1001.voices.to_vec(), by_number);
}

/// Tier 3. S6: "The VRC7 instrument ROM dump also shows 3 drum patches",
/// equal to S4's fits on a YM2413 (spec 5.6; snare byte $68 is C4, T2).
#[test]
fn test_the_percussion_rows_are_the_wikis_drum_rows() {
    let Some((_, drums)) = wiki_rows() else {
        return;
    };
    assert_eq!(drums.len(), 3);
    assert_eq!(DS1001.percussion().to_vec(), drums);
    assert_eq!(YM2413.percussion().to_vec(), drums); // T2: $68 on both
}

/// Tier 3 (S6: "these are hardwired ... instrument $0 ... can be controlled by
/// registers $00-$07", the same eight-byte layout); spec 5.1.
#[test]
fn test_a_rom_voice_plays_as_its_row_loaded_as_the_user_tone() {
    for variant in [YM2413, DS1001] {
        for voice in 1..16u8 {
            let row = variant.voices[voice as usize - 1];
            let mut from_rom = OPLL::new(variant.clone());
            user_tone(&mut from_rom, &row); // the same writes on both chips, so they keep in step
            write(&mut from_rom, 0x30, voice << 4);
            note(&mut from_rom, 0, 0x120, 4);
            let mut from_user = OPLL::new(variant.clone());
            user_tone(&mut from_user, &row);
            write(&mut from_user, 0x30, 0x00);
            note(&mut from_user, 0, 0x120, 4);
            assert_eq!(melody(&mut from_rom, 3000), melody(&mut from_user, 3000));
        }
    }
}

// ------------------------------------------- six channels, no rhythm (S6)

/// Tier 3. S6: "6 channels of 2-operator FM"; the rhythm bit "is treated as
/// though it were always enabled ... The VRC7 has no rhythm DAC, so the 5 rhythm
/// channels are always inaudible". T15 for what the registers of channels 6 to
/// 8 do (accepted, silent).
#[test]
fn test_the_ds1001_sounds_six_channels_and_never_the_rest() {
    let mut chip = OPLL::new(DS1001);
    user_tone(&mut chip, &PLAIN_SINE);
    for channel in 0..9 {
        note(&mut chip, channel, 0x100, 4);
    }
    write(&mut chip, 0x0E, 0x3F); // every rhythm key, and the mode bit
    let trace = words(&mut chip, 2000);
    for slot in 0..18 {
        let heard: Vec<(i32, i32)> = trace.iter().map(|sample| sample[slot]).collect();
        if C[..6].contains(&slot) {
            assert!(heard.iter().map(|&(melody, _)| melody).max().unwrap() > 200);
        } else {
            assert!(heard.iter().all(|&pair| pair == (0, 0)));
        }
    }
}

// ----------------------------------------------------- the reset bit (S6, S7)

/// Tier 3. S6, "Audio Reset ($E000)": the bit will "clear its registers
/// (including tremolo LFO state, but not including vibrato LFO state)". The
/// board calls `reset()` for it; spec 14.2.
#[test]
fn test_the_vrc7_reset_clears_registers_and_tremolo_not_vibrato() {
    let mut chip = tone_chip(
        &PLAIN_SINE,
        Tone {
            variant: DS1001,
            ..Tone::default()
        },
    );
    chip.run(SAMPLE * 5000);
    chip.take_outputs();
    let (vibrato, tremolo) = (chip.vibrato_time, chip.tremolo_time);
    assert!(vibrato != 0 && tremolo != 0);
    chip.reset();
    assert!(chip.registers.iter().all(|&b| b == 0));
    assert_eq!(chip.tremolo_time, 0);
    assert_eq!(chip.vibrato_time, vibrato);
}

// --------------------------------------------------- the test register (S6)

const BOTH: [OPLLVariant; 2] = [DS1001, YM2413];

fn on(variant: &OPLLVariant, key: i32) -> Tone {
    Tone {
        variant: variant.clone(),
        key,
        ..Tone::default()
    }
}

/// Tier 3 on the DS1001; PROVISIONAL on the YM2413 (T4). S6: "Bit 0: The
/// envelope generators are replaced with constant 0 output (full volume) ...
/// The envelopes are still running while their output is overridden".
#[test]
fn test_test_bit_0_forces_full_volume_while_the_envelopes_run() {
    for variant in BOTH {
        let mut chip = tone_chip(&PLAIN_SINE, on(&variant, 0)); // released: level 127
        assert_eq!(*melody(&mut chip, 600).iter().max().unwrap(), 0);
        write(&mut chip, 0x0F, 0x01);
        assert_eq!(*melody(&mut chip, 600).iter().max().unwrap(), 255);
        assert_eq!(chip.level[C[0]], 127); // the envelope itself is untouched
        write(&mut chip, 0x0F, 0x00);
        assert_eq!(*melody(&mut chip, 600).iter().max().unwrap(), 0);
    }
}

/// Tier 3 / T4. S6: "Bit 1: Hold LFO phase at zero. This halts, disables, and
/// resets both the tremolo and vibrato LFO".
#[test]
fn test_test_bit_1_holds_both_lfos_at_zero() {
    for variant in BOTH {
        let mut chip = tone_chip(&PLAIN_SINE, on(&variant, 1));
        chip.run(SAMPLE * 3000);
        write(&mut chip, 0x0F, 0x02);
        chip.run(SAMPLE * 3000);
        chip.take_outputs();
        assert!(chip.tremolo_time == 0 && chip.vibrato_time == 0 && chip.tremolo == 0);
    }
}

/// Tier 3 / T4. S6: "Bit 2: Holds and resets waveform phase to zero. The
/// envelopes are not halted, though the output will be silent".
#[test]
fn test_test_bit_2_holds_every_phase_at_zero_and_silences_a_carrier() {
    for variant in BOTH {
        let mut chip = tone_chip(&PLAIN_SINE, on(&variant, 1));
        write(&mut chip, 0x0F, 0x04);
        let trace = words(&mut chip, 600);
        assert!(trace.iter().all(|sample| sample[C[0]].0 == 0));
        assert!(chip.phase[C[0]] == 0 && chip.phase[M[0]] == 0);
    }
}

/// Tier 3. As above, with the modulator at full level.
///
/// Python marks it `xfail(strict=True)`: "OPEN, not fixed: S6 says the output is silent with
/// bit 2 set. With a sounding modulator the core's carrier is moved by the modulator's output
/// at phase 0, which is not zero (the sine table has no zero, spec 6.3), so a small word is
/// heard. No document says how the chip avoids it."
#[test]
#[should_panic(expected = "OPEN: a modulated carrier is heard with test bit 2")]
fn test_test_bit_2_silences_a_modulated_carrier_too() {
    let mut chip = tone_chip(
        &[0x21, 0x21, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00],
        on(&DS1001, 1),
    );
    write(&mut chip, 0x0F, 0x04);
    assert!(
        words(&mut chip, 600)
            .iter()
            .all(|sample| sample[C[0]].0 == 0),
        "OPEN: a modulated carrier is heard with test bit 2"
    );
}

/// Tier 3 / T4. S6: "Bit 3: Update tremolo and vibrato LFOs every sample ...
/// (Tremolo is 64x faster, and vibrato is 1024x faster.)" Spec 7.7, 10.2.
#[test]
fn test_test_bit_3_runs_the_lfos_every_sample() {
    for variant in BOTH {
        let mut chip = tone_chip(&PLAIN_SINE, on(&variant, 1));
        write(&mut chip, 0x0F, 0x08);
        let (tremolo, vibrato) = (chip.tremolo_time, chip.vibrato_time);
        chip.run(SAMPLE);
        chip.take_outputs();
        assert_eq!((chip.tremolo_time - tremolo).rem_euclid(13_440), 64);
        assert_eq!((chip.vibrato_time - vibrato).rem_euclid(8_192), 1024);
    }
}

// ------------------------------------------------ the frequency formula (S6)

/// Tier 3. S6: F = 49716 Hz × freq / 2^(19 - octave), for a multiplier of 1.
/// Counted as rising zero crossings of a plain sine over 20,000 samples.
#[test]
fn test_the_frequency_formula_as_the_wiki_prints_it() {
    for variant in BOTH {
        for (f_number, octave) in [(0x100, 4), (0x0AB, 6), (0x1FF, 2)] {
            let mut chip = tone_chip(
                &PLAIN_SINE,
                Tone {
                    f_number,
                    block: octave,
                    variant: variant.clone(),
                    ..Tone::default()
                },
            );
            let count = 20_000;
            let expected = count as f64 * f_number as f64 / 2f64.powi(19 - octave);
            assert!((rises(&melody(&mut chip, count)) as f64 - expected).abs() <= 1.0);
        }
    }
}

// ---------------------------------------------- SMS Power's rhythm set-up (S9)

/// Python's `re.search(rf"\|\|\${register}\s*\|\|\${value}\b", text)`.
fn sms_power_has(text: &str, register: &str, value: &str) -> bool {
    let first = format!("||${register}");
    let second = format!("||${value}");
    let mut from = 0;
    while let Some(found) = text[from..].find(&first) {
        let after = from + found + first.len();
        let rest = text[after..].trim_start();
        if let Some(tail) = rest.strip_prefix(&second) {
            let boundary = tail
                .chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if boundary {
                return true;
            }
        }
        from = from + found + 1;
    }
    false
}

/// Tier 3. S9, "Rhythm mode": "$16 $20, $17 $50, $18 $C0, $26 $05, $27 $05,
/// $28 $01", as Yamaha advises (S1), with the five keys in $0E. Each sound's
/// slot on the rhythm pin (spec 12.2) carries a word.
#[test]
fn test_sms_powers_rhythm_values_play_the_five_sounds() {
    if let Ok(bytes) = std::fs::read(sms_power()) {
        let text = String::from_utf8_lossy(&bytes);
        for (register, value) in [
            ("16", "20"),
            ("17", "50"),
            ("18", "C0"),
            ("26", "05"),
            ("27", "05"),
            ("28", "01"),
        ] {
            assert!(sms_power_has(&text, register, value), "{register}");
        }
    }
    let mut chip = OPLL::new(YM2413);
    for (register, value) in [
        (0x16, 0x20),
        (0x17, 0x50),
        (0x18, 0xC0),
        (0x26, 0x05),
        (0x27, 0x05),
        (0x28, 0x01),
    ] {
        write(&mut chip, register, value);
    }
    write(&mut chip, 0x0E, 0x20);
    write(&mut chip, 0x0E, 0x3F);
    let trace = words(&mut chip, 3000);
    for slot in [13, 14, 15, 16, 17] {
        // hi-hat, tom, bass drum, snare, top cymbal
        assert!(
            trace
                .iter()
                .map(|sample| sample[slot].1.abs())
                .max()
                .unwrap()
                > 50,
            "{slot}"
        );
    }
}
