# Phase 7 — Expose Weight Fitting via `tters`: Completion Summary

**Status: ✅ Implemented. The `tters` R binding now *fits* the inverse-probability
weights in Rust end-to-end — an R user goes straight from a raw cohort to a
weighted, expanded trial frame with no pre-computed factor table. All 5 weight
fixtures reproduce *through the FFI round-trip by fitting* — structural columns
bit-exact, `weight` within the staged ~1e-6 (ADR-2) — and the verified core,
contract suite, and Phases 1–6 behaviour are byte-identical.**
Date: 2026-06-30.

Phase 7 is the first deferred item from Phase 6: *"Expose fitting through the
`tters` binding."* Until now the R companion only called the Phase-3 **application**
path (it took a pre-computed factor table). The binding now surfaces the Phase-6
**fitting** surface (`tte_expand::fit_weights_parquet` /
`expand_weighted_fitted_parquet`, behind the `weights-fit` feature) with an
ergonomic R signature, so the switch/IPCW models are fitted in Rust and the nested
`WeightSpec` is built from plain R inputs. Robust/sandwich variance and the MSM
coefficient model **stay in R** (out of scope, as in v1).

## What was implemented

Edits are confined to `bindings/tters/**` (the binding shim, R wrappers,
NAMESPACE/man, the staged testthat) plus this Phase-7 doc folder. The verified
core crate (`crates/tte-expand/**`), the contract suite, `fixtures/`, `oracle/`,
and `SPEC.md` are **untouched**.

### A. The extendr shim (`bindings/tters/src/rust/src/lib.rs`)

Two new `#[extendr]` functions, thin FFI shims over the Phase-6 surface, mirroring
the existing `expand_parquet` / `expand_weighted_parquet` pattern (flat scalar args
at the boundary; every `tte_expand::ExpandError` mapped via
`.map_err(|e| Error::Other(format!("tte-expand: {e}")))` — so `ExpandError::WeightFit`
surfaces as a clean R error condition, no new mapping):

- `fit_weights_parquet(...)` → fits the IPW models and writes the
  `(id, period, weight_factor)` factor table (`tte_expand::fit_weights_parquet`).
- `expand_weighted_fitted_parquet(...)` → fits, expands, applies, and writes the
  weighted frame in one call (`tte_expand::expand_weighted_fitted_parquet`).

Both take the cohort/output paths and `ExpandOptions` scalars, plus the flattened
`WeightSpec`: `use_switch` / `use_censor` gate flags, the four covariate-name
vectors as extendr `Strings` (R character vectors), `censor_col`, and `pool_censor`.
Three small helpers assemble the core types: `parse_pool` (`"none"`/`"numerator"`/
`"both"` → `PoolCensor`, loud on a typo, mirroring `parse_estimand`), `covariates`
(`Strings` → `Vec<String>`), and `build_weight_spec`, which threads the gate flags
through the **builder** constructors (`WeightSpec::switching` / `ipcw` /
`with_censor`) — `WeightSpec` and the `*Spec` types are `#[non_exhaustive]`, so they
are assembled through their public constructors, never a struct literal across the
crate boundary. The four new functions are registered in `extendr_module!`.

### B. The R surface (`bindings/tters/R/tters-package.R`, NAMESPACE, man/)

Two ergonomic, user-facing wrappers, matching the style of `expand_trial` /
`expand_trial_weighted` (defaults, `stopifnot` validation, integer coercion,
`invisible(output_path)`):

- `fit_trial_weights(input_path, output_path, …)` — writes the factor table.
- `expand_trial_weighted_fitted(input_path, output_path, …)` — raw cohort →
  weighted, expanded frame in one call.

The nested spec is **NULL-driven**: a switching model is fitted iff either
`switch_numerator` / `switch_denominator` is non-`NULL`; an IPCW model iff
`censor_col` is non-`NULL`. An internal `.tters_weight_spec()` resolves the
`NULL`-driven arguments into the flat `(use_*, character-vector)` form the shims
expect (`NULL` → `character(0)`; an absent component is dropped, not emptied).
Covariates are character vectors of column names; estimand / censor column / pool
are strings — so the R caller never touches the Rust type system.

