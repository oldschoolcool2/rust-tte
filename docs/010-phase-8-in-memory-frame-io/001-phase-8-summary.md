# Phase 8 — In-Memory Frame I/O via `tters`: Completion Summary

**Status: ✅ Implemented. The `tters` R binding now goes *frame-in → frame-out* —
an R user passes a cohort as an in-memory `data.frame` and gets the expanded /
weighted / fitted result back as a `data.frame`, with NO intermediate Parquet on
the critical path. The whole fixture battery reproduces *through the in-memory FFI
round-trip* — the six structural columns bit-exact (and dtype-exact: R
`integer`↔`Int32`, `double`↔`Float64`), `weight` within the staged tolerances
(application ~1e-12, fitted ~1e-6, ADR-2) — and the verified core, contract suite,
and Phases 1–7 behaviour are byte-identical.**
Date: 2026-06-30.

Phase 8 is the first deferred item from Phase 7: *"In-memory frame I/O — a
`data.frame`/Arrow return that re-opens the FFI marshalling surface deliberately
kept out of v1."* Until now every `tters` entry point was parquet-path-in /
parquet-path-out. The binding now surfaces four **`*_df`** entry points that take an
in-memory cohort and return an in-memory `data.frame`, composing the
already-public `expand` / `apply_weights` / `fit_weights` core LazyFrame API in
memory. Robust/sandwich variance and the MSM coefficient model **stay in R** (out
of scope, as in v1).

## What was implemented

Edits are confined to `bindings/tters/**` (the binding shim, R wrappers,
NAMESPACE/man, the staged testthat, `DESCRIPTION`) plus this Phase-8 doc folder.
The verified core crate (`crates/tte-expand/**`), the contract suite, `fixtures/`,
`oracle/`, and `SPEC.md` are **untouched** — no core change was needed (the
in-memory composition is already public and returns LazyFrames).

### A. The marshalling module (`bindings/tters/src/rust/src/frame.rs`)

A new, self-contained module that moves columns across the FFI boundary
dtype-exactly, with NO new R-side dependency:

- `lazyframe_from_list(&List) -> Result<LazyFrame>` — an R `data.frame` (a `list`
  of equal-length columns) → a Polars `LazyFrame`. Per column: R `integer` →
  `Int32`, `double` → `Float64`, `logical` → `Boolean`, `character` → `String`,
  with R `NA` mapped to a Polars null (`from_iter_options`). It rejects loudly
  (rather than silently corrupting) an `integer64` (`bit64`) column — stored as a
  REALSXP of i64 bits, not IEEE doubles — and a `factor` (an INTSXP of level
  codes); both would be a silent correctness bug.
- `dataframe_to_robj(&DataFrame) -> Result<Robj>` — a Polars `DataFrame` → an R
  `data.frame`: each column marshalled per the inverse table (`Int32` → `integer`,
  `Float64` → `double`, …; narrower ints / `Float32` widen losslessly; Polars null
  → R `NA` via `collect_robj`), assembled as a `list` with `class = "data.frame"`
  and the **compact** automatic `row.names = c(NA, -nrow)` (so `nrow()` works
  without materialising an `1:n` index — important at scale). `Int64`/`UInt32`/
  `UInt64` results (which never occur in the battery) raise a clear error rather
  than silently widening.

The mapping reproduces, bit-for-bit, what `scan_parquet` / `ParquetWriter` already
do (Arrow `int32`↔R `integer`, `double`↔R `double`), so the in-memory path matches
the same fixtures at the same tolerances as the parquet path.

### B. The four `#[extendr]` shims (`bindings/tters/src/rust/src/lib.rs`)

Thin FFI shims that marshal the cohort frame in, **compose the public core
LazyFrame API in memory**, and marshal the collected `DataFrame` back out — every
`tte_expand::ExpandError` mapped via `Error::Other(format!("tte-expand: {e}"))`,
the exact mapping the parquet shims use (so `ExpandError::WeightFit` and the
unknown-estimand / unknown-`pool_censor` errors surface identically):

- `expand_df(cohort, …)` → `expand` → collect → frame.
- `expand_weighted_df(cohort, factors, …)` → `expand` → `apply_weights` (both
  frames marshalled in) → collect → frame.
- `fit_weights_df(cohort, …, <flat WeightSpec>)` → `fit_weights` → collect → the
  `(id, period, weight_factor)` factor frame.
- `expand_weighted_fitted_df(cohort, …, <flat WeightSpec>)` → `fit_weights` →
  `expand` → `apply_weights` (mirroring `expand_weighted_fitted_parquet`) →
  collect → frame.

They reuse the existing `build_options` / `build_weight_spec` helpers (flat-scalar
+ `Strings` FFI convention; `WeightSpec`'s `#[non_exhaustive]` types assembled via
builders). All four are registered in `extendr_module!`.

### C. The R surface (`bindings/tters/R/tters-package.R`, NAMESPACE, man/)

