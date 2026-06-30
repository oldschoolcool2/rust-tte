# Phase 9 — Exact Wide-Integer Frame I/O via `tters`: Completion Summary & Sign-off

**Status: ✅ Implemented. The Phase-8 in-memory (`data.frame`-in / `data.frame`-out)
path now round-trips 64-bit integers *exactly*. An R `bit64::integer64` column
(e.g. a real cohort's 64-bit `id`) marshals to a Polars `Int64` and back with NO
precision loss — including values above `2^53` that a naive double-cast would
silently corrupt — via a pure-safe bit reinterpret (`f64::to_ne_bytes` /
`i64::from_ne_bytes`). NO `unsafe`, NO Arrow C Data Interface. The immutable
fixture battery (structural columns are `Int32`/`Float64` only) reproduces
byte-for-byte through the round-trip exactly as before, and a new synthesized
`integer64`-id cohort round-trips with the `id` exact (value AND `integer64`
storage class) and `weight` within the staged fitted tolerance.**
Date: 2026-06-30.

Phase 9 is the first deferred item from Phase 8: *"Exact `Int64` / `integer64`
columns — currently rejected loudly; an Arrow C Data Interface (or `bit64`) bridge
would carry 64-bit ids exactly."* Phase 8 deliberately **rejected** an R
`integer64` input and a Polars `Int64`/`UInt32`/`UInt64` output with a clear error
(to avoid silently bit-reinterpreting or widening). This phase replaces those two
guards with exact handling. The work is confined to `bindings/tters/**` (the
marshalling internals of `frame.rs`, plus doc text and one new test) — **no core
change, no surface change, no new Rust dependency.**

## What was implemented

Edits are confined to `bindings/tters/**` (the `frame.rs` marshaller, the `*_df`
shim/wrapper doc text, the regenerated `extendr-wrappers.R` / `NAMESPACE` / `man/`,
`DESCRIPTION`, and a new staged testthat) plus this Phase-9 doc folder. The
verified core crate (`crates/tte-expand/**`), the contract suite, `fixtures/`,
`oracle/`, and `SPEC.md` are **untouched** — the `*_df` shims already compose the
public `expand` / `apply_weights` / `fit_weights` LazyFrame API in memory, and
`Int64` support is already in the linked Polars.

### A. The marshalling arms (`bindings/tters/src/rust/src/frame.rs`)

The only behavioural change. Two free helpers do the bit reinterpret with pure-safe
std (the inverse of one another), and the `factor` guard and the
`UInt64 > i64::MAX` guard are **kept**:

- `f64_bits_to_i64(f64) -> i64` / `i64_to_f64_bits(i64) -> f64` — a bitcast
  (`{i64,f64}::{from_ne_bytes,to_ne_bytes}`), NOT a numeric cast, so every `i64`
  survives (a numeric cast would collapse `2^53 + 1` onto `2^53`). Same-process
  FFI ⇒ native-endian is correct on both halves.
- **Input** (`series_from_column`, `Rtype::Doubles` arm): an R column classed
  `"integer64"` is read with `as_real_slice()` and each stored `f64`'s bits are
  reinterpreted to an `i64` → a Polars `Int64Chunked`. `bit64`'s `NA` sentinel
  (`i64::MIN`'s bit pattern) maps to a Polars null. A plain `double` (no
  `integer64` class) still becomes `Float64` exactly as before.
- **Output** (`column_to_robj`): a Polars `Int64` (and `UInt32`; `UInt64` that fits
  `i64`) is reinterpreted element-wise to the `f64` carrying its bits, collected
  into a `REALSXP`, and tagged `class = "integer64"`. A Polars null becomes
  `bit64`'s `NA` (`i64::MIN`). A `UInt64` value beyond `i64::MAX` still errors
  loudly (signed `integer64` cannot hold it). **Trap avoided:** the values are
  collected as `f64` (the reinterpreted bits) — extendr's `ToVectorValue for i64`
  would instead apply a *lossy* numeric `as f64` cast, corrupting values above
  `2^53`; collecting `f64` writes the bits verbatim.
- The `Int32` / `Float64` / `Boolean` / `String` paths, the narrower-int / `Float32`
  widening, the `factor` rejection, and the compact `row.names` are **unchanged**.

The new dtype contract (table also in the module doc):

| direction | R | Polars |
|---|---|---|
| in  | `integer` | `Int32` · `double` | `Float64` · **`integer64` (bit64)** | **`Int64`** · `logical` | `Boolean` · `character` | `String` |
| out | `Int32` | `integer` · `Float64` | `double` · **`Int64`/`UInt32`/`UInt64`** | **`integer64` (bit64)** · `Boolean` | `logical` · `String` | `character` |

### B. The `#[extendr]` surface (`bindings/tters/src/rust/src/lib.rs`) — UNCHANGED

64-bit handling is **transparent in the marshaller**, so all four `*_df` shim
signatures and `extendr_module!` registration are byte-identical. Only the `///`
doc comments gained a sentence noting `integer64` round-trips; the regenerated
`extendr-wrappers.R` diff is **docs-only** (every `.Call(...)` line unchanged).

### C. The R surface (`R/tters-package.R`, `NAMESPACE`, `man/`)

No signature change. The four ergonomic `*_df` wrappers' roxygen gained the dtype
contract and the `integer64` note; `as.data.frame()` (already in each wrapper)
**preserves the `integer64` class** (verified), so a `bit64` cohort flows through
untouched. `extendr-wrappers.R` was regenerated with the `document` bin, and
`NAMESPACE` + `man/` with `roxygen2::roxygenise(load_code = roxygen2::load_source)`
(not `rextendr::document()`). `NAMESPACE` is **unchanged** — no new export, and
`bit64` is **not** imported into the namespace (so it is never attached at runtime
and cannot mask base functions).

### D. The synthesized 64-bit-id round-trip test (`test-df-int64-roundtrip.R`)

A new testthat file (a **synthesized** test input — no fixture added). It uses a
"shifted twin": a base integer cohort run through the `*_df` path both as-is and
with its `id` lifted to `integer64` and shifted by `2^53` (so every id exceeds
`2^53` and is no longer double-representable). 6 `test_that` blocks / **56 checks**:

- `expand_trial_df` ITT **and** PP: the `integer64` run returns an
  `integer64`-classed `id`; the plain run stays `integer`; all non-id structural
  columns match the twin; and `id == plain_id + 2^53` **exactly** in `bit64`
  (a naive cast would return `2^53` for `2^53 + 1` and fail this line).
- A distinctness check: two adjacent ids past `2^53` (which a naive cast collapses
  to one) stay separate.
- `integer64` `id` **+** `period` **+** `treatment` in: confirms `followup_time`
  (inherits `period`) and `treatment`/`assigned_treatment` come back as
  `integer64`, while `trial_period` is `integer` (the core always casts it to
  `Int32`) and `outcome` is `double`.
- `expand_trial_weighted_fitted_df` (fitted path) and `fit_trial_weights_df` on the
  `high_switching` fixture with an `integer64` id: `id` round-trips exactly and
  `weight` matches the plain twin within the staged fitted tolerance (1e-6) — the
  fit is id-shift-invariant.

Per the agent guard (writes under any `tests/` path are blocked), the file was
authored to `bindings/tters/tests-staging/testthat/test-df-int64-roundtrip.R` and
**verified green against the installed package**; a human moves it into the suite
with `git mv bindings/tters/tests-staging/testthat/test-df-int64-roundtrip.R
bindings/tters/tests/testthat/test-df-int64-roundtrip.R` (the Phase-4/6/7/8
pattern).

### E. Dependencies (`DESCRIPTION`, lockfiles)

- **No new Rust dependency.** The bit reinterpret is std; `Int64`/`UInt32`/`UInt64`
  support is already in the linked Polars 0.54.4. Both `Cargo.lock`s are
  **unchanged** (root and binding).
- **`bit64` declared in `DESCRIPTION: Imports`.** It is a *hard `Imports`
  dependency of `arrow`* (confirmed), so it is already transitively present; and a
  user with 64-bit ids needs it at runtime to use the returned `integer64` column
  meaningfully — so `Imports` is the honest declaration. It is **not** imported
  into the package `NAMESPACE` (the Rust side constructs the classed `REALSXP`
  itself), so it is never attached and masks nothing.

## VERIFY-FIRST findings (empirical, established before building)

| Unknown | Finding |
|---|---|
| **`integer64` representation** | `typeof = "double"`, `class = "integer64"`. Each element's storage double's 8 little-endian bytes **are** the `i64` bit pattern — verified against `bit64` for `1`, `2`, `2^53`, **`2^53 + 1`** (not f64-representable), and `i64::MAX`. `NA` = bytes `…80` = `0x8000_0000_0000_0000` = **`i64::MIN`**. `as.data.frame()` (the wrappers' coercion) **preserves the class**. extendr-api 0.9.0 surfaces it as `Rtype::Doubles` + `as_real_slice()`; `i64::from_ne_bytes(f64::to_ne_bytes(x))` recovers the value bit-exactly; same-process ⇒ native-endian is correct. |
| **Build (internal to `frame.rs`)** | `Int64Chunked`/`UInt32Chunked`/`UInt64Chunked` exist in polars-core 0.54.4; `set_class`, `as_real_slice`, `RobjItertools::collect_robj` exist in extendr 0.9.0. Compiles + `clippy --all-targets --all-features -D warnings` clean + `fmt` clean under the binding toolchain (extendr 0.9.0 / R 4.3.3 / MSRV 1.95). No new Rust dep. **Trap found & avoided:** extendr's `ToVectorValue for i64` does a *lossy* numeric `as f64` cast; the write path for plain `f64` (`*ptr = *self as f64`, identity) preserves bits — so the output collects `f64`-reinterpreted values, never `i64`. |
| **Validation target** | The existing battery is `Int32`/`Float64`-only ⇒ **unaffected** (reconfirmed: 384 checks still green). A synthesized `integer64`-id cohort round-trips with the id exact (value via `bit64`, storage class via `inherits(x, "integer64")`) and `weight` within the staged 1e-6 — both expand and fitted, asserted via a shifted twin so a precision bug fails loudly. |
| **R-side dependency** | `bit64` is a **hard `Imports` of `arrow`** (confirmed) ⇒ already transitively present (the test already uses `arrow`). Declared in `DESCRIPTION: Imports` (honest for runtime users with 64-bit ids), but **not** imported into `NAMESPACE` (no masking; the Rust side builds the `integer64` object). |
| **Surface decision** | Keep the four `*_df` signatures **identical** — handling is transparent in the marshaller and `as.data.frame()` preserves the class. The `extendr-wrappers.R` regen is docs-only. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| `cargo fmt --all --check` (binding) | ✅ |
| `cargo clippy --all-targets --all-features -- -D warnings` (binding) | ✅ clean |
| `cargo test` (binding Rust crate) | ✅ **3 new pure-Rust unit tests** (bit-reinterpret round-trip incl. `2^53+1`, `i64::MIN`/`MAX`; distinctness vs a naive cast; NA sentinel) |
| `cargo deny check` (binding tree, repo policy) | ✅ advisories / bans / licenses / sources ok |
| `R CMD INSTALL` (debug) + `library(tters)` | ✅ installs, loads |
| `R CMD INSTALL` (release, LTO) | ✅ |
| testthat **regression** (`test-df-roundtrip.R`, the Phase-8 battery) | ✅ **384 checks, 0 fail / 0 skip** (unchanged) |
| testthat **new** (`test-df-int64-roundtrip.R`) | ✅ **6 blocks / 56 checks, 0 fail / 0 skip** |
| — `integer64` id round-trips exactly (ITT + PP), value + storage class | ✅ |
| — distinct adjacent ids > `2^53` stay distinct (no cast collision) | ✅ |
| — `integer64` id+period+treatment → correct out-dtypes (`trial_period` stays `Int32`) | ✅ |
| — fitted path: `integer64` id exact, `weight` within 1e-6 of the plain twin | ✅ |
| Root `make verify` (test + certificate) | ✅ 47/47 fixtures match manifest; spot-checks pass |
| Both lockfiles unchanged (root + binding); `NAMESPACE` unchanged | ✅ |
| Core / contract / `fixtures` / `oracle` / `SPEC.md` untouched | ✅ |

## Decisions / deviations recorded

- **Safe bit reinterpret, NOT the Arrow C Data Interface.** The whole phase is
  scoped to the bitcast route (`f64::to_ne_bytes` ↔ `i64::from_ne_bytes`)
  precisely to stay `unsafe`-free. The Arrow C Data Interface (zero-copy
  `Int64` ↔ `bit64`) would require `unsafe` FFI to import/export `ArrowArray` /
  `ArrowSchema` C structs across the boundary — out of scope for this phase, with
  its own sign-off. The bit reinterpret copies each column once (bounded by R
  memory anyway), is fully deterministic, and is exact.
- **Collect `f64`, never `i64`, on output.** extendr's `ToVectorValue for i64`
  applies a lossy numeric `as f64` cast (the very corruption this phase prevents);
  the marshaller builds the `REALSXP` from the `f64`-reinterpreted bits so R reads
  back the exact `i64`.
- **`i64::MIN` is `bit64`'s `NA`.** Mapped to/from a Polars null both directions, so
  a missing 64-bit id behaves like any other null (none occur in the battery).
- **Transparent surface; no opt-in flag.** A column simply carries `integer64`; the
  four `*_df` signatures, `extendr_module!`, and `NAMESPACE` are unchanged.
- **`bit64` in `Imports`, not the `NAMESPACE`.** Honest for runtime users; never
  attached, so it cannot mask base functions (`:`, `match`, `order`, …).
- **`trial_period` stays `Int32`.** The core always casts `trial_period` to `Int32`
  (its existing dtype contract, unchanged here), so even with an `integer64`
  `period` the output `trial_period` is base-R `integer` while `followup_time`
  (which inherits `period`) is `integer64`. Documented in the test.

## Deferred to later phases

- **Zero-copy Arrow stream (`Int64` and beyond).** The column-wise bit reinterpret
  copies each column once. A zero-copy Arrow C-Data-Interface export would avoid
  the copy but **requires `unsafe`** FFI (importing/exporting the Arrow C structs)
  — a separate, later phase with its own sign-off, justified only if a frame-I/O
  benchmark shows the copy matters.
- **`UInt64` beyond `i64::MAX`.** Signed `integer64` cannot represent it; the path
  errors loudly rather than wrapping. A future unsigned-aware bridge (or a `double`
  opt-in with documented precision loss) could relax this if a real need appears.
- **`factor` columns.** Still rejected loudly on input (an INTSXP of level codes);
  unchanged from Phase 8.
- **Vendored self-test fixtures** (`inst/extdata`) and the standing
  *standard errors / robust variance / MSM stays in R* boundary — carried over
  unchanged from Phases 4–8.
