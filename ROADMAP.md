# tte-expand — Roadmap

> This is the condensed phase plan. The full, authoritative roadmap (with thesis,
> architecture decisions, agent loop protocol, and copy-paste prompts) lives in
> [`docs/001-initial-ideations/003-project-plan.md`](docs/001-initial-ideations/003-project-plan.md).

## Scope

| In scope (v1) | Out of scope (v1 — stays in R) |
|---|---|
| Sequential expansion: long input → expanded trial frame | Pooled-logistic weight *fitting* (`parglm`) |
| ITT expansion (carry assigned treatment forward) | Robust / sandwich variance (`sandwich`) |
| Per-protocol artificial censoring (`expand_until_switch`) | MSM coefficient estimation, CIs |
| Weight *application* (multiply pre-computed weights) | Any novel methodology |
| extendr binding + R companion package (`tters`) | Clone-censor-weight (CCW) grace-period design |
| Reproducibility validation suite + benchmark | Bayesian / MCMC anything |

**Rule of thumb:** Rust owns deterministic data transformation; R keeps
statistical estimation.

## Architecture decisions (ADRs)

- **ADR-1 — R is the Oracle.** The package run on fixed seed data produces
  immutable expected outputs; Rust's only job is to match them. Never "fix" the
  Oracle to make Rust pass.
- **ADR-2 — Tolerance is staged.** Expansion / censoring flags → **exact**.
  Weight application → exact join, ~1e-12 on the float product. Anything with a
  solver (v2+) → a documented, harness-locked tolerance.
- **ADR-3 — Fixtures are Parquet, never CSV.** Preserve dtypes.
- **ADR-4 — Polars (lazy) engine, `#![forbid(unsafe_code)]`.** Out-of-core via
  lazy/streaming to beat the RAM wall.
- **ADR-5 — extendr is the bridge, R is the first target.**
- **ADR-6 — Feed the agent a behavioural spec, not R source.** When `SPEC.md`
  and a fixture disagree, the fixture wins and the agent flags it.

## Phases & Definitions of Done

