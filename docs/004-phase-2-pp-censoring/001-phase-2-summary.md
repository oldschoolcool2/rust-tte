# Phase 2 — Per-Protocol Artificial Censoring: Completion Summary & Sign-off

**Status: ✅ PP engine implemented and verified bit-exact against the Oracle;
ITT path unchanged.**
Date: 2026-06-29.

Phase 2's Definition of Done (from [`../../ROADMAP.md`](../../ROADMAP.md)) is
*"exact match on PP fixtures incl. the ITT-vs-PP divergence case; ITT path
unchanged."* The engine (`tte_expand::expand` with `Estimand::PerProtocol`) now
reproduces `TrialEmulation 0.0.4.11`'s per-protocol expansion **bit-for-bit** on
all generated PP fixtures (9 edge + 8 scenarios), the ITT contract is byte-for-byte
untouched, and `cargo` fmt/clippy/test/check + `pre-commit` are green.

## What was implemented

### A. Per-protocol censoring in the engine (`crates/tte-expand/src/lib.rs`)

PP is exactly the ITT expansion (§2) with each emulated trial's follow-up
**censored at the first deviation** from its `assigned_treatment`:

1. A new `Estimand { Itt, PerProtocol }` enum (default `Itt`, `#[non_exhaustive]`)
   and an `ExpandOptions.estimand` field with an `ExpandOptions::with_estimand`
   builder. `ExpandOptions::new`'s signature is unchanged, so every existing
   caller and the whole ITT suite are unaffected — the ITT branch returns the
   Phase-1 frame verbatim.
2. For `PerProtocol`, the ITT frame (already sorted by
   `(id, trial_period, followup_time)`) gains a per-trial cumulative deviation
   flag and is filtered to the **adherent prefix**:

   ```text
   deviated      = (treatment != assigned_treatment) as Int32          # {0,1}
   cumulative    = deviated.cum_max()  OVER (id, trial_period)
                                       ORDER BY followup_time           # cum_agg
   keep row iff  cumulative == 0
   ```

   The cumulative max is `0` exactly on the rows strictly before the first
   deviation; it flips to `1` at the deviation and stays `1`, which **drops the
   deviating row and everything after it in one step** — so a later switch-back can
   never resume follow-up. The baseline row (`followup_time == 0`) never deviates
   by construction, so every trial keeps at least its baseline.

**Polars design.** The flag uses the `cum_agg` feature (added to `Cargo.toml` at
the Phase-0 scaffold in anticipation of this phase) via `cum_max`, wrapped in
`over_with_options(...)` with an
**explicit `order_by = followup_time`** so the cumulative is independent of
physical row order — determinism does not rely on the upstream sort. The censoring
re-`select`s the six structural columns, so PP output has the **same columns,
dtypes and order as ITT**; only the row count changes. No `unsafe`, no Rust `as`
casts (Polars `.cast` only), no `unwrap`/`expect`/indexing in the library path.

### B. The contract: fixtures, SPEC §3, and the PP test

- **`oracle/40_dump_fixtures.R`** gained an `oracle_expand_pp()` branch. ITT is
  untouched. PP is generated as **`PP = ITT ∩ S4-survivors`** (see *Decisions*).
- **`SPEC.md` §3** was finalised: the first-deviation/deviating-row-excluded rule,
  the switch-back trap, the `cum_max` operational rule, the schema decision, the
  generation provenance, the divergence map, and worked examples for `E06`/`E02`/
  `E04`. §5 gained the **PP monotone-censoring & adherence** invariant.
- **`crates/tte-expand/tests/pp.rs`** mirrors `tests/itt.rs`: all 17 PP fixtures,
  bit-exact frame equality on the structural columns with a readable row-level
  diff. The in-crate `#[cfg(test)]` module gained the 17 PP fixture matches plus a
  self-contained `invariant_pp_monotone_censoring` property test (derived from the
  input fixtures alone, so it is a true invariant, not a tautology).

## Fixtures generated from the Oracle

`expected_<case>_pp.parquet` for the 9 edge cases (`E01`–`E09`) and 8 simulated
scenarios (`common`, `rare_event`, `ultra_rare_event`, `rare_initiation`,
`high_switching`, `heavy_censoring`, `short_followup`, `strong_confounding`),
derived from the **committed `input_<case>.parquet`** so each PP fixture follows
the exact bytes the engine reads.

