# Phase 6 — Weights in Rust (v2): Completion Summary & Sign-off

**Status: ✅ Implemented. The engine now *fits* the inverse-probability weights in
Rust by binding a mature logistic solver, reproducing the R `TrialEmulation`
factor table within a documented ~1e-6 tolerance across all 5 weight fixtures —
structural columns still bit-exact, applied-weight product still ~1e-12, and the
verified Phases 1–5 behaviour byte-identical.**
Date: 2026-06-30.

Phase 6's Definition of Done (from [`../../ROADMAP.md`](../../ROADMAP.md)) is
*"weights within documented tolerance of `parglm`; explicit statement of where
exactness ends."* The engine (`tte_expand::fit_weights` /
`expand_weighted_fitted_parquet`, behind the non-default `weights-fit` feature)
now **produces** the per-`(id, period)` factor table that Phase 3
([`apply_weights`]) consumes, by reconstructing the legacy
`data_preparation(use_censor_weights = …)` weight pipeline and binding
[`smartcore`]'s unregularised binomial-logit solver. Robust/sandwich variance and
the MSM coefficient model **stay in R** (out of scope, as in v1).

## What was implemented

### A. Weight fitting in the engine (`crates/tte-expand/src/fit.rs`, new)

A faithful, deterministic reconstruction of the legacy weight path, in three stages:

1. **Design preparation** — a port of the package's `data_manipulation` (drop
   pre-eligibility and post-event person-periods, lag `treatment` → `am_1`, derive
   the `switch` flag, set `eligible0/1` from `am_1`) **plus the compiled
   `censor_func`** (a ~60-line deterministic per-subject state machine, ported from
   the public `Causal-LDA/TrialEmulation/src/code.cpp`) for the per-protocol
   estimand. This is pure data transformation — Rust's job — not statistics.
