# ym2413-rust

The Yamaha YM2413 (OPLL) FM sound chip, and the VRC7's DS1001 variant, transcribed from
[ym2413-python](../ym2413-python) at `f88dccf` and proven equivalent to it: the same slot words,
the same output pairs and the same state, for both variants. It is the Rust counterpart of the
family's restarted, document-built OPLL core.

`src/opll.rs`, `src/tables.rs` and `src/variants.rs` are ym2413-python's `opll.py`, `tables.py`
and `variants.py` function for function, in the same order, with the same names and the same
spec citations and CHOICE / PROVISIONAL labels. Nothing is redesigned; where Rust forced a
difference the module header says what it is. The package is `ym2413-rust`, the crate `ym2413`;
it has no dependencies and is not published (use it by path).

## Using the core

```rust
use ym2413::{OPLL, YM2413, DS1001};

let mut chip = OPLL::new(YM2413);          // or OPLL::new(DS1001) for the VRC7
for (register, value) in [(0x30, 0x30), (0x10, 0x20), (0x20, 0x17)] {
    chip.write(0, register);                // latch an address
    chip.write(1, value);                   // piano, full volume; F-number $120, block 3, key on
    chip.run(84);
}
chip.run(3_579_545);                        // one second of the chip's clock
let pairs = chip.take_outputs();            // one (melody, rhythm) pair per 72 clocks
```

ym2413-python's `docs/api.md` is the contract; [docs/status.md](docs/status.md) lists the Rust
API a board uses. `chip.trace = Some(Vec::new())` records every slot's (melody word, rhythm
word) in the chip's one's-complement form; `None` (the default) costs one branch a slot.

**Save states: the `serde` feature.** With `features = ["serde"]`, `OPLL` and `OPLLVariant` are
`Serialize`/`Deserialize`, every field included, so a host's save state restores the chip
exactly; `cargo test --features serde` checks that a chip saved half way through a stream
carries on identically. Without the feature the crate has no dependencies.

## What is known, and how

Exactly what ym2413-python claims, no more: the core does what Yamaha's application manual says,
reproduces what the openMSX project reports measuring on one real YM2413, and plays the
DS1001's own instrument ROM as the NESdev wiki gives it. **No recording of a console is in
hand, so nothing is claimed exact to a console.** The YM2413's fifteen built-in melodic voices
are an emulator's hand-tuned table, not a dump. ym2413-python's README has the table of each
behaviour's standing and its `docs/claims.md` the evidence. This crate adds only the proof that
it computes what ym2413-python computes.

ym2413-python was written from documents under its `docs/restart.md` rules; this crate was
transcribed from ym2413-python alone, under the same rules. No GPL or LGPL source, no Nuked
model and none of the withdrawn earlier cores were read; no closed-box model was run.

## Status

All four rungs pass. Results and exact commands: [docs/status.md](docs/status.md).

| Rung | What | Result |
| --- | --- | --- |
| 1 | ym2413-python's tests (contract, manual, measured, community, provisional), transcribed | 130 of 130, the strict expected failure kept as one |
| 2 | Register-stream lockstep: 96 seeded scripts over both variants; every slot word, output pair, state and table compared byte for byte | 96 of 96 identical |
| 3 | Every stream of ym2413-python's `validation/streams.py`, both variants | 40 of 40 identical |
| 4 | ym2413-python's 30 seeded faults, broken the same way here | 30 of 30 caught |

```text
cargo test --release             # rung 1; needs nothing else (CI runs it with fmt and clippy)
scripts/fetch_ym2413python.sh    # the reference at the pin into external/ (for rungs 2 to 4)
scripts/rung1.sh
scripts/rung2.sh [count] [seed] [steps]
scripts/rung3.sh
scripts/rung4.sh
cargo run --release --example speed
```

[docs/handoff-brief.md](docs/handoff-brief.md) is the plan the crate was built from.

## Licence

MIT (`LICENSE`), like the reference. `src/variants.rs` also carries emu2413's MIT notice, for its
instrument table used as data (ym2413-python's T1, T3 and U22), reproduced here:

```text
The MIT License (MIT)

Copyright (c) 2001-2019 Mitsutaka Okazaki

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
