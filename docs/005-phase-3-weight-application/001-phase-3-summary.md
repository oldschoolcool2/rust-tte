# Phase 3 — Weight Application: Completion Summary

**Status: ✅ Weight application implemented and verified against the Oracle within
tolerance; ITT and PP paths byte-identical.**
Date: 2026-06-29.

Phase 3's Definition of Done (from [`../../ROADMAP.md`](../../ROADMAP.md)) is
*"exact join; float product within 1e-12 (no solver — weights come from R)."* The
engine (`tte_expand::apply_weights` / `expand_weighted_parquet`) now reproduces
`TrialEmulation 0.0.4.11`'s **weighted** expanded frame: the six structural
columns match **bit-for-bit** and the per-row `weight` matches within a **relative
1e-12** tolerance, across all 5 weight fixtures (2 estimands × switch / censor /
combined weight models). The ITT and PP contracts are untouched, and `cargo`
fmt/clippy/test/check + `pre-commit` are green.

## What was implemented

### A. Weight application in the engine (`crates/tte-expand/src/lib.rs`)

The per-row `weight` is the **cumulative product**, within each
`(id, trial_period)` ordered by `followup_time`, of a pre-computed
per-`(id, period)` inverse-probability **factor** (SPEC §4):

```text
period      = trial_period + followup_time
mult        = 1.0                              if followup_time == 0   (baseline)
            = weight_factor(id, period)        otherwise               (left-join)
weight      = cum_prod(mult)  OVER (id, trial_period) ORDER BY followup_time
```

1. A free function `apply_weights(expanded, factors, &opts) -> LazyFrame` joins the
   factor table (`id, period, weight_factor`) onto the structural expansion on
   `(id, period)`, forces the baseline multiplier to `1.0`, takes the cumulative
   product, emits `weight` as **Float64**, and re-sorts to the canonical
   `(id, trial_period, followup_time)`. A convenience
   `expand_weighted_parquet(input, factors, output, &opts)` chains
   `expand` → `apply_weights` → write.
2. **ITT/PP untouched.** Weighting is a *separate* entry point; `ExpandOptions` and
   `expand`/`expand_parquet` are unchanged, so the default (no factor table)
   output stays bit-identical to Phases 1–2. The estimand selects the weight
   *model* upstream in R, but the application arithmetic (join + cumulative
   product) is identical for ITT and PP.

