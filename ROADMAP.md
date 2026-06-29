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
| **5 — Benchmark + certificate** | criterion vs upstream; validation report | Report shows bit-exact equivalence + speed/memory curves; runs in CI. |
| **6 — (optional, v2) Weights in Rust** | Bind a mature logistic solver | Weights within documented tolerance of `parglm`; explicit statement of where exactness ends. |

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