**The S4 path.** The legacy `data_preparation(estimand_type = "PP")` path fits a
switch-weight `glm` that errors (`Argument mu must be a nonempty numeric vector`)
on degenerate single-patient / control-only cohorts — and it crashes **exactly**
on the cases where PP == ITT (`E01`, `E03`, `E05`, `E07`, `E08`, `E09`). The PP
row-set is therefore taken from the modern S4 path
`trial_sequence("PP") |> set_data(…) |> set_expansion_options(output =
save_to_datatable(), chunk_size = 0) |> expand_trials()`, which does the
structural expansion **without** fitting any `glm` and never crashes. Where the
legacy PP path *does* run (`E02`, `E04`, `E06` + all 8 scenarios) the two agree on
the retained rows bit-for-bit.

**No flag column (the schema decision).** The S4 datastore's native schema is
`id, trial_period, followup_time, outcome, weight, treatment` — no
`assigned_treatment`, a placeholder `weight` (all `1.0`; no weight model is
fitted), and a different order. We keep PP in the **same six `STRUCTURAL_COLS` as
ITT**: drop the placeholder `weight` (exactly as the ITT path does) and reattach
`assigned_treatment` from the ITT frame. This is exact, not invented — on every
retained PP row `treatment == assigned_treatment` (the kept rows are the adherent
prefix). `STRUCTURAL_COLS` is therefore **unchanged**; censoring manifests purely
as missing rows.

R environment: `TrialEmulation 0.0.4.11`, `arrow 24.0.0`, `data.table 1.18.4`,
`digest 0.6.39`, `jsonlite 2.0.0` (R 4.3.3), Posit PPM binaries — same as Phase 1.

## Verification performed (2026-06-29, Rust 1.95.0)

| Check | Result |
|---|---|
| PP edge fixtures `E01`–`E09` (all 9) — bit-exact (schema + values + order) | ✅ |
| PP scenario fixtures (8 cohorts, up to 59 559 rows) — bit-exact | ✅ |
| ITT-vs-PP divergence reproduced (`E02` 27→11, `E04` 11→7, `E06` 4→2; scenarios) | ✅ |
| `E06` switch-back trap: PP keeps only `followup_time` 0,1 (no resume at 3) | ✅ |
| PP == ITT cases (`E01`/`E03`/`E05`/`E07`/`E08`/`E09`) produced and matched | ✅ |
| **ITT unchanged** — `tests/itt.rs` (17) + in-crate ITT all bit-exact | ✅ |
| `invariant_pp_monotone_censoring` (contiguous adherent prefix; subset of ITT) | ✅ |
| Two PP generators (re-simulated cohort vs committed-input read) — sha256 match | ✅ |
| `cargo test --workspace --all-features --all-targets --locked` | ✅ |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean |
| `cargo fmt --all --check` | ✅ clean |
| `cargo check --workspace --all-features --all-targets --locked` | ✅ |
| `pre-commit run --all-files` | ✅ all hooks pass |

## Decisions / deviations recorded

- **Deviation-row convention = EXCLUDED.** Read off the Oracle, not guessed: PP
  keeps rows **strictly before** the first deviation. The deviating row itself is
  dropped (`E06`: `followup_time` 2 and 3 both absent). The fixture is authority.
- **No 7th column / no flag column.** The S4 path carries `weight` (placeholder)
  but not `assigned_treatment`; we keep the ITT-consistent six `STRUCTURAL_COLS`
  (drop `weight`, reattach `assigned_treatment`). `assigned_treatment == treatment`
  on every PP row, so this is exact. `STRUCTURAL_COLS` and the ITT fixtures are
  untouched.
- **`PP = ITT ∩ S4-survivors`.** Sidesteps the legacy-PP `glm` crash on degenerate
  cohorts and inherits the ITT frame's input-derived dtypes verbatim (so PP and
  ITT fixtures share schema bit-for-bit). Verified equal to the legacy PP path
  wherever it runs.
- **Determinism.** The window cumulative pins `order_by = followup_time`
  explicitly, so PP does not depend on physical row order; the final frame is the
  same total, deterministic `(id, trial_period, followup_time)` sort as ITT.
- **SPEC clarification flagged.** The Phase-1 §3 draft said "same six structural
  columns" — empirically true under this schema decision, but the *generation*
  uses the S4 path + `assigned_treatment` reattach (not `data_preparation` PP).
  §3 now documents this precisely.

## Deferred to Phase 3+

- **Weight application (Phase 3):** join the pre-computed IPCW on
  `(id, trial_period, followup_time)` and multiply (exact join; ~1e-12 on the
  float product). The S4 `weight` column we drop here is the natural carrier once
  a real switch/censor weight model is in scope — that needs the R solver and is
  out of scope for the deterministic engine.
- **Tier-2 golden pipeline (Phase 3):** unchanged from Phase 1.
- **Oracle hygiene:** the scenario `validate_input()` `data.table`-`by=` bug
  (worked around by simulating cohorts directly) still wants a one-line fix so
  `run_all.R` can regenerate the full ITT + PP set end-to-end.