| Phase | Goal | Definition of Done |
|---|---|---|
| **0 — Scaffold** ✅ | Repo, tooling, Oracle, failing harness | **Done (2026-06-29).** Workspace compiles; `clippy -D warnings` / `fmt` / `test` green; lockfiles committed (Polars 0.54.4, MSRV 1.95 — Polars requires latest stable). Fixture generation + `STRUCTURAL_COLS` freeze remain — see [Phase-0 summary](docs/002-phase-0-scaffold/001-phase-0-summary.md). |
| **1 — ITT expansion** ✅ | `expand()` / `expand_parquet()` | **Done (2026-06-29).** Bit-exact match (schema + values + order) on every generated ITT fixture — all 9 edge cases `E01`–`E09` + 8 simulated scenarios; invariants (incl. re-entry assignment) + `forbid(unsafe_code)` hold; fmt/clippy/test/check + pre-commit green. `E06`'s PP view + golden/weights deferred to Phase 2/3. See [Phase-1 summary](docs/003-phase-1-itt-expansion/001-phase-1-summary.md). |
| **2 — Per-protocol censoring** ✅ | `expand_until_switch` deviation logic | **Done (2026-06-29).** Bit-exact match on every PP fixture — all 9 edge cases `E01`–`E09` + 8 scenarios — incl. the ITT-vs-PP divergence (`E02`/`E04`/`E06` + scenarios) and `E06`'s switch-back trap; first-deviation censoring via `cum_max` window (deviating row excluded, no switch-back resume); PP keeps the same six `STRUCTURAL_COLS` as ITT (`PP = ITT ∩ S4-survivors`, no flag column); ITT path byte-identical; monotone-censoring invariant + `forbid(unsafe_code)` hold; fmt/clippy/test/check + pre-commit green. See [Phase-2 summary](docs/004-phase-2-pp-censoring/001-phase-2-summary.md). |
| **3 — Weight application** ✅ | Join + cumulative IPCW product | **Done (2026-06-29).** Structural columns bit-exact + per-row `weight` within relative 1e-12 across all 5 weight fixtures (2 estimands × switch / censor / combined models); `weight = cum_prod` of the per-`(id, period)` factor over `(id, trial_period)` ordered by `followup_time` (the adversarially-verified cumulative-product design, not a per-row join); `STRUCTURAL_COLS_WEIGHTED` = the six structural cols + `weight`; ITT/PP paths byte-identical; weight cumulative-product invariant + `forbid(unsafe_code)` hold; no new Polars feature (`cum_prod` rides `cum_agg`); fmt/clippy/test/check + pre-commit green. See [Phase-3 summary](docs/005-phase-3-weight-application/001-phase-3-summary.md). |
| **4 — extendr binding** ✅ | `tters` R-callable wrapper | **Done (2026-06-29).** The `tters` extendr shim exposes the verified core to R — `expand_parquet` / `expand_weighted_parquet` with estimand selection (`"ITT"`/`"PP"`), `eligible`/`outcome` overrides, and faithful `ExpandError`→R-error mapping — plus ergonomic `expand_trial` / `expand_trial_weighted` wrappers. `R CMD INSTALL` succeeds (debug + release); an R round-trip reproduces the **full battery** (17 ITT + 17 PP structural bit-exact + 5 weighted within rel 1e-12) through the binding. extendr-api 0.9.0 / rextendr 0.5.0 / R 4.3.3 reconciled; binding MSRV bumped 1.71→1.95 (Polars). Core/contract untouched; root workspace green; both lockfiles committed. See [Phase-4 summary](docs/006-phase-4-extendr-binding/001-phase-4-summary.md). |
| **5 — Benchmark + certificate** ✅ | criterion vs upstream; validation report | **Done (2026-06-30).** The reproducibility certificate (`make verify`) recomputes **47/47** fixture SHA-256 against the manifests, re-verifies equivalence (structural bit-exact + `weight` within 1e-12), and records the Oracle + toolchain pins — pure cargo, fails on drift. Criterion runtime + R-vs-Rust peak-RSS curves show the **R-OOM (≈5×10⁶) / Rust-OK (10⁷ in 2.5 s, 2.74 GiB, exact row parity)** regime; the Tier-2 whole-pipeline golden (Rust-expand → R-estimate) matches upstream within tolerance. CI runs `make verify` + a bench smoke. Engine/contract untouched. See [Phase-5 summary](docs/007-phase-5-benchmark-certificate/001-phase-5-summary.md). |
| **6 — (optional, v2) Weights in Rust** ✅ | Bind a mature logistic solver | **Done (2026-06-30).** The engine now *fits* the IPW weights (`fit_weights` / `expand_weighted_fitted_parquet`, behind the non-default `weights-fit` feature): a faithful port of the legacy `data_manipulation` + the compiled `censor_func` state machine feeds a bound **`smartcore`** unregularised binomial-logit solver (chosen over linfa-logistic / ndarray-glm via a bake-off vs R `glm` — `parglm` is deprecated → `glm`), producing the per-`(id, period)` factor table Phase 3 consumes. All 5 weight fixtures match within the staged **1e-6** (ADR-2; observed worst 3.4e-7) — structural still bit-exact, applied product still 1e-12, fit deterministic. Robust/sandwich variance + the MSM stay in R. The `tters` binding stays solver-free (feature-gated); the certificate asserts the new tolerance; fmt/clippy/test/check/deny + `make verify` green. See [Phase-6 summary](docs/008-phase-6-weights-in-rust/001-phase-6-summary.md). |
| **7 — (optional, v2) Expose weight fitting via `tters`** ✅ | Surface in-Rust IPW *fitting* to R | **Done (2026-06-30).** The `tters` binding can now *fit* the IPW switch/IPCW models in Rust end-to-end — new ergonomic `fit_trial_weights()` / `expand_trial_weighted_fitted()` wrappers (over `fit_weights_parquet` / `expand_weighted_fitted_parquet` extendr shims) take a raw cohort straight to a weighted, expanded frame with **no pre-computed factor table**. The nested `WeightSpec` is passed ergonomically — covariates as character vectors, estimand / censor column / pool as strings, `NULL` drops a component — and `ExpandError::WeightFit` maps to a clean R error (the existing mapping). Enabling `features = ["weights-fit"]` pulls **`smartcore`** into the binding, so `bindings/tters/src/rust/Cargo.lock` now records it (the deliberate converse of Phase 6, which kept it untouched). `R CMD INSTALL` succeeds (debug + release); a testthat round-trip reproduces all **5 weight fixtures by *fitting*** (structural bit-exact, `weight` within the staged 1e-6). Core/contract untouched; root `make verify` green; the certificate still parses the binding lockfile; `cargo deny` clean. See [Phase-7 summary](docs/009-phase-7-expose-weight-fitting/001-phase-7-summary.md). |
| **8 — (optional, v2) In-memory frame I/O via `tters`** ✅ | Cohort `data.frame` → result `data.frame` (no Parquet) | **Done (2026-06-30).** The `tters` binding now runs **frame-in / frame-out**: four new ergonomic `*_df` wrappers (`expand_trial_df` / `expand_trial_weighted_df` / `fit_trial_weights_df` / `expand_trial_weighted_fitted_df`, over matching `#[extendr]` `*_df` shims) take an in-memory cohort `data.frame` and return a `data.frame`, with **no intermediate Parquet** on the critical path. A new `frame.rs` marshals R columns ↔ Polars **dtype-exactly** (R `integer`↔`Int32`, `double`↔`Float64`, NA-aware; loud on `integer64`/`factor`/`Int64`) and **composes the already-public `expand` / `apply_weights` / `fit_weights` LazyFrame API in memory** — *no core change*. The structural battery is `Int32`/`Float64`-only, so base R is dtype-exact with **no new R-side dependency** (only test-time `arrow`, now declared in `Suggests`). `R CMD INSTALL` succeeds (debug + release); a 49-test in-memory testthat round-trip reproduces the battery — 17 ITT + 17 PP structural bit-exact + dtype-exact, the 5 weight fixtures by *applying* (≤1e-12) and by *fitting* (≤1e-6). `polars` is declared directly in the binding (one added lockfile edge, no new versions; root lockfile unchanged); core/contract untouched; root `make verify` green; `cargo deny` clean. See [Phase-8 summary](docs/010-phase-8-in-memory-frame-io/001-phase-8-summary.md). |
| **9 — (optional, v2) Exact wide-integer frame I/O via `tters`** ✅ | 64-bit (`integer64`) columns round-trip exactly through the in-memory path | **Done (2026-06-30).** Closes the Phase-8 64-bit gap: an R `bit64::integer64` column (a real cohort's 64-bit `id`) marshals to a Polars `Int64` and back **exactly**. `frame.rs` reinterprets the bits (`f64::to_ne_bytes` ↔ `i64::from_ne_bytes`, a bitcast — NOT a numeric cast) for `Int64` in/out (plus `UInt32` and `UInt64`-that-fits-`i64`), maps `i64::MIN` ↔ Polars null (`bit64` NA), and keeps the `factor` + `UInt64`-overflow guards — **NO `unsafe`, NO Arrow C Data Interface, NO precision loss above `2^53`** (where a naive double-cast corrupts). The four `*_df` signatures, `extendr_module!`, and `NAMESPACE` are **unchanged** (handling is transparent; `as.data.frame()` preserves the `integer64` class); `bit64` is declared in `DESCRIPTION: Imports` (a hard `arrow` dep already) but not imported into the namespace. `R CMD INSTALL` succeeds (debug + release); the Phase-8 battery still reproduces (384 checks) and a new synthesized `integer64`-id round-trip passes (56 checks: value + storage class exact, adjacent-id distinctness, fitted within 1e-6), plus 3 new Rust unit tests. **No new Rust dependency; both lockfiles unchanged.** Core/contract/`fixtures`/`oracle`/`SPEC.md` untouched; root `make verify` green (47/47); `cargo deny` clean. See [Phase-9 summary](docs/011-phase-9-exact-wide-integer-frame-io/001-phase-9-summary.md). |

## The adversarial fixture battery (the moat)

The fixtures are an **epidemiology task, not a Rust task** — happy-path fixtures
pass while logic is subtly wrong. Cases (graded difficulty):

1. Patient eligible at **multiple** `trial_period`s (core behaviour) vs only baseline.
2. Event/censoring **on the trial baseline visit** (`followup_time = 0`).
3. Treatment switch **exactly at a trial boundary**.
4. **ITT vs PP divergence** on the same patient.
5. **Last-period eligibility** → single-row trials.
6. **Ties** in event/censor timing.
7. A patient who **never initiates**.
8. Eligible → ineligible → **eligible again** (re-entry).
9. Minimal fixtures: 1 patient / 1 period; 1 patient eligible every period (max fan-out).

## Contribution pathway

1. Engage `Causal-LDA/TrialEmulation` maintainers early (issue proposing an
   optional Rust expansion backend).
2. Ship as a companion (`tters`) first, not a fork.
3. License Apache-2.0; preserve upstream `NOTICE`; cite example data.
4. Write up via JOSS + a short repro/methods note (ENCePP / RWE framing).
5. Positioning: "verified high-performance backend for the gold-standard
   sequential TTE tool", explicitly *with* the maintainers.
