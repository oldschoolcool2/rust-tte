# Documentation

Design docs, research, and decision records for `tte-expand`.

## Convention

Documentation is organised into **numbered topic folders** and **numbered files**:

```
docs/
└── NNN-short-description/        # a numbered topic folder
    ├── 001-first-document.md     # numbered, ordered Markdown files
    ├── 002-second-document.md
    └── ...
```

- Folders: `NNN-short-description/` — `NNN` is a zero-padded sequence (`001`,
  `002`, …); the description is kebab-case.
- Files: `NNN-short-description.md` inside a folder, numbered in reading order.
- Numbers are **append-only** — never renumber an existing folder/file; add the
  next number.

## Index

| Folder | Contents |
|---|---|
| [`001-initial-ideations/`](001-initial-ideations/) | The originating research and plan: project memories, the simulation-inputs/validation literature review, the phased project plan, and the executable Phase-0 fixture-generation pre-work. |
| [`002-phase-0-scaffold/`](002-phase-0-scaffold/) | Phase-0 completion summary, verification results, decisions/deviations, and the remaining human sign-off items. |
| [`003-phase-1-itt-expansion/`](003-phase-1-itt-expansion/) | Phase-1 ITT engine: the self-join algorithm, the input-derived dtype rules, fixtures generated, bit-exact verification, and the deferred PP/edge-case sign-off items. |
| [`004-phase-2-pp-censoring/`](004-phase-2-pp-censoring/) | Phase-2 per-protocol engine: first-deviation artificial censoring (`cum_max`/window), the `PP = ITT ∩ S4-survivors` fixture recipe, the no-flag-column schema decision, bit-exact verification, and the ITT-unchanged proof. |
| [`005-phase-3-weight-application/`](005-phase-3-weight-application/) | Phase-3 weight application: the join + cumulative-product (`cum_prod`/window) design, the per-`(id, period)` factor table recovered from the Oracle weights, the `STRUCTURAL_COLS_WEIGHTED` schema, the 1e-12 tolerance, verification within tolerance, and the ITT/PP-unchanged proof. |
| [`006-phase-4-extendr-binding/`](006-phase-4-extendr-binding/) | Phase-4 extendr binding (`tters`): the FFI shim exposing estimand selection + weighted expansion to R, the regenerated extendr wrappers / NAMESPACE / man, the toolchain & version reconciliation (extendr 0.9 / rextendr 0.5 / R 4.3.3, MSRV bumped to 1.95), the `R CMD INSTALL` + testthat round-trip reproducing the full battery, and the root-workspace-unchanged proof. |
| [`007-phase-5-benchmark-certificate/`](007-phase-5-benchmark-certificate/) | Phase-5 benchmark + reproducibility certificate: the pure-cargo certificate generator (recompute every fixture SHA-256 vs the manifests + re-verify equivalence + record Oracle/toolchain pins), the criterion runtime micro-benchmarks, the R-vs-Rust runtime/peak-RSS harness incl. the R-OOM / Rust-OK regime, the Tier-2 whole-pipeline golden (Rust-expand → R-estimate matches upstream), the `make verify` entry point + CI, and the engine-unchanged proof. |
| [`008-phase-6-weights-in-rust/`](008-phase-6-weights-in-rust/) | Phase-6 weight *fitting* (optional v2): producing the per-`(id, period)` factor table in Rust by porting the legacy `data_manipulation` + the compiled `censor_func` state machine and binding `smartcore`'s unregularised logistic solver (chosen via a bake-off vs R `glm`), the staged ~1e-6 fitted-weight tolerance and where exactness ends, the non-default `weights-fit` feature that keeps the `tters` binding lean, the extended certificate, and the Phases-1–5-unchanged proof. |
| [`009-phase-7-expose-weight-fitting/`](009-phase-7-expose-weight-fitting/) | Phase-7 exposing weight *fitting* through the `tters` R binding: the `fit_trial_weights` / `expand_trial_weighted_fitted` ergonomic wrappers over the new `fit_weights_parquet` / `expand_weighted_fitted_parquet` extendr shims (the nested `WeightSpec` mapped from R character-vector/string inputs with `NULL`-dropped components, `ExpandError::WeightFit`→R-error mapping), enabling the `weights-fit` feature so `smartcore` enters the binding tree + lockfile (the deliberate converse of Phase 6), the `R CMD INSTALL` (debug + release) + testthat round-trip reproducing the 5 weight fixtures *by fitting* within 1e-6, and the certificate / `cargo deny` / root-workspace-unchanged proofs. |

### `001-initial-ideations/`

| File | What it is |
|---|---|
| [`001-project-memories.md`](001-initial-ideations/001-project-memories.md) | High-level orientation: what the project is and the key decisions. |
| [`002-research-simulation-inputs.md`](001-initial-ideations/002-research-simulation-inputs.md) | Literature review of simulation inputs (DGPs, coefficients, known-truth estimands) and validation criteria across three tiers. |
| [`003-project-plan.md`](001-initial-ideations/003-project-plan.md) | The authoritative roadmap: thesis, scope, ADRs, phases, agent loop protocol, copy-paste prompts. |
| [`004-prework-fixtures.md`](001-initial-ideations/004-prework-fixtures.md) | Phase-0 made concrete: the R Oracle scripts that produce the fixture battery and the three-tier validation map. |

### `002-phase-0-scaffold/`

| File | What it is |
|---|---|
| [`001-phase-0-summary.md`](002-phase-0-scaffold/001-phase-0-summary.md) | Phase-0 sign-off: what each agent built, the verification (compiles / clippy / fmt / test green), decisions (Polars 0.54.4, MSRV 1.88, `dtype-categorical`), and remaining human sign-off. |