The low-level `R/extendr-wrappers.R` was regenerated with the `document` bin
(`cargo run --bin document`, the rextendr-deprecation workaround), and `NAMESPACE`
+ `man/*.Rd` with `roxygen2::roxygenise(load_code = roxygen2::load_source)` — no
`rextendr::document()` (deprecated → `devtools`, not installed). The binding now
exports **8** functions (4 new: `fit_weights_parquet`, `expand_weighted_fitted_parquet`,
`fit_trial_weights`, `expand_trial_weighted_fitted`).

### C. The fitted round-trip test (`tests/testthat/test-fit-roundtrip.R`)

A new testthat file reproduces the **5 weight fixtures by *fitting*** through the
binding (the converse of `test-roundtrip.R`, which exercises the *application*
path): `expand_trial_weighted_fitted()` fits each scenario from its raw cohort,
and the result is compared structural-bit-exact + `weight` within the **fitted**
tolerance `FITTED_WEIGHT_REL_TOL = 1e-6` — mirrored from
`crates/tte-expand/tests/weights_fit.rs`, **not** the 1e-12 application constant.
It also checks that `fit_trial_weights()` writes a `(id, period, weight_factor)`
table that drives the apply path to the same fixture, and that an unknown
`pool_censor` surfaces a clear R error (the `ExpandError`→R mapping live).

Per the agent guard (writes under any `tests/` path are blocked), the file was
authored to `bindings/tters/tests-staging/testthat/test-fit-roundtrip.R`; a human
moves it into the suite with
`git mv bindings/tters/tests-staging/testthat/test-fit-roundtrip.R bindings/tters/tests/testthat/test-fit-roundtrip.R`
(exactly how Phases 4 and 6 shipped their tests).

### D. The binding lockfile change (smartcore enters — the converse of Phase 6)

The binding's `tte-expand` path dependency now enables `features = ["weights-fit"]`,
which pulls **`smartcore`** (and `approx` + the `num` family) into the binding's
tree. So `bindings/tters/src/rust/Cargo.lock` **now records smartcore** — the
deliberate converse of Phase 6, which kept this lockfile solver-free. This is the
intended footprint change: the binding can no longer *not* fit weights. The
solver was already justified in Phase 6 (pure-Rust, Apache-2.0, **no native
BLAS/LAPACK** — FP-reproducible across machines); `cargo deny check` on the binding
tree stays clean.

## VERIFY-FIRST findings (empirical, established before building)

| Unknown | Finding |
|---|---|
| **extendr type-mapping** | From the extendr-api **0.9.0** source: R character vectors map to `Strings` (`.iter() → &Rstr`, `Rstr: AsRef<str>`); `bool` / `&str` / `i32` scalars are supported directly. The nested, `#[non_exhaustive]` `WeightSpec` builds cleanly from flat R inputs via the public `::new` / builder constructors. Chosen design: flat scalars + `Strings` covariate vectors + `use_switch`/`use_censor` gate flags at the FFI boundary (matching the existing "R passes flat scalar args" convention), with a `NULL`-driven ergonomic R wrapper deriving the flags. Lowest-risk I/O is parquet-path in/out (the existing pattern) — no Polars-frame marshalling across the FFI. |
| **Build (smartcore × extendr)** | Enabling `weights-fit` on the binding dep compiles the full extendr + polars + smartcore tree under the binding toolchain (extendr-api 0.9.0, R 4.3.3, MSRV 1.95): a clean `cargo build` in ~52 s (debug, warm registry), `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check` clean. smartcore coexists with the extendr cdylib with **no symbol/link issues**. One fix: extendr 0.9 deprecated `Rstr::as_str` → used `.as_ref()`. |
| **Validation target** | The 5 fixtures are reproducible by *fitting* — proven in the core (`cargo test -p tte-expand --features weights-fit --test weights_fit`: 6/6, `weight` within 1e-6, worst 3.4e-7 high_switching PP). Phase 7 proves it survives the **FFI round-trip** through R. The fitting specs mirror the canonical core map exactly (switching: n ~ x2, d ~ x2 + x1; data_censored PP adds IPCW on `censored`; data_censored ITT is IPCW-only with the numerator pooled across `am_1` strata). Switching cohorts read from `fixtures/scenarios/`, data_censored from `fixtures/weights/`. |
| **Supply chain** | `cargo deny check` on the binding tree (repo policy): advisories / bans / licenses / sources **ok**. smartcore (Apache-2.0) + `approx` / `cfg-if` / `num*` / `rand` are all covered by the existing `deny.toml` allow-list — no allow-list edit. The Phase-5 certificate reads the binding lockfile only via `lock_version(&binding_lock, "extendr-api")`, a substring scan anchored on `name = "extendr-api"` then the next `version =` line — adding smartcore to the lockfile **cannot** interpose, so the certificate still parses it. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| `cargo build` (binding, `weights-fit` on) | ✅ smartcore × extendr compile clean |
| `cargo clippy --all-targets --all-features -- -D warnings` (binding) | ✅ clean |
| `cargo fmt --all --check` (binding) | ✅ |
| `cargo deny check` (binding tree, repo policy) | ✅ advisories / bans / licenses / sources ok |
| Core fitted contract (`--test weights_fit`, 5 fixtures + determinism) | ✅ 6/6, `weight` within 1e-6 (worst 3.4e-7) |
| `R CMD INSTALL` (debug) + `library(tters)` exports 8 fns | ✅ |
| `R CMD INSTALL` (release, LTO) | ✅ |
| testthat **fit** round-trip — 5 fixtures *by fitting*, structural bit-exact, `weight` ≤ 1e-6 | ✅ |
| testthat — `fit_trial_weights` table drives the apply path; unknown `pool_censor` → clean R error | ✅ |
| testthat **apply** round-trip (`test-roundtrip.R`, 17 ITT + 17 PP + 5 weighted) — no regression | ✅ |
| Root `make verify` (test + certificate) | ✅ green |
| Binding lockfile records smartcore; root lockfile unchanged; certificate parses binding lockfile | ✅ |
| Core / contract / `fixtures` / `oracle` / `SPEC.md` untouched | ✅ |

