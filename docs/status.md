# Status

2026-11-13. **The ladder passes: all four rungs.** Local only: no remote, nothing pushed, not yet
adopted by a board.

Reference: ym2413-python `f88dccf174780d7c958a45129ce9e337450f387b`, cloned into the git-ignored
`external/ym2413-python` (`scripts/fetch_ym2413python.sh`); its `reference/` directory was not
opened (and is not in the clone). Run on Linux x86_64, rustc 1.93.1 (clippy 0.1.99 from the
rustup stable toolchain), the reference on PyPy 7.3.23 (Python 3.11.15,
`/data/emu/ym2413-python/.venv-pypy/bin/python`).

| Rung | What | Result | Command |
| --- | --- | --- | --- |
| 1 | Every test in ym2413-python's `tests/` transcribed (`tests/contract.rs`, `manual.rs`, `measured.rs`, `community.rs`, `provisional.rs`; parametrized cases folded into loops), the strict `xfail` kept as `#[should_panic]` | 130 of 130 test functions pass (Python's 515 cases), plus one check that the test helper `PyRandom` draws what Python's `random.Random` draws. The two wiki-parsing tests ran (famicom-python and sms-python are beside this repository), none skipped | `scripts/rung1.sh` |
| 2 | Register-stream lockstep: 96 seeded scripts (48 per variant) of random and scripted writes to every address $00-$FF (registers, aliases, unmapped, the test register, rhythm mode and keys), data without an address, ports 0-3, `run()` by odd amounts inside slots, long runs, resets, `trace` switched on and off mid-sample, `take_samples()`; both cores' traces (every slot word, every output pair, the whole state at checkpoints, the computed ROM tables) compared byte for byte | 96 of 96 identical: 158,528 writes, 8,349,361 output pairs, 6,632,402 traced samples of 18 slot words, 1,257 state checkpoints | `scripts/rung2.sh [count] [seed] [steps]` |
| 3 | Every stream of ym2413-python's `validation/streams.py` (20), on both variants, replayed as its harness replays them; per-take FNV-1a hashes of the output pairs and of every slot word, and the final state | 40 of 40 identical: 2,344,658 output pairs, 42,203,844 slot words | `scripts/rung3.sh` |
| 4 | The 30 seeded faults of ym2413-python's `validation/faults.py`, the same rules broken in `src/` (`scripts/seeded_faults.py`); the unbroken copy is judged first | 30 of 30 caught (below) | `scripts/rung4.sh` |

`cargo test --release` (rung 1) needs nothing else; CI runs it with `cargo fmt --check` and
`cargo clippy --all-targets -D warnings`, with the `serde` feature. Rungs 2 to 4 need the
reference (PyPy is found at the path above or as `pypy3`; `PYTHON=` overrides, `JOBS=` sets the
parallelism, default 8). Traces go to the git-ignored `external/lockstep/` and
`external/streams/`.

### Rung 4: which rungs catch the faults

| Rung | Faults caught |
| --- | --- |
| rung 1, any test file | 30 of 30 |
| rung 1 contract | 4 |
| rung 1 manual | 17 |
| rung 1 measured | 24 |
| rung 1 community | 9 |
| rung 1 provisional | 12 |
| rung 2 (lockstep) | 30 of 30 |
| rung 3 (streams) | 28 of 30: not "no register aliases" (no stream writes an alias) nor "VRC7 reset clears the vibrato" (no stream resets) |

Every fault is caught by rung 1 and by rung 2 independently. The Rust text of each fault is the
Python fault's, translated; two differ beyond syntax and say so in the script: "test bit 0
ignored" tests `& 0x00` (Python's `& 0x100` cannot be written on a `u8`; both are never true) and
"total attenuation not clipped" deletes the line (Python replaces it with `pass`).

The first run of the fault script attributed rung 1 wrongly (it joined cargo's stdout and stderr,
so result lines lost their file headers and a fallback marked every file); its rung 2, rung 3 and
"caught at all" results were right. The script was fixed to merge the streams in order and to
stamp every copied file's modification time before building (copies keep the sources' old times,
and cargo judges freshness by them); the counts above are from that fixed run. Each fault copy
has its own build directory.

## Speed

`cargo run --release --example speed` (a YM2413, nine channels on ROM voices 1-9 at full volume,
20 s of chip time): **about 80 to 120 times real time** on this machine, loaded (load average
15-27 while measured; three runs gave 77x, 81x, 107x; an earlier run at lower load 122x).
`scripts/speed_ref.py` runs the same set-up on ym2413-python under PyPy: 8.4x to 9.0x at the same
time (its README says about 11x). Correct, not fast: the core computes slot by slot (T16), as the
reference does.

## Suspected errors in the reference

None in the core: no case where `src/ym2413_python/` looked wrong, and nothing was changed.
In its tests:

- **`test_manual.py::test_table_iii_6_modulation_index`, second half.** `first`, the first sample
  where the fed-back modulator differs from the one without feedback, is 0 for every FB (the
  notes are keyed by `note()`, whose writes run four samples before `attribute()` starts
  reading). So `b[first - 1] + b[first - 2]` reads Python's `b[-1] + b[-2]`, the last two samples
  of the 400, and the following `all(... for i in range(2, first))` checks nothing. The test
  passes, but its second half does not test what its comment says. Transcribed as Python reads
  it (negative indices from the end), with a note.
- Dead or idle code, harmless: `test_tremolo_is_3_7_hz` builds a `trace` it only checks for
  `not None`; `test_the_snare_at_f_number_0_shows_the_noise_alone` collects `heard` and checks
  only its length; `test_the_top_cymbal_is_two_peaks_and_reaches_full_scale` builds a first chip
  and writes $38 to it, then never uses it. Kept (the first as a comment).

## The API a board uses

Crate `ym2413` (package `ym2413-rust`; `ym2413-rust = { path = "../ym2413-rust" }`, then
`use ym2413::...`). No dependencies. The names are ym2413-python's (its `docs/api.md`):

```rust
use ym2413::{OPLL, OPLLVariant, YM2413, DS1001, CLOCKS_PER_CYCLE, CYCLES_PER_SAMPLE};

let mut chip = OPLL::new(YM2413);          // OPLL::new(variant: OPLLVariant); DS1001 for the VRC7
chip.write(0, 0x30);                        // write(port: u8, value: u8): bit 0 of port is A0
chip.write(1, 0x10);                        // data to the latched address (it stays latched)
chip.run(clocks);                           // run(clocks: u64): input clocks, 3.579545 MHz
let pairs: Vec<(i32, i32)> = chip.take_outputs(); // (melody, rhythm) per finished sample
let values: Vec<i32> = chip.take_samples(); // melody + rhythm, the same samples
chip.reset();                               // /IC, or the VRC7's $E000 bit 6

chip.trace = Some(Vec::new());              // validation: each sample's 18 (melody, rhythm) words
let words = chip.trace.take();              // one's complement (-1 is minus zero); None = off

// CLOCKS_PER_CYCLE: u64 = 4, CYCLES_PER_SAMPLE: usize = 18 (72 clocks a sample)
// A new variant: OPLLVariant { snare_last_byte: 0x48, ..YM2413 } (Python's dataclasses.replace)
```

Every piece of state is a public field, as every Python attribute is public (`registers`,
`level`, `phase`, `counter`, ...); `variant` is configuration. `OPLL` is `Clone + PartialEq`, so
a save-state walker can copy it whole. The modules `opll`, `tables` and `variants` are public
because the tests reach into them (`tables::tables()`, `variants::VIBRATO_OFFSETS`, ...).

**Save states** (the `serde` feature; ym2413-python relies on plain attributes): `OPLL` and
`OPLLVariant` derive `Serialize`/`Deserialize` over every field, `trace` included, the variant in
full (its name as a `Cow<'static, str>`, so a restored variant owns it), the 64 register bytes as
bytes. `tests/save_state.rs` saves a chip half way through four streams on YM2413, DS1001 and a
third variant, inside a slot, restores it with bincode and checks it carries on identically.
Provisional: the format is serde's, so a field change changes it; a host should version its own
states.

## Where Rust differs from the Python (the module headers say the same)

- Integers are `i32` (registers `u8`, the clock `u64`); every value stays far inside them.
  Negative `>>` is arithmetic and `!` is Python's `~`, as the operator arithmetic needs.
- `write` takes `u8`s (Python masks the value and reads bit 0 of the port).
- `LOG_SIN`, `EXP` and `KEY_SCALE_STEPS` are computed once on first use (`tables::tables()`,
  a `OnceLock`) from the same formulas, rounding halves to even as Python's `round()`; rung 2
  compares them with Python's value for value. `TREMOLO` and the slot maps are `const`.
- `COPY_SLOT` is an array of `Option`s; `percussion` is a method; Python's `hasattr` in
  `reset()` is `new()` setting `vibrato_time` first; `trace` is an `Option<Vec<Vec<_>>>` that
  fills exactly as Python's list of lists.
- The tests: Python's `random.Random` is `common::PyRandom`; keyword arguments are a `Tone`
  record and a few named helpers; the two regular expressions of `test_community.py` are small
  parsers; a skip prints `SKIPPED` and returns; `tests/common/mod.rs` carries
  `rhythm_chip` and `two_operator_reference`, which `test_provisional.py` imports from
  `test_measured.py`. The three contract tests about Python objects (frozen dataclass, plain
  attributes, PyPy's 80-attribute limit) check the Rust counterpart of what they guard.

## Open

- No board uses the crate yet (sms-python's and famicom-python's Rust boards would).
- No remote: creating one, and making it public, is the owner's call. Before that, the
  emu2413-derived instrument table's notice travels in `src/variants.rs` and the README, as in
  the reference.
- The wasm32 build is untried; the library is plain `std` with no dependencies.
- Speed: correct, not fast; a `-fast` fork is the place for a whole-sample path.