### `003-phase-1-itt-expansion/`

| File | What it is |
|---|---|
| [`001-phase-1-summary.md`](003-phase-1-itt-expansion/001-phase-1-summary.md) | Phase-1 sign-off: the ITT self-join engine, the input-derived dtype contract, the fixtures generated from the Oracle, bit-exact verification (13 fixtures + invariants, all gates green), the two Oracle bugs found, and the deferred PP / `E04`–`E09` / golden items. |

### `004-phase-2-pp-censoring/`

| File | What it is |
|---|---|
| [`001-phase-2-summary.md`](004-phase-2-pp-censoring/001-phase-2-summary.md) | Phase-2 sign-off: per-protocol first-deviation censoring (`Estimand::PerProtocol`, `cum_max` over `(id, trial_period)` ordered by `followup_time`), the `PP = ITT ∩ S4-survivors` fixture recipe, the no-flag-column / ITT-consistent schema decision, bit-exact verification across 17 PP fixtures + the monotone-censoring invariant, and the ITT-unchanged proof. |

### `005-phase-3-weight-application/`

| File | What it is |
|---|---|
| [`001-phase-3-summary.md`](005-phase-3-weight-application/001-phase-3-summary.md) | Phase-3 sign-off: weight application (`apply_weights` / `expand_weighted_parquet`) as a join of the per-`(id, period)` factor + a `cum_prod` window over `(id, trial_period)` ordered by `followup_time`, the legacy-path weight fixtures + factor tables (recovered as the trial-invariant ratio), the `STRUCTURAL_COLS_WEIGHTED` schema, the relative-1e-12 tolerance, the adversarially-verified cumulative-product decision, verification within tolerance across 5 fixtures, and the ITT/PP-unchanged proof. |

### `006-phase-4-extendr-binding/`

| File | What it is |
|---|---|
| [`001-phase-4-summary.md`](006-phase-4-extendr-binding/001-phase-4-summary.md) | Phase-4 sign-off: the `tters` extendr shim (`expand_parquet` / `expand_weighted_parquet` with estimand selection + faithful `ExpandError`→R error mapping), the ergonomic `expand_trial` / `expand_trial_weighted` wrappers, the regenerated extendr wrappers / NAMESPACE / man, the VERIFY-FIRST toolchain & version reconciliation (extendr-api 0.9.0, rextendr 0.5.0, R 4.3.3; MSRV bumped 1.71→1.95 for Polars), the `R CMD INSTALL` (debug + release) + testthat round-trip reproducing the full battery (structural exact + `weight` within 1e-12), and the root-workspace-unchanged proof. |

### `007-phase-5-benchmark-certificate/`

| File | What it is |
|---|---|
| [`001-phase-5-summary.md`](007-phase-5-benchmark-certificate/001-phase-5-summary.md) | Phase-5 sign-off: the reproducibility certificate (`make verify` recomputes 47/47 fixture SHA-256 vs the manifests, re-verifies equivalence, and records Oracle + toolchain pins), the criterion `expand`/`apply_weights` micro-benchmarks, the R-vs-Rust runtime/peak-RSS harness with the R-OOM (≈5×10⁶) / Rust-OK (10⁷ in 2.5 s) regime, the Tier-2 whole-pipeline golden (Rust-expand → R-estimate matches upstream within tolerance), the VERIFY-FIRST findings (criterion/cargo-deny, R timing+RSS method, golden-tier decision, CI scoping), the CI job, and the engine-unchanged proof. |

### `008-phase-6-weights-in-rust/`

| File | What it is |
|---|---|
| [`001-phase-6-summary.md`](008-phase-6-weights-in-rust/001-phase-6-summary.md) | Phase-6 sign-off: weight *fitting* in Rust (`fit_weights` / `expand_weighted_fitted_parquet`, behind the non-default `weights-fit` feature) — porting `data_manipulation` + the compiled `censor_func` state machine and binding `smartcore`'s unregularised binomial-logit solver to produce the per-`(id, period)` factor table Phase 3 consumes, the VERIFY-FIRST findings (fixtures = machine-eps reconstruction, the `glm`/`parglm` bake-off → smartcore, the ~60-line `censor_func` port, the staged 1e-6 tolerance, CI), the staged tolerance + where exactness ends, the solver choice + what stays in R, the extended certificate, and the binding-lean / Phases-1–5-unchanged proofs. |

### `009-phase-7-expose-weight-fitting/`

| File | What it is |
|---|---|
| [`001-phase-7-summary.md`](009-phase-7-expose-weight-fitting/001-phase-7-summary.md) | Phase-7 sign-off: exposing in-Rust weight *fitting* through `tters` — the `fit_trial_weights` / `expand_trial_weighted_fitted` ergonomic wrappers over the new `fit_weights_parquet` / `expand_weighted_fitted_parquet` extendr shims (`NULL`-driven `WeightSpec` from R character-vector/string inputs, faithful `ExpandError::WeightFit`→R-error mapping), the VERIFY-FIRST findings (extendr-0.9 type mapping, smartcore × extendr build, the fitted-tolerance target, supply chain + certificate-lockfile robustness), the deliberate `bindings/tters/src/rust/Cargo.lock` change (smartcore enters — the converse of Phase 6), the `R CMD INSTALL` (debug + release) + testthat fit-round-trip reproducing the 5 weight fixtures within 1e-6, and the core-untouched / root-green / cert-parses-lockfile proofs. |