2. **Model fitting** — bind `smartcore`'s `LogisticRegression` with `alpha = 0`
   (unregularised; deterministic L-BFGS from a zero start) to fit the switching
   weight models (numerator/denominator × `am_1` stratum) and/or the IPCW censoring
   models (`1 - censored ~ …`, pooled per `pool_cense`). The crate **does not**
   hand-roll IRLS; it only *binds* a mature solver and applies the fitted
   coefficients (sigmoid of the linear predictor = R's `predict(type="response")`).
3. **Combination** — per person-period, `wt = wt_switch · wtC` where the switch
   weight is `p_n/p_d` (or `(1-p_n)/(1-p_d)`) and the IPCW factor is `pC_n/pC_d`,
   exactly `TrialEmulation::weight_func`. The result is emitted as the
   `(id, period [Int32], weight_factor [Float64])` table `apply_weights` already
   joins — so fitting is **additive**: `fit_weights` → `apply_weights` reuses the
   verified Phase-3 cumulative-product application unchanged.

Public API (re-exported from the crate root, behind `weights-fit`):
`WeightSpec` / `SwitchWeightSpec` / `CensorWeightSpec` / `PoolCensor`,
`fit_weights(cohort, &opts, &spec) -> LazyFrame`, `fit_weights_parquet`, and the
end-to-end `expand_weighted_fitted_parquet(input, output, &opts, &spec)` (the
fully-in-Rust analogue of `expand_weighted_parquet`). A new
`ExpandError::WeightFit(String)` variant carries fit-specific failures.

### B. Feature gating (the binding stays lean)

`smartcore` is an **optional** dependency behind the **non-default** `weights-fit`
feature. The engine's `--all-features` CI exercises the fit; a plain `cargo build`
and the `tters` R binding (which never fits weights) stay solver-free, so the
binding's tree and `bindings/tters/src/rust/Cargo.lock` are **untouched**. The
new path is otherwise additive — `expand` / `apply_weights` / `ExpandOptions` and
every Phase 1–5 fixture remain byte-identical.

### C. Certificate extension (`crates/tte-expand/benches/certificate.rs`)

The reproducibility certificate now also **fits** the IPW models in Rust and
asserts `weight` within the staged `1e-6` for three cases (data_censored ITT-IPCW,
data_censored PP switch+IPCW, high_switching PP switch). §5 ("Tolerance contract —
where exactness ends") documents the staged boundary. `make certificate` runs with
`--features weights-fit`; `make verify` therefore asserts the fitted tolerance.

### D. Tests

- **Co-located regression net** — `src/fit.rs`'s `#[cfg(test)] mod tests`: all 5
  fixtures end-to-end (structural bit-exact + `weight` within 1e-6), plus a
  run-to-run determinism test.
- **Canonical contract test** — authored to
  `crates/tte-expand/tests-staging/weights_fit.rs` (the agent guard blocks writing
  under `tests/`); a human moves it with
  `git mv crates/tte-expand/tests-staging/weights_fit.rs crates/tte-expand/tests/weights_fit.rs`
  (exactly how Phase 4 shipped its testthat suite). Gated with
  `#![cfg(feature = "weights-fit")]`.

## VERIFY-FIRST findings (empirical, established before building)

| Unknown | Finding |
|---|---|
| **Fixtures** | No new fixtures needed. The committed `input_<name>_<estimand>_weights.parquet` factor tables **are** R's fitted per-`(id, period)` `wt`; the cohort inputs are present. An R reconstruction from the cohort inputs reproduced them to **machine epsilon**: ITT-IPCW **2.2e-16**, PP-switch (high_switching) **4.4e-16**, PP-combined (data_censored) **1.3e-15**. So the existing fixtures are the end-to-end validation target — nothing under `fixtures/`/`oracle/` changed. |
| **Solver** | `fit_glm` is plain `glm(binomial(logit))` IRLS — **`parglm` is deprecated and silently falls back to `glm`** on 0.0.4.11, so the target is *unregularised* logistic regression. A 3-way empirical bake-off vs R `glm` coefficients (real design matrices + a synthetic case): **smartcore 0.3.2** L-BFGS @ `alpha=0` matched to **1.6e-8**, linfa-logistic 0.7.1 to 2.1e-8, ndarray-glm 0.1.0 (true IRLS) to 3.2e-8. **Chose smartcore**: pure-Rust, Apache-2.0, 13 permissive deps, **no native BLAS/LAPACK** (ndarray-glm needs system OpenBLAS — a cross-machine FP-reproducibility hazard vs the determinism contract), `cargo deny` clean. |
| **`censor_func`** | The per-protocol design matrix depends on the package's **compiled C++** `censor_func`. It is a ~60-line deterministic per-subject state machine (fetched from public upstream `src/code.cpp`) — *data transformation, not statistics* — and is portable. Measured deletions: **0** rows for high/moderate/frequent_switching, **404/725** for data_censored PP; the Rust port reproduces both. |
| **Tolerance** | Staged **1e-6** relative on the fitted `weight` (ADR-2). Justified by the achieved Rust-vs-Oracle worst: high_switching PP **3.4e-7**, the other four ≤ **3.5e-8** — ~3× headroom under 1e-6. L-BFGS converges *to* the MLE, not bit-for-bit, so a tighter/bit-exact bound would be wrong. |
| **CI / cost** | Models are tiny (≤ 2405 rows, 1–3 covariates), sub-millisecond fits — negligible. Root CI (`--all-features`) exercises the fit (clippy/test/check/deny + the certificate bench); `make verify` adds the fitted assertion. No new heavy CI surface. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| Fitted weights (5 fixtures) — structural bit-exact, `weight` within 1e-6 | ✅ worst 3.4e-7 (high_switching PP); others ≤ 3.5e-8 |
| Run-to-run determinism (fit twice → byte-identical) | ✅ |
| `cargo test --workspace --all-features --all-targets --locked` (lib 45 / itt 17 / pp 17 / weights 5) | ✅ |
| `cargo test --workspace --all-features --doc --locked` | ✅ |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean |
| `cargo clippy` (default, **no** `weights-fit`) | ✅ clean (binding-lean path) |
| `cargo fmt --all --check` | ✅ |
| `cargo check --workspace --all-targets --all-features --locked` | ✅ |
| `cargo deny check` (smartcore tree) | ✅ advisories / bans / licenses / sources ok |
| `make verify` → certificate (47/47 integrity + fitted spot-checks within 1e-6) | ✅ |
| **Phases 1–5 unchanged** — itt 17 / pp 17 / weights 5 + certificate structural | ✅ byte-identical |
| smartcore absent from the default tree / `tters` binding | ✅ (feature-gated) |

## Decisions / deviations recorded

- **Where exactness ends (ADR-2 staged).** Expansion / per-protocol censoring →
  **bit-exact**. Weight *application* (cumulative product) → **~1e-12**. Weight
  *fitting* → **~1e-6** (the new, documented bound). The fit is L-BFGS-to-MLE, not
  bit-for-bit IRLS; chasing bit-parity with R's `glm` would be wrong (and is
  explicitly out of scope per the project plan §6 and ADR-2).
- **Solver = smartcore 0.3.2, bound not hand-rolled.** Selected after an empirical
  bake-off (smartcore/linfa/ndarray-glm all matched ~1e-8). smartcore wins on
  supply chain: pure-Rust, no system BLAS, smallest deny-clean tree, deterministic
  (zero-start L-BFGS, no RNG). ndarray-glm is the only *true IRLS* match but drags
  in system OpenBLAS — rejected on the determinism/build surface, not accuracy.
- **What stays in R.** Robust/sandwich variance and the MSM coefficient estimation
  — unchanged from v1. The engine fits only the IPW nuisance models (the
  deterministic-given-the-fit factor table); it computes no standard errors.
- **`censor_func` ported from public C++.** The one compiled dependency of the PP
  design matrix; ported as a deterministic row-keep state machine (like Phase 2's
  `cum_max`), validated by the data_censored PP fixture (404-row deletion).
- **Feature-gated `weights-fit` (off by default).** Keeps the R binding and a plain
  build solver-free and the binding lockfile untouched, while root CI's
  `--all-features` covers the fit. `make certificate` opts in explicitly.
- **No `unsafe`; tolerances in the harness, never in `src/`.** `smartcore`'s
  `Array` trait (which shadows std slice `.get`) is confined to one extractor
  helper so the rest of the module stays std-slice.

## Deferred to later phases

- **Expose fitting through the `tters` binding.** The R companion still calls the
  Phase-3 *application* path (pre-computed factors). Surfacing `fit_weights` /
  `expand_weighted_fitted_parquet` to R (and adding smartcore to the binding) is a
  deliberate follow-up; the binding lockfile is intentionally untouched here.
- **Additional weight-model shapes.** Only the model shapes the 5 fixtures exercise
  are validated (intercept + `x1`/`x2`; `pool_cense` none/numerator; `eligible_wts`
  unused). The `As-Treated` estimand, pooled-both censoring, `eligible_wts_*`
  gating, and richer covariate formulae are supported in code paths but not
  fixture-validated — out of scope until fixtures exist.
- **Standard errors / robust variance in Rust.** Permanently out of scope: R owns
  statistical estimation (the Rust-owns-transform / R-owns-estimation split).
