# Phase 1 — ITT Expansion: Completion Summary

**Status: ✅ ITT engine implemented and verified bit-exact against the Oracle.**
Date: 2026-06-29.

Phase 1's Definition of Done (from
[`../../ROADMAP.md`](../../ROADMAP.md)) is *"bit-exact match on the ITT fixtures
for the structural columns; property tests pass; `forbid(unsafe_code)` holds."*
The engine (`tte_expand::expand` / `expand_parquet`) now reproduces
`TrialEmulation 0.0.4.11`'s ITT expansion **bit-for-bit** on all generated ITT
fixtures, with `cargo` fmt/clippy/test/check green.

## What was implemented

### A. The ITT expansion engine (`crates/tte-expand/src/lib.rs`)

The expansion is a deterministic **self-join** on the lazy Polars API:

1. **Seeds** — rows with `eligible == 1` and `first_period ≤ period ≤
   last_period`. Each contributes `trial_period := period` (cast `Int32`) and
   `assigned_treatment := treatment` (the baseline treatment).
2. **Follow-up** — inner-join seeds to the full person-time on `id`, keeping
   `period ≥ trial_period`. Each kept row becomes one output row with
   `followup_time := period − trial_period`, the *actual* `treatment`/`outcome`
   at that period, and the carried-forward `assigned_treatment`.
3. **Order** — sort by `(id, trial_period, followup_time)` ascending, a unique
   key (so the sort is total and deterministic).

ITT performs **no** artificial censoring on treatment switching; follow-up runs
to the patient's last observed period (the input already truncates a patient at
their first event/censoring, so this matches "until event/censor/end of data").

**Key Polars / dtype design choices (this is where bit-exactness lives).** The
Oracle's output dtypes are *not* uniform — they track the input dtypes plus two
fixed coercions. The engine reproduces them exactly by reading the input parquet
as-is and:

| Output column | Rule |
|---|---|
| `id` | passthrough of input `id` dtype (float64 in edge, int32 in scenarios) |
| `trial_period` | **Int32** always |
| `followup_time` | inherits the input `period` dtype (`period − trial_period`) |
| `assigned_treatment`, `treatment` | passthrough of input `treatment` dtype |
| `outcome` | **Float64** always |

The input `period` dtype is recovered once via `collect_schema()` so
`followup_time` can be cast back to it. No `unsafe`, no Rust `as` casts (only
Polars runtime `.cast`), so the `cast_possible_truncation` / `forbid(unsafe_code)`
gates hold. `ExpandOptions` gained defaulted `eligible_col` / `outcome_col`
fields (defaults `"eligible"` / `"outcome"`); `ExpandOptions::new`'s signature is
unchanged.

### B. Fixtures generated from the Oracle

Running the Oracle's own functions produced the Phase-1 **ITT** fixtures under
`fixtures/edge/` and `fixtures/scenarios/` (+ `MANIFEST.json`):

