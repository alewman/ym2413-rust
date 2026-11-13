> Historical: the plan this crate was built from, 2026-11-13; current state in [status.md](status.md).

# Handoff brief: ym2413-rust

## The request

2026-11-13: a Rust transcription of the restarted pure-Python core ym2413-python (the Yamaha
YM2413/OPLL and the VRC7's DS1001), proven equivalent to it, in the shape of gb-apu-rust. Two
sibling agents port sn76489-python and ym2612-python at the same time in their own directories.

## Why, and the rules that bind

The family's earlier sound cores were translations of GPL/LGPL C models and were withdrawn. The
Python core was rebuilt from documents under `docs/restart.md` of ym2413-python (rules 1-11 and
the 2026-11-13 amendment). Those rules bind this crate:

- Nothing under `/data/backup/nuked-derived/` is read (that includes the withdrawn
  `zzz-ym2413-rust`), nor any Nuked `.c`/`.h` source anywhere, nor any GPL emulator's source.
- The **only source** is ym2413-python at `f88dccf174780d7c958a45129ce9e337450f387b`: its code,
  tests, scripts and docs, cloned into the git-ignored `external/ym2413-python`. Its `reference/`
  directory is not opened (and is not in the clone: it is git-ignored there).
- **Transcribe; don't redesign.** Function for function, same order, same names (Rust case
  conventions aside), same comments, spec citations and CHOICE/PROVISIONAL labels kept. Where
  Rust forces a difference the module header says so.
- The variants stay data. `variants.py` carries emu2413's MIT notice for its instrument table:
  it travels with that table into `src/variants.rs` and the README.
- If the Python looks wrong it is **not fixed here**: behaviour stays identical and the case is
  written in `docs/status.md` under "Suspected errors in the reference".
- No closed-box model is run (`validation/closed_box.py`, the Nuked harness: not run).

## Scope

Crate `ym2413`, package `ym2413-rust`, edition 2021, MIT, `publish = false`, no dependencies by
default; an optional `serde` feature makes `OPLL` (and `OPLLVariant`) serialisable for a host's
save states.

Modules mirror the Python package:

- `src/tables.rs` = `tables.py`: the ROMs computed from the same formulas (once, on first use),
  the fixed tables, the slot maps.
- `src/variants.rs` = `variants.py`: `OPLLVariant` with the same fields, `YM2413`, `DS1001`,
  the voice tables with emu2413's notice, `percussion_rows`, `VIBRATO_OFFSETS`.
- `src/opll.rs` = `opll.py`: `decay_step`, `attack_step`, `OPLL` with `new`, `reset`, `write`,
  `run`, `take_outputs`, `take_samples` and the private steps in the same order.
- `src/lib.rs` = `__init__.py`: the six public names.

The API keeps the reference's shape: `write(port, value)`, `run(clocks)`, `reset()`,
`take_outputs()` (one (melody, rhythm) pair per sample), `take_samples()`, and `trace`, an
`Option` that costs one branch per slot when `None`. The state is public fields, as the Python
attributes are public (the tests read `level`, `phase`, `counter` and the rest).

## The ladder

Each rung has a script in `scripts/` that says PASS or FAIL.

1. **Rung 1**: every test in the reference's `tests/` (contract, manual, measured, community,
   provisional) transcribed to `cargo test`, the strict expected failure kept as an expected
   failure (`#[should_panic]`). Tests that parse files of sibling repositories skip when the file
   is absent. Parametrized cases become loops. Python's `random.Random` is reproduced in the test
   helpers (MT19937 with CPython's seeding and `randrange`/`choice`) so the contract tests use the
   same streams.
2. **Rung 2**: register-stream lockstep. `scripts/lockstep_gen.py` writes seeded random and
   scripted streams (every register including rhythm mode, the test register, aliases, unmapped
   addresses; `run()` by odd amounts not aligned to slots; resets; both variants);
   `scripts/lockstep_ref.py` runs them on the reference under PyPy and
   `examples/ym2413-trace.rs` on this crate. Traces (every slot word, every output pair, the
   whole observable state at checkpoints, the computed tables) compared byte for byte.
3. **Rung 3**: every stream of the reference's `validation/streams.py`, exported by
   `scripts/export_streams.py`, through both cores and both variants: slot words and outputs
   identical.
4. **Rung 4**: the 30 faults of the reference's `validation/faults.py`, the same rules broken in
   the Rust source by `scripts/seeded_faults.py`; each must be caught by some rung; report which
   rungs catch how many.

Also: the speed as a real-time factor, release build, nine channels sounding (the reference
gives about 11x on PyPy).

## Working rules

Local only: `git init`, commits at good points with the trailer
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; no remote, nothing pushed. No
`rm -rf`, no `sed -i`, no forced git commands; set things aside in the git-ignored `trash/`.
`target/` stays in the repository (on /data). At most 8 parallel jobs. `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` clean; a minimal CI workflow like gb-apu-rust's.
Chip-level judgement calls are taken and written down; policy questions are reported, not
decided.