## Decisions / deviations recorded

- **The binding lockfile changes — intentionally.** Enabling `weights-fit` pulls
  smartcore into `bindings/tters/src/rust/Cargo.lock`. This is the deliberate
  converse of Phase 6 (which kept it solver-free) and is committed as part of this
  phase. The root `Cargo.lock` is unchanged (the binding is a detached workspace).
- **Parquet-path I/O, not frame marshalling.** The shims write parquet and the R
  wrapper returns `output_path`, exactly like the existing binding — the
  lowest-risk surface that keeps all dtype-exact Polars work in the verified core.
  An in-memory `data.frame` return is deferred.
- **Flat-scalar FFI + NULL-driven ergonomic wrapper.** The `#[extendr]` fns take
  flat scalars + `Strings` + gate flags (the established convention); the R wrapper
  presents `NULL`-droppable `switch_*` / `censor_*` arguments and derives the flags.
  `WeightSpec`'s `#[non_exhaustive]` types are assembled via builders, never struct
  literals.
- **Error mapping reused verbatim.** `ExpandError::WeightFit` surfaces through the
  same `Error::Other(format!("tte-expand: {e}"))` mapping as every other variant —
  no special-casing; the live path is exercised by the unknown-`pool_censor` test.
- **What stays in R.** Robust/sandwich variance and the MSM coefficient estimation
  — unchanged. The binding fits only the IPW nuisance models (the deterministic
  factor table); it computes no standard errors.
- **Tolerances live in the harness.** The testthat mirrors
  `FITTED_WEIGHT_REL_TOL = 1e-6` from the Rust harness (fitting is L-BFGS-to-MLE,
  not bit-for-bit IRLS — ADR-2), distinct from the 1e-12 *application* constant in
  `test-roundtrip.R`. No tolerance is defined in `src/`.

## Deferred to later phases

- **In-memory frame I/O.** R↔Rust still hands data over via parquet paths. A
  zero-copy `data.frame`/Arrow return for `fit_*` is a possible ergonomic follow-up;
  it would re-open the FFI marshalling surface deliberately kept out of v1.
- **Vendored self-test fixtures.** The testthat still resolves the repo-root
  `fixtures/` (via `$TTERS_FIXTURE_DIR` or a walk-up) and skips when absent. A
  vendored `inst/extdata` subset for a fully self-contained installed self-test
  remains deferred (carried over from Phase 4).
- **Additional weight-model shapes.** Only the 5 fixtures' shapes are validated
  through R (intercept + `x1`/`x2`; `pool_cense` none/numerator). As-Treated,
  pooled-both censoring, and `eligible_wts` gating exist in the core but are not
  fixture-validated — out of scope until fixtures exist.
- **Standard errors / robust variance.** Permanently out of scope: R owns
  statistical estimation (the Rust-owns-transform / R-owns-estimation split).
