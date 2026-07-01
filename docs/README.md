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
| [`010-phase-8-in-memory-frame-io/`](010-phase-8-in-memory-frame-io/) | Phase-8 in-memory (frame-in / frame-out) I/O for `tters`: the four `*_df` entry points (`expand_trial_df` / `expand_trial_weighted_df` / `fit_trial_weights_df` / `expand_trial_weighted_fitted_df`) that take a cohort `data.frame` and return a `data.frame` with no intermediate Parquet — a `frame.rs` marshalling module (column-wise typed vectors, R `integer`↔`Int32` / `double`↔`Float64`, NA-aware, loud on `integer64`/`factor`/`Int64`) composing the already-public `expand` / `apply_weights` / `fit_weights` core LazyFrame API in memory, the VERIFY-FIRST findings (battery is `Int32`/`Float64`-only ⇒ base R is dtype-exact with no new deps; extendr-0.9 / polars-0.54 marshalling API; in-memory reproduces the battery), the dedicated-`*_df`-vs-`return=` decision and `polars` declared directly (one lockfile edge, no new versions), the `R CMD INSTALL` (debug + release) + 49-test in-memory testthat round-trip, and the core-untouched / root-`make verify`-green / `cargo deny`-clean proofs. |
| [`011-phase-9-exact-wide-integer-frame-io/`](011-phase-9-exact-wide-integer-frame-io/) | Phase-9 exact wide-integer frame I/O for `tters`: closing the Phase-8 64-bit gap so an R `bit64::integer64` column (a real cohort's 64-bit `id`) round-trips through the in-memory `*_df` path *exactly* — `frame.rs` now reinterprets the bits (`f64::to_ne_bytes` ↔ `i64::from_ne_bytes`, `i64::MIN`↔null) for `Int64` in/out (plus `UInt32`/`UInt64`-that-fits), with NO `unsafe`, NO Arrow C Data Interface, and NO precision loss above `2^53` (where a naive double-cast corrupts). The VERIFY-FIRST findings (`integer64` is a REALSXP of i64 bits, NA = `i64::MIN`, `as.data.frame()` preserves the class; the extendr `ToVectorValue for i64` lossy-cast trap; the battery is `Int32`/`Float64`-only ⇒ unaffected), the safe-bit-reinterpret-vs-Arrow-C-interface decision (the latter needs `unsafe`), the transparent (unchanged) `*_df` surface + `bit64` in `Imports`-not-`NAMESPACE`, the `R CMD INSTALL` (debug + release) + the 384-check battery regression + a new 56-check synthesized `integer64`-id round-trip (value + storage class exact, distinctness, fitted within 1e-6), and the no-new-dep / lockfiles-unchanged / core-untouched / `make verify`-green proofs. |
| [`012-phase-10-distribution-readiness/`](012-phase-10-distribution-readiness/) | Phase-10 distribution & self-contained installability for `tters`: making the R package buildable, installable, and self-testing OUTSIDE the monorepo so it can ship via r-universe / a source tarball (the prerequisite for the upstream `te_datastore` companion-backend outreach). Closes the path-dep blocker (`R CMD build` tarballs only the subdir; `cargo vendor` never vendors an escaping `path` dep) by keeping the committed `path` dep as the dev default and **synthesizing a `git`+pinned-`rev` form at distribution time** — a `.prepare` r-universe hook (dynamic-rev rewrite, build-time network fetch) and a `build-offline-tarball.sh` that `cargo vendor`s the core (baking its workspace inheritance ⇒ identical Polars features ⇒ bit-exact) + crates.io deps into a gitignored, release-only `vendor.tar.xz`. Adds a ~170 KB `inst/extdata` fixture subset so the installed testthat runs (not `skip()`s) against `system.file("extdata")`. The VERIFY-FIRST findings (the reproduced break; the adversarially-verified r-universe build model; git-dep-vs-in-package-copy as a determinism decision; the `paths`-override-only-for-crates.io gotcha; the subset; the 638 MB footprint), the decisive **monorepo-absent offline install + 444-pass/0-fail battery vs `inst/extdata`** proof, and the committed-tree-pristine / core-byte-identical / `make verify`-green / not-CRAN proofs. |
| [`013-phase-11-te-datastore-backend/`](013-phase-11-te-datastore-backend/) | Phase-11 `te_datastore` companion backend: wiring the verified `tters` engine into upstream `TrialEmulation` (maintainer-green-lit, `#243`) so a `trial_sequence()` pipeline runs the **expansion in Rust** while sampling + the MSM fit stay in **R**, consuming the Rust output **bit-identically** to the default path. R-only glue over the Phase-8/9 binding (no Rust changed). A `te_datastore_tters` S4 subclass + `save_to_tters()` constructor + `save_expanded_data`/`read_expanded_data`/`show` (sampling **inherits** the base method for RNG-identical parity), a drop-in `expand_trials_tters()` that maps the `tters` `*_df` output to the exact keeplist frame (read R's `wt` verbatim → accumulate in Rust; re-sort to the stored `index` order; join baseline adjustment covariates), and `Suggests`-level opt-in with graceful fallback. The VERIFY-FIRST findings (SEAM confirmation; structural-bit-exact + `weight`-to-machine-ε data-contract equivalence across ITT/PP × weighted/unweighted; downstream `load`/`sample`/`fit_msm` parity to 1.4e-11; chunk-invariance; the #243 reconciliation), the 8-test/58-assertion testthat parity suite, and the core-untouched / lockfiles-unchanged / debug-`R CMD INSTALL` proofs. |

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

### `010-phase-8-in-memory-frame-io/`

| File | What it is |
|---|---|
| [`001-phase-8-summary.md`](010-phase-8-in-memory-frame-io/001-phase-8-summary.md) | Phase-8 sign-off: in-memory (frame-in / frame-out) I/O for `tters` — the `frame.rs` marshalling module (dtype-exact column-wise R `data.frame` ↔ Polars frame, NA-aware, loud on `integer64`/`factor`/`Int64`) and four `*_df` shims/wrappers (`expand_trial_df` / `expand_trial_weighted_df` / `fit_trial_weights_df` / `expand_trial_weighted_fitted_df`) composing the already-public `expand` / `apply_weights` / `fit_weights` LazyFrame API in memory (no intermediate Parquet, no core change), the VERIFY-FIRST findings (structural battery is `Int32`/`Float64`-only ⇒ base R is dtype-exact with no new R deps; the extendr-0.9 / polars-0.54 marshalling API; the battery reproduced through the round-trip), the dedicated-`*_df`-surface and direct-`polars`-dep decisions (one lockfile edge, no new versions; `arrow` declared in `Suggests`), the `R CMD INSTALL` (debug + release) + 49-test in-memory testthat round-trip (17 ITT + 17 PP bit-exact + dtype-exact, 5 weight fixtures by applying ≤1e-12 and by fitting ≤1e-6), and the core-untouched / root-`make verify`-green / `cargo deny`-clean proofs. |

### `011-phase-9-exact-wide-integer-frame-io/`

| File | What it is |
|---|---|
| [`001-phase-9-summary.md`](011-phase-9-exact-wide-integer-frame-io/001-phase-9-summary.md) | Phase-9 sign-off: exact wide-integer (`bit64::integer64`) in-memory frame I/O for `tters` — closing the Phase-8 gap so a 64-bit `id` round-trips through the `*_df` path *exactly*. `frame.rs` now reinterprets the bits (`f64::to_ne_bytes` ↔ `i64::from_ne_bytes`) for `Int64` in/out (plus `UInt32` and `UInt64`-that-fits-`i64`), maps `i64::MIN` ↔ Polars null (`bit64` NA), and keeps the `factor` + `UInt64`-overflow guards — NO `unsafe`, NO Arrow C Data Interface, NO precision loss above `2^53`. The VERIFY-FIRST findings (`integer64` = a REALSXP whose 8 bytes are the i64, NA = `i64::MIN`, `as.data.frame()` preserves the class; the extendr `ToVectorValue for i64` lossy-cast trap → collect `f64`; the battery is `Int32`/`Float64`-only ⇒ unaffected), the safe-bit-reinterpret-vs-Arrow-C-Data-Interface decision (the latter needs `unsafe`), the transparent (unchanged) `*_df` / `extendr_module!` / `NAMESPACE` surface and `bit64` in `Imports`-not-`NAMESPACE`, the `R CMD INSTALL` (debug + release) + 3 new Rust unit tests + the 384-check Phase-8 battery regression + a new 56-check synthesized `integer64`-id round-trip (value + storage class exact, adjacent-id distinctness, id+period+treatment dtypes, fitted within 1e-6), and the no-new-Rust-dep / both-lockfiles-unchanged / `NAMESPACE`-unchanged / core-untouched / root-`make verify`-green / `cargo deny`-clean proofs. |

### `012-phase-10-distribution-readiness/`

| File | What it is |
|---|---|
| [`001-phase-10-summary.md`](012-phase-10-distribution-readiness/001-phase-10-summary.md) | Phase-10 sign-off: distribution & self-contained installability for `tters` — making the package buildable, installable, and self-testing OUTSIDE the monorepo (r-universe / source tarball), the prerequisite for the upstream `te_datastore` outreach. The committed `bindings/tters/src/rust/Cargo.toml`/`Cargo.lock` keep the `path` dep (dev default, byte-identical), and the distributable `git`+pinned-`rev` form is **synthesized at dist time**: `tools/rewrite-core-dep.sh` (surgical, `--locked`-clean lock edit), the `.prepare` r-universe hook (dynamic `git rev-parse HEAD`, build-time network fetch), and `tools/build-offline-tarball.sh` (`cargo vendor`s the git core — baking its workspace inheritance ⇒ identical Polars features ⇒ bit-exact — + crates.io deps into a gitignored, release-only `vendor.tar.xz` the existing Makevars plumbing installs offline). D1 vendors a ~170 KB `inst/extdata` subset (9 edge + `common` + `data_censored`) so the testthat resolver's `system.file("extdata")` branch runs the real battery standalone; D3 adds `tools/r-universe/packages.json` + the go-live recipe; D4 adds `inst/NOTICE` (upstream Apache-2.0 attribution + example-data provenance) and the gitignore/Rbuildignore hygiene. The VERIFY-FIRST findings (reproduced break; adversarially-verified r-universe build model + build-time network; git-dep-vs-copy as a determinism decision; the `paths`-override-crates.io-only gotcha; the 638 MB footprint; CRAN out), the decisive **monorepo-absent `CARGO_NET_OFFLINE` install (576 crates from vendor) + 444-pass/0-fail battery vs `inst/extdata`**, and the committed-tree-pristine / core-byte-identical (vendored vs `crates/tte-expand/src`) / `cargo deny`-clean / root-`make verify`-green proofs. |

### `013-phase-11-te-datastore-backend/`

| File | What it is |
|---|---|
| [`001-phase-11-summary.md`](013-phase-11-te-datastore-backend/001-phase-11-summary.md) | Phase-11 sign-off: the `te_datastore` companion backend wiring `tters` into upstream `TrialEmulation` (maintainer-green-lit, `#243`) — a `trial_sequence()` pipeline runs the **expansion in Rust** while sampling + the MSM fit stay in **R**, bit-identical to the default path. R-only glue (no Rust changed). The `te_datastore_tters` S4 subclass + `save_to_tters()` + `save_expanded_data`/`read_expanded_data`/`show` (sampling **inherits** the base method ⇒ RNG-identical), a drop-in `expand_trials_tters()` mapping the `tters` `*_df` output to the exact keeplist frame (read R's `wt` verbatim → accumulate in Rust; re-sort to the stored `(id, period_new, trial_period)` order; join baseline adjustment covariates at `period == trial_period`), and `Suggests`-level opt-in + graceful fallback (AT/absent-TE → R). The VERIFY-FIRST findings (SEAM = storage-not-compute; structural-bit-exact + `weight`-to-machine-ε equivalence across ITT/PP × weighted/unweighted; the row-order re-sort make-or-break; downstream `load`/`sample`/`fit_msm` parity to 1.4e-11; chunk-invariance; #243 reconciliation — interface/pin/contract assumptions stand), the 8-test/58-assertion testthat parity suite, the inherit-`sample_expanded_data` / read-`wt`-verbatim / interface decisions, and the core-untouched / both-lockfiles-unchanged / debug-`R CMD INSTALL` proofs. AT `dose` path + a formal vignette deferred. |
