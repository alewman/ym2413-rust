//! The speed as a real-time factor: a YM2413 with nine channels sounding (ROM voices 1 to 9,
//! full volume, keyed on), run for some seconds of chip time; chip time over wall time.
//! `scripts/speed_ref.py` does the same on ym2413-python.
//!
//! ```text
//! cargo run --release --example speed [seconds, default 20]
//! ```

use std::time::Instant;

use ym2413::{OPLL, YM2413};

const CLOCK_HZ: u64 = 3_579_545;

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .map(|arg| arg.parse().expect("seconds"))
        .unwrap_or(20);
    let mut chip = OPLL::new(YM2413);
    for channel in 0..9u8 {
        let f_number: u16 = 0x120 + 23 * channel as u16;
        for (register, value) in [
            (0x30 + channel, (channel + 1) << 4),
            (0x10 + channel, (f_number & 0xFF) as u8),
            (0x20 + channel, 0x10 | 4 << 1 | (f_number >> 8) as u8),
        ] {
            chip.write(0, register);
            chip.write(1, value);
            chip.run(84);
        }
    }
    chip.take_outputs();
    let start = Instant::now();
    let mut heard = 0u64;
    for _ in 0..seconds {
        chip.run(CLOCK_HZ);
        heard += chip.take_samples().iter().filter(|&&v| v != 0).count() as u64;
    }
    let wall = start.elapsed().as_secs_f64();
    println!(
        "{seconds} s of chip time in {wall:.3} s: {:.1} x real time ({heard} non-zero samples)",
        seconds as f64 / wall
    );
}
