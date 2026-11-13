//! The Yamaha YM2413 (OPLL) and the Konami VRC7's DS1001.
//!
//! Written from documents and measurements (ym2413-python's `docs/spec.md`), not from another
//! program's source: its `docs/restart.md` has the rules. The names below are the
//! whole public surface (its `docs/api.md`).
//!
//! A transcription of ym2413-python (`src/ym2413_python/` at f88dccf), module for module:
//! `opll.rs` is `opll.py`, `tables.rs` is `tables.py`, `variants.rs` is `variants.py`. The six
//! names of Python's `__all__` are re-exported here; the modules are public too, because the
//! reference's tests (and so this crate's) reach into `tables` and `variants`, and a board's
//! save-state walker reads the chip's fields.
//!
//! Save states (not in ym2413-python, which relies on plain attributes): with the `serde`
//! feature `OPLL` and `OPLLVariant` are `Serialize`/`Deserialize`, every field included. Without
//! the feature the crate imports nothing.

pub mod opll;
pub mod tables;
pub mod variants;

pub use opll::{CLOCKS_PER_CYCLE, CYCLES_PER_SAMPLE, OPLL};
pub use variants::{OPLLVariant, DS1001, YM2413};
