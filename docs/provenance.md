# Provenance

Where this crate's code came from, and the licence of each source. Written 2026-11-25 by the
family's provenance audit, wave 2, from this repository's own history, documents and comments.
No third-party source was opened to write this file.

## Verdict: clean

| Part | Source | Licence | Verdict |
| --- | --- | --- | --- |
| `src/` (`opll.rs`, `tables.rs`, `variants.rs`, `lib.rs`) | transcribed from ym2413-python alone, at `f88dccf` (now `67fdb1e` after the core's 2026-11-22 history rewrite, the same code); docs/status.md "How it was built" | MIT (the family's own work) | **clean** |
| `src/variants.rs`: the VRC7 and YM2413 instrument tables, PROVISIONAL rows | emu2413 v1.6.4 (Mitsutaka Okazaki), carried over from the reference with its notice | MIT; the notice is in `src/variants.rs` and README.md | **clean** (permissive, notice kept) |
| `tests/` | ym2413-python's tests transcribed | MIT | **clean** |
| `scripts/` | the ladder's drivers; they run the reference, never a closed-box model | MIT | **clean** |

What the reference rests on (Yamaha's documents, the openMSX YM2413 capture write-ups read
under the middle rule, emu2413's table) is recorded with its chain in ym2413-python's
`docs/provenance.md`; this crate adds no source of its own.

## How it was checked

- Commit messages (7 commits, 2026-11-13 to 2026-11-22) and docs/status.md: the only source
  named is ym2413-python at the pin; "No GPL or LGPL source, no Nuked model and none of the
  withdrawn earlier cores were read, and no closed-box model was run."
- Every mention of an emulator in the tree (`git grep`): emu2413 (MIT, notice kept), the
  openMSX capture project (measurements, through the reference) and the withdrawn Nuked-derived
  predecessor (named in the README's history note only). No GPL or LGPL code, no quoted
  fragment of one, no function or variable name taken from one.
- History: every commit by the family's handle; no third-party file was ever committed.
