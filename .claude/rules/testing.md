# Testing

The fixtures are the contract. Tests encode it; you make `src/` satisfy them.

## Ground rules (also in CLAUDE.md)
- **Never edit `tests/`, `fixtures/`, `oracle/`, or `SPEC.md`** to make a test
  pass. No `#[ignore]`, no weakened/commented-out assertions, no hard-coding a
  fixture's expected values. The protect-immutable-paths hook blocks these.
- Tolerances are defined by the harness, not by you.

## Layout
- **Unit tests:** in-module under `#[cfg(test)] mod tests { ... }`, next to the
  code they exercise. Fast, pure, no I/O.
- **Integration / golden tests:** in `crates/tte-expand/tests/`, comparing engine
  output against the Parquet fixtures in `fixtures/`.
- **Doctests:** runnable `///` examples on public items; they double as
  documentation and are run by `cargo test`.

## Practice
- Prefer `cargo nextest run` for the suite (faster, per-test process isolation);
  `cargo test` remains the source of truth for doctests.
- On failure, assert on the **row-level diff**, not just a boolean — the message
  must show the offending rows/columns so a human can adjudicate a possible
  fixture/spec conflict.
- **Property-based tests** (`proptest`) for invariants that must hold across
  inputs (e.g. row counts, monotonic trial indices, no duplicate keys). Shrinking
  must be deterministic — seed any randomness.
- Tests must be **deterministic and independent**: no shared mutable global
  state, no wall-clock, no order-dependence between tests (see [[determinism]]).
- `assert_eq!`/`assert!` with a context message over bare `assert!`. Use
  `#[should_panic(expected = "...")]` with the specific message, not a bare
  `#[should_panic]`.
- Coverage (optional): `cargo llvm-cov` — treat gaps in the transform path as
  TODO tests, never as a reason to relax an assertion.