Four ergonomic, user-facing `*_df` wrappers, matching the path family's style
(defaults, `stopifnot`, integer coercion) but **taking a cohort `data.frame` and
returning a `data.frame`**:

- `expand_trial_df(cohort, …)` — structural ITT/PP.
- `expand_trial_weighted_df(cohort, factors, …)` — apply a pre-computed factor
  frame.
- `fit_trial_weights_df(cohort, …)` — return the factor frame.
- `expand_trial_weighted_fitted_df(cohort, …)` — raw cohort → weighted frame in
  one call.

Each coerces the input with `as.data.frame()` so a tibble / `data.table` / Arrow
`Table` is accepted; the weight-bearing pair reuse the Phase-7 `.tters_weight_spec()`
`NULL`-driven resolver. A **dedicated `*_df` family** was chosen over a `return=`
flag on the existing wrappers (see *Decisions*). The low-level `extendr-wrappers.R`
was regenerated with the `document` bin, and `NAMESPACE` + `man/*.Rd` with
`roxygen2::roxygenise(load_code = roxygen2::load_source)` — no `rextendr::document()`.
The binding now exports **16** functions (8 new: the four `*_df` shims + four
ergonomic `*_df` wrappers).

### D. The in-memory round-trip test (`tests/testthat/test-df-roundtrip.R`)

A new testthat file reproduces the battery **through the in-memory path** (the
converse of the parquet `test-roundtrip.R` / `test-fit-roundtrip.R`): a fixture is
read into an R `data.frame` with `arrow::read_parquet()`, run through the `*_df`
wrapper, and the returned frame compared to the committed `expected_*`. It covers
the full **17 ITT + 17 PP** structural battery (edge `E01`–`E09` + 8 scenarios),
the **5 weight fixtures by *applying*** (within 1e-12) **and by *fitting*** (within
1e-6), the `fit_trial_weights_df` → `expand_trial_weighted_df` in-memory chain, an
explicit **dtype-exactness** assertion (`integer` id passthrough vs the E02
`double` id), and the two error-mapping cases — **49 tests, all green**. Tolerances
are mirrored from the Rust harness (`WEIGHT_REL_TOL = 1e-12`,
`FITTED_WEIGHT_REL_TOL = 1e-6`), never invented.

Per the agent guard (writes under any `tests/` path are blocked), the file was
authored to `bindings/tters/tests-staging/testthat/test-df-roundtrip.R`; a human
moves it into the suite with
`git mv bindings/tters/tests-staging/testthat/test-df-roundtrip.R bindings/tters/tests/testthat/test-df-roundtrip.R`
(exactly how Phases 4/6/7 shipped their tests). It was **verified green against the
installed package** before staging.

### E. Dependencies (`DESCRIPTION`, the binding lockfile)

- `polars` is now a **direct** dependency of the binding Rust crate (so the shim
  can name `DataFrame`/`Series`/`DataType`). Polars 0.54.4 was *already* in the
  binding tree (transitively via `tte-expand`) and pinned in the lockfile; this
  only declares it directly (`default-features = false, features = ["lazy"]`,
  mirroring the root workspace). The **only** lockfile change is one added
  dependency *edge* under the `tters` package entry — **zero new crate versions**;
  `cargo deny` stays clean.
- `arrow` is now declared in `DESCRIPTION: Suggests` (the testthat reads fixtures
  with it; the gap predates Phase 8). The in-memory path itself needs **no R
  runtime package** — it is pure base-R `data.frame` ↔ extendr.

## VERIFY-FIRST findings (empirical, established before building)