- **Edge battery (all 9 graded cases):** `E01_single`, `E02_id4_canonical`,
  `E03_event_at_baseline`, `E04_reentry`, `E05_never_treats`,
  `E06_switch_then_back`, `E07_last_period_only`, `E08_ties`, `E09_max_fanout`.
  E04/E06/E08/E09 were signed off (epi review + Oracle confirmation) after the
  initial five; E04 (re-entry), E08 (ties), E09 (496-row fan-out) and E06's ITT
  view all pass bit-exact. (E06's *PP* view is Phase 2.)
- **Simulated scenarios (8):** `common`, `rare_event`, `ultra_rare_event`,
  `rare_initiation`, `high_switching`, `heavy_censoring`, `short_followup`,
  `strong_confounding` (the `large_scale` benchmark cohort is excluded, per
  `run_all.R`). These exercise events, censoring, and switching that the edge
  cases do not.

R environment: `TrialEmulation 0.0.4.11`, `arrow 24.0.0`, `data.table 1.18.4`,
`digest 0.6.39`, `jsonlite 2.0.0` (R 4.3.3), installed as Linux binaries from the
Posit Public Package Manager. **`STRUCTURAL_COLS` was verified correct as-is** via
the *VERIFY FIRST* snippet — `names(prep$data)` is exactly the six structural
columns (+ a dropped `weight`); this version emits **no** PP censor/expand-flag
column, so `oracle/00_setup.R` needed no change.

## Verification performed (2026-06-29, Rust 1.95.0)

| Check | Result |
|---|---|
| ITT edge fixtures E01–E09 (all 9) — bit-exact (schema + values + order) | ✅ |
| ITT scenario fixtures (8 cohorts, up to 114 475 rows) — bit-exact | ✅ |
| `expand_parquet` write→reread round-trip preserves dtypes | ✅ |
| Invariants (followup_time ≥ 0; one baseline per trial; assignment sourced from `trial_period`) | ✅ |
| `cargo test --workspace --all-features --all-targets --locked` | ✅ 37 passed (20 lib + 17 integration), 0 ignored |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean |
| `cargo fmt --all --check` | ✅ clean |
| `cargo check --workspace --all-features --all-targets --locked` | ✅ |
| `pre-commit run --all-files` | ✅ all hooks pass |

The 17-case canonical contract test (`crates/tte-expand/tests/itt.rs`, ITT edge +
scenarios) is mirrored by 20 in-crate tests (`src/lib.rs` `#[cfg(test)]`) — fixture
matches plus `expand_parquet` round-trip and the property invariants, including
`invariant_assigned_treatment_sourced_from_trial_period` (the re-entry-critical
property: `assigned_treatment` is the input `treatment` at each trial's own
`trial_period`, never frozen from first eligibility).

## Decisions / deviations recorded

- **Output dtypes are input-derived, not uniform.** Reproduced exactly (table
  above); a naive uniform-dtype output would fail bit-exactness — e.g.
  `followup_time` is `float64` in `E01`/`E03` (because their input `period` is an
  R double literal `0`) but `int32` elsewhere (`period = 0:9` via `:`).
- **Row order** matches the Oracle's `order(id, trial_period, followup_time)`
  via an explicit, total, ascending sort.
- **Two Oracle-side issues found while generating fixtures (both
  in protected files, neither affects ITT output):**
  1. `validate_input()` in `oracle/10_simulate.R` calls `data.table` `by=` syntax
     on a plain `data.frame` (`unused argument (by = id)`), so the scenario build
     path — and thus `run_all.R` — aborts. **Workaround:** the scenario fixtures
     were generated by calling `simulate_cohort()` directly (validate_input only
     *validates*; it returns its input unchanged), so the fixtures are identical
     to what a fixed `run_all.R` would emit.
  2. PP `data_preparation` errors inside `glm` (`mu must be a nonempty numeric
     vector`) on degenerate single-row inputs (e.g. `E01`). Because
     `dump_fixture` generates ITT **and** PP by default, `run_all.R` would also
     abort here. **Workaround:** Phase-1 generation is **ITT-only**.
- **Provenance deviation:** no `renv.lock` existed, so packages are PPM "latest"
  as of 2026-06-29, not a pinned snapshot. The fixtures themselves are
  deterministic; pinning (`renv::snapshot()`) is a follow-up.
- **`eligible`/`outcome` columns** are not in `ExpandOptions::new`'s 5-arg
  signature (fixed by the frozen `tests/itt.rs` call), so they are stored as
  defaulted fields (`"eligible"`/`"outcome"`) with builder overrides.

## Deferred to Phase 2+ / open questions

1. **Edge cases `E04`/`E06`/`E08`/`E09` — signed off and landed (ITT).** Epi
   review + Oracle confirmation fixed the conventions in `oracle/30_edge_cases.R`
   (a header documents the two eligibility conventions: monotone "never-yet-treated"
   for E01–E03/E05–E09 vs. deliberately non-monotone re-entry for E04). All four ITT
   fixtures pass bit-exact; E04's re-entry assignment rule is now a SPEC §2/§5
   invariant + property test. **Only `E06`'s PP view remains** (Phase 2): the
   1→0→1 trajectory must censor at the first deviation and *not* resume at the
   switch-back.
2. **Per-protocol (Phase 2):** `expand_until_switch` first-deviation censoring.
   The package's rule is "discard data after the first switch" (arXiv:2402.12083);
   generate PP fixtures via the S4 `trial_sequence("PP") |> … |> expand_trials()`
   path, which expands **without** fitting the switch-weight `glm` and so sidesteps
   the degenerate-input crash. Confirm whether that path emits a censor/expand-flag
   column to freeze into `STRUCTURAL_COLS`.
3. **Tier-2 golden pipeline & weights (Phase 3):** unchanged; needs the R solver.
   A future explicit censoring (`C`) input column would let `E08` carry a real
   competing-risk tie (same event-before-censor precedence).
4. **Oracle hygiene:** fix the two `run_all.R` blockers (the `validate_input`
   `data.table` bug and the PP `glm` crash) so CI can regenerate the full fixture
   set (ITT + PP + golden) and diff sha256.