**Polars design.** The cumulative product uses the `cum_prod` window via
`over_with_options(...)` with an **explicit `order_by = followup_time`**, so the
accumulation is independent of physical row order — the deterministic `cum_prod`
analogue of Phase 2's `cum_max`. `cum_prod` is gated by the **already-enabled**
`cum_agg` feature (the same one Phase 2's `cum_max` uses), so **no new Polars
feature or dependency was added**. A missing factor on a follow-up row would
surface as a `null` weight (a loud failure), never a silent `1.0`. No `unsafe`, no
Rust `as` casts (Polars `.cast` only), no `unwrap`/`expect`/indexing in the
library path.

### B. The contract: fixtures, `STRUCTURAL_COLS_WEIGHTED`, SPEC §4, and the test

- **`oracle/00_setup.R`** gained `STRUCTURAL_COLS_WEIGHTED <- c(STRUCTURAL_COLS,
  "weight")` (the unweighted six-column ITT/PP contract is untouched).
- **`oracle/20_scenarios.R`** gained two genuinely-switching cohorts
  (`moderate_switching`, `frequent_switching`; `switch_prob` 0.15/0.35) — the
  existing scenarios use absorbing treatment (`switch_prob = 0`), so their
  switch-weight `glm` cannot converge.
- **`oracle/42_dump_weights.R`** (new) runs the **legacy
  `data_preparation(use_censor_weights = …)`** weight path and writes, per case,
  the weighted frame plus the per-`(id, period)` factor table (see *Fixtures*).
- **`SPEC.md` §4** was finalised (the cumulative-product rule, the join key, the
  schema, the worked micro-example) and **§5** gained the
  **weight cumulative-product invariant**.
- **`crates/tte-expand/tests/weights.rs`** (new) asserts, for all 5 cases, exact
  structural columns + `|weight − expected| ≤ 1e-12·max(1, |expected|)` with a
  worst-row diff. The in-crate module gained a self-contained
  `invariant_weight_cumulative_product` property test (baseline `weight == 1`,
  `weight > 0`, per-`(id, period)` factor invariant across overlapping trials,
  structural columns unchanged under weighting).

## Fixtures generated from the Oracle

Five weighted fixtures (`expected_<name>_<estimand>_weighted.parquet`) with their
factor tables (`input_<name>_<estimand>_weights.parquet`), under
`fixtures/weights/`:

| Cohort | Estimand | Weight model | Rows | `weight` range |
|---|---|---|---|---|
| `high_switching` | PP | switching | 8064 | [0.114, 14.56] |
| `moderate_switching` | PP | switching | 7620 | [0.047, 6.65] |
| `frequent_switching` | PP | switching | 7388 | [0.057, 11.13] |
| `data_censored` | PP | switching + censoring (IPCW) | 500 | [0.774, 1.45] |
| `data_censored` | ITT | censoring (IPCW) | 1558 | [0.432, 1.70] |

Structural inputs the engine expands are committed alongside:
`fixtures/scenarios/input_{moderate,frequent}_switching.parquet` (deterministic
from the seeded DGP) and `fixtures/weights/input_data_censored.parquet` (the
bundled package dataset). `MANIFEST_weights.json` records sha256 + row counts.

**The weight path.** On `TrialEmulation 0.0.4.11` only the **legacy**
`data_preparation(use_censor_weights = …)` path emits real weights; the S4
`calculate_weights()` path returned `weight ≡ 1.0` on `data_censored`. The legacy
PP row-set equals the S4 PP row-set (verified `setequal`), which equals the
engine's PP expansion — so the weighted frame aligns row-for-row with the engine's
structural output.

**The factor table (`weight_factor`).** `TrialEmulation` exposes only the final
cumulative `weight`, not the per-period contribution. The per-`(id, period)`
factor is therefore **recovered as the trial-invariant ratio**
`weight[t] / weight[t-1]` — proven invariant across the overlapping trials sharing
an `(id, period)` (max spread ≤ 4.4e-16; baseline `weight == 1.0` exactly). It is
R's per-period stabilised weight contribution; R owns the values (the `glm` fit),
the engine owns their cumulative application.

**Schema decision.** The weighted fixtures carry `STRUCTURAL_COLS_WEIGHTED` =
the six `STRUCTURAL_COLS` + `weight` (Float64), in the order
`id, trial_period, followup_time, assigned_treatment, treatment, outcome, weight`.
The unweighted ITT/PP `STRUCTURAL_COLS` contract and fixtures are unchanged.

R environment: `TrialEmulation 0.0.4.11`, `arrow 24.0.0`, `data.table 1.18.4`,
`digest 0.6.39`, `jsonlite 2.0.0` (R 4.3.3), Posit PPM binaries — same as
Phases 1–2.

## Verification performed (2026-06-29, Rust 1.95.0)

| Check | Result |
|---|---|
| Weight fixtures (5) — structural columns bit-exact, `weight` within 1e-12 | ✅ |
| Independent engine-algorithm reconstruction (R) vs Oracle `weight` | ✅ ≤ 6.1e-16 |
| Adversarial verification panel (4 cohorts, independent R) — cum-product claim | ✅ 4/4, none refuted |
| Baseline `weight == 1.0`; `weight > 0`; factor trial-invariant | ✅ |
| **ITT unchanged** — `tests/itt.rs` (17) + in-crate ITT all bit-exact | ✅ |
| **PP unchanged** — `tests/pp.rs` (17) + in-crate PP all bit-exact | ✅ |
| `invariant_weight_cumulative_product` property test | ✅ |
| `cargo test --workspace --all-features --all-targets --locked` (lib 39 / itt 17 / pp 17 / weights 5) | ✅ |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean |
| `cargo fmt --all --check` | ✅ clean |
| `cargo check --workspace --all-features --all-targets --locked` | ✅ |
| `pre-commit run --all-files` | ✅ all hooks pass |

## Decisions / deviations recorded

- **Cumulative product, not a per-row join.** Read off the Oracle and
  **adversarially verified** (4 independent analyses, none refuted): for
  `followup_time ≥ 2`, `weight ≠ single-period factor` (differences up to 12.7), so
  the running product is required. This is exactly why ADR-2 grants a ~1e-12 float
  tolerance instead of bit-exactness — the engine redoes the product and may
  reassociate relative to R.
- **Per-period factor recovered by factorisation.** `TrialEmulation` exposes only
  the final `weight`; the per-`(id, period)` factor is the (proven trial-invariant)
  ratio `weight[t]/weight[t-1]`. Exact and well-defined; R owns the values.
- **Legacy path, not S4.** S4 `calculate_weights()` produced `weight ≡ 1.0` on
  this version; the legacy `data_preparation` path emits real weights and is what
  the Oracle already uses for ITT/PP structure.
- **Cohort feasibility.** Only cohorts with *actual* treatment switching
  (`high_switching` + two new switching scenarios) or explicit censoring
  (`data_censored`) can carry weights. The originally-considered `common` /
  `strong_confounding` were dropped: with absorbing treatment their switch `glm`
  has no events and fails to converge (non-portable weights).
- **ITT-IPCW model.** The pooled-censor numerator path has a covariate-scoping bug
  on 0.0.4.11 (`object 'x1'/'x2' not found`) for intercept-only/`x1` numerators;
  `cense_n_cov = ~x2, cense_d_cov = ~x2, pool_cense = "numerator"` fits cleanly.
- **Tolerance = relative 1e-12** (ADR-2), defined in the test harness, never in
  `src/`. Observed engine-vs-Oracle reassociation is ~1e-15.
- **No new Polars feature/dependency** — `cum_prod` rides the already-enabled
  `cum_agg` feature.
- **SPEC §4 clarification.** The Phase-1 draft said the join key was
  `(id, trial_period, followup_time)` and "multiply the IPCW column"; the finalised
  §4 specifies the faithful design — join the per-`(id, period)` factor and take
  the cumulative product (the factor is genuinely keyed by person-period and reused
  across overlapping trials).

## Deferred to later phases

- **extendr binding (Phase 4):** expose `expand` / `apply_weights` to R via
  `tters`; round-trip the full battery.
- **Tier-2 golden certificate (Phase 5):** whole-pipeline goldens via
  `initiators()` (robust coefficients/SE) — unchanged from Phase 1.
- **Weight-model *fitting* (v2, out of scope):** `glm`/`parglm`/`sandwich` stay in
  R. The engine reproduces the deterministic *application* only; the per-period
  factor is and remains an R input.
- **Oracle hygiene:** `oracle/run_all.R` still does not wire in
  `42_dump_weights.R` and remains subject to the known scenario `validate_input()`
  `data.table`-`by=` bug (the new generator is standalone and dodges it via direct
  simulation).