| Unknown | Finding |
|---|---|
| **Marshalling mechanism** | The fixtures' structural columns are **only ever `Int32` or `Float64`** (`id`/`treatment`/`assigned_treatment` pass through as int32 *or* double; `trial_period`/`followup_time` `Int32`; `outcome`/`weight` `Float64`; **no `Int64`**). Base R represents both natively, so **column-wise typed vectors** (Option A) are *fully dtype-exact* with **no extra deps** and full int-vs-double control — chosen over the Arrow C interface (more moving parts, no benefit here) and the tempfile-Parquet fallback (not in-memory). Proven empirically: returned `typeof` is `integer` for int-id cohorts and `double` for the E02 double-id cohort (id passthrough), `integer` for `trial_period`/`followup_time`, `double` for `outcome`/`weight`. R→Polars (`integer`→`Int32`, `double`→`Float64`) is identical to `scan_parquet`. |
| **Build (polars × extendr)** | Declaring `polars` directly + `frame.rs` compiles under the binding toolchain (extendr-api 0.9.0, R 4.3.3, MSRV 1.95): clean `cargo build`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`. Key API facts (extendr 0.9.0 / polars 0.54.4): a `data.frame` arrives as `List`; `List::iter()` yields `(&str, Robj)`; `Robj::{as_integer_slice,as_real_slice,as_logical_slice,as_str_iter}` + `Rtype` for input; `ChunkedArray::iter()` (NOT `into_iter`) + `RobjItertools::collect_robj` (NA-aware via `ToVectorValue for Option<T>`) for output; `DataFrame::columns()` returns `&[Column]`; compact `row.names = c(NA, -n)` via `i32::MIN`. |
| **Validation target** | All 5 weight fixtures reproduce through the in-memory round-trip — by *applying* (worst `weight` rel ≤ 8.4e-16 ≤ 1e-12) and by *fitting* (worst 3.4e-7 ≤ 1e-6, matching the core's observed worst, `high_switching` PP) — plus the full 17 ITT + 17 PP structural battery bit-exact, all dtype-exact. 49 testthat checks, 0 fail / 0 skip. |
| **R-side dependencies** | The in-memory path needs **no R runtime package** (pure base-R `data.frame` ↔ extendr). Only the *tests* use `arrow` (to read fixtures); that previously-undeclared dependency is now in `DESCRIPTION: Suggests`. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| `cargo build` (binding, polars direct dep) | ✅ polars × extendr × smartcore compile clean |
| `cargo clippy --all-targets --all-features -- -D warnings` (binding) | ✅ clean |
| `cargo fmt --all --check` (binding) | ✅ |
| `cargo test` (binding Rust crate) | ✅ (shim has no unit tests; 0/0) |
| `cargo deny check` (binding tree, repo policy) | ✅ advisories / bans / licenses / sources ok |
| `R CMD INSTALL` (debug) + `library(tters)` exports 16 fns | ✅ |
| `R CMD INSTALL` (release, LTO) | ✅ |
| testthat **in-memory** round-trip (`test-df-roundtrip.R`) | ✅ **49 tests, 0 fail / 0 error / 0 skip** |
| — 17 ITT + 17 PP structural, in memory, bit-exact + dtype-exact | ✅ |
| — 5 weight fixtures by *applying* (`expand_trial_weighted_df`), `weight` ≤ 1e-12 | ✅ |
| — 5 weight fixtures by *fitting* (`expand_trial_weighted_fitted_df`), `weight` ≤ 1e-6 | ✅ |
| — `fit_trial_weights_df` table drives the in-memory apply path | ✅ |
| — unknown estimand / unknown `pool_censor` → clean R error | ✅ |
| Root `make verify` (test + certificate) | ✅ 47/47 fixtures match manifest; spot-checks pass |
| Binding lockfile: only an added `polars` edge (no new versions); root lockfile unchanged | ✅ |
| Core / contract / `fixtures` / `oracle` / `SPEC.md` untouched | ✅ |

## Decisions / deviations recorded

- **Dedicated `*_df` family, not a `return=` flag.** The path functions keep
  clean path-in/path-out signatures; the in-memory family is discoverable and
  self-documenting (`expand_trial_df`, `expand_trial_weighted_df`,
  `fit_trial_weights_df`, `expand_trial_weighted_fitted_df`), and avoids
  overloading `output_path`/`factors_path` semantics on a flag.
- **Column-wise typed vectors, not Arrow.** Justified by the empirical finding
  that the structural battery is `Int32`/`Float64` only — base R is dtype-exact for
  both, with zero new dependencies. An Arrow C-interface bridge (needed only for
  exact `Int64`/`integer64`) is the documented follow-up.
- **`polars` declared directly in the binding** (one added lockfile edge, no new
  versions) rather than re-exporting polars types from the verified core. Editing
  `crates/tte-expand/src/` — even a re-export — was avoided; `bindings/**` is the
  allowed surface and a justified binding-lockfile change is sanctioned.
- **No core change.** `expand` / `apply_weights` / `fit_weights` are already public
  and return LazyFrames, so the shim composes them in memory directly.
- **Loud failure over silent corruption.** `integer64` / `factor` inputs and
  `Int64`/`UInt32`/`UInt64` outputs (none in the battery) raise a clear R error
  rather than silently mis-reading bits or widening — the determinism-rule ethos.
- **What stays in R / tolerances.** Robust/sandwich variance and the MSM are
  unchanged; the binding only marshals + fits the deterministic factor table.
  Tolerances are mirrored from the Rust harness, never defined in `src/`.

## Deferred to later phases

- **Exact `Int64` / `integer64` columns.** Out of the validated battery; an Arrow
  C Data Interface (or `bit64`) bridge would carry 64-bit ids exactly — currently
  rejected loudly.
- **Zero-copy Arrow stream.** The column-wise path copies each column once
  (allocations bounded by R memory anyway). A zero-copy Arrow export is a possible
  performance follow-up if a frame-I/O benchmark shows it matters.
- **Vendored self-test fixtures** (`inst/extdata`) and the standing **standard
  errors / robust variance stays in R** boundary — both carried over unchanged
  from Phases 4–7.
