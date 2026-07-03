# tters — R binding for `tte-expand` (extendr)

[![r-universe](https://oldschoolcool2.r-universe.dev/badges/tters)](https://oldschoolcool2.r-universe.dev/tters)

`tters` is the R companion package that exposes the `tte-expand` Rust
core crate to R via [extendr](https://extendr.github.io/). It
reproduces, bit-for-bit, the sequential trial-emulation data expansion
of the
[`TrialEmulation`](https://cran.r-project.org/package=TrialEmulation) R
package, with a Parquet path, an in-memory `data.frame` path, and a
`TrialEmulation` companion backend (see
[`../../ROADMAP.md`](https://oldschoolcool2.github.io/ROADMAP.md)).

## Installation

``` r

install.packages("tters",
  repos = c("https://oldschoolcool2.r-universe.dev", "https://cloud.r-project.org"))
```

No Rust toolchain is required to install from r-universe (binary builds
are provided). Building from source needs Cargo / `rustc >= 1.95`.

## How it fits together

    bindings/tters/
    ├── DESCRIPTION, NAMESPACE          # R package metadata
    ├── R/                              # R wrappers (extendr-wrappers.R is generated)
    ├── configure[.win], cleanup[.win]  # R CMD INSTALL build hooks
    ├── tools/                          # config.R / msrv.R (toolchain checks)
    └── src/
        ├── Makevars.in, Makevars.win.in, entrypoint.c, tters-win.def
        └── rust/                       # a DETACHED Cargo workspace
            ├── Cargo.toml              # extendr-api + path dep on ../../../../crates/tte-expand
            └── src/lib.rs              # #[extendr] FFI shims

The Rust crate under `src/rust/` declares an **empty `[workspace]`
table**, making it its own workspace root with its own `Cargo.lock`.
This is required so `R CMD INSTALL` builds a self-contained,
reproducible (CRAN-vendorable) crate. It is excluded from the repo-root
workspace (`exclude = ["bindings/tters"]` in the root `Cargo.toml`) and
is therefore **not** linted by the main CI `--workspace` clippy job —
extendr’s macro-expanded FFI would otherwise trip pedantic lints.

## Companion backend for `TrialEmulation`

`tters` plugs into upstream `TrialEmulation`’s `te_datastore` extension
API so a `trial_sequence()` pipeline runs the expensive **expansion in
Rust** while estimation (sampling + the marginal structural model) stays
in R, consuming the Rust output **bit-identically** to the default path.
This is `Suggests`-level and opt-in: `TrialEmulation` is never an
`Imports`, and the backend is a no-op for anyone who does not call it.
Validated against `TrialEmulation` **v0.0.4.11**.

Set up the trial sequence exactly as for
`TrialEmulation::expand_trials()`, then call
[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md)
instead — everything downstream is unchanged:

``` r

library(TrialEmulation)
library(tters)
data("data_censored")

trial <- trial_sequence("ITT") |>
  set_data(data = data_censored) |>
  set_censor_weight_model(
    censor_event = "censored", numerator = ~x2, denominator = ~ x2 + x1,
    pool_models = "numerator",
    model_fitter = stats_glm_logit(save_path = tempfile())
  ) |>
  calculate_weights() |>                       # weight MODELS fit in R
  set_outcome_model(adjustment_terms = ~x2) |>
  set_expansion_options(output = save_to_tters(), chunk_size = 0)

trial <- expand_trials_tters(trial)            # the EXPANSION runs in Rust
trial <- load_expanded_data(trial, seed = 1234, p_control = 0.5)
trial <- fit_msm(trial)                        # estimation stays in R
```

The produced frame is byte-equivalent to
`TrialEmulation::expand_trials()` (structural columns bit-exact,
`weight` to within machine precision), so `load_expanded_data()`,
`sample_controls()`, and `fit_msm()` behave identically. The split is
deliberate: **Rust owns the deterministic data transformation; R owns
statistical estimation.** The per-period weight `wt` computed by
`calculate_weights()` is read verbatim — Rust performs only its
deterministic accumulation. If `TrialEmulation` (or the Rust build) is
unavailable, or for the not-yet-supported AT estimand,
[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md)
falls back to `TrialEmulation::expand_trials()` with a message
(`fallback = FALSE` to force the Rust path).

## Regenerating with rextendr

These files mirror what
[`rextendr`](https://extendr.github.io/rextendr/) produces. To
(re)scaffold or refresh after changing the Rust signatures, from this
directory in R:

``` r

# install.packages("rextendr")
rextendr::document()        # regenerates R/extendr-wrappers.R + NAMESPACE
# or, to (re)create the binding scaffold from scratch:
# rextendr::use_extendr()
```

> **Verify pinned versions before relying on this scaffold.**
> `extendr-api` is pinned to `0.9` and `Config/rextendr/version` to
> `0.5.0` from research that could not be confirmed against crates.io
> offline. Run `rextendr::document()` and `cargo update` (with network)
> to reconcile to the installed versions.

## Caveats for distribution

- **Path dependency vs. CRAN.** `cargo vendor` does not vendor path
  dependencies, and `R CMD build` only tarballs files under
  `bindings/tters/`. Before any CRAN submission, either publish
  `tte-expand` to a registry and switch to a version dependency, or add
  a build step that copies `crates/tte-expand/` into the package. The
  vendored Polars tree is also large; r-universe / source install is the
  realistic channel.
- **Two lockfiles.** Commit both the root `Cargo.lock` and
  `src/rust/Cargo.lock` (Dependabot tracks the latter at
  `/bindings/tters/src/rust`).
