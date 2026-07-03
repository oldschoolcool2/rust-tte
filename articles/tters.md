# Using tters with TrialEmulation

`tters` reproduces the sequential trial-emulation *data expansion* of
[`TrialEmulation`](https://cran.r-project.org/package=TrialEmulation)
**bit-for-bit**, in a memory-safe Rust + Polars engine. Rust owns the
deterministic data transformation; R keeps statistical estimation. That
split is exposed two ways: standalone expansion functions, and a drop-in
`te_datastore` backend for a `TrialEmulation` pipeline.

## Install

``` r

install.packages("tters",
  repos = c("https://oldschoolcool2.r-universe.dev", "https://cloud.r-project.org"))
```

No Rust toolchain is needed to install the binary build from r-universe.

## Standalone: expand a cohort in memory

The `*_df` functions take a `data.frame` and return one, with no
intermediate files. Column dtypes are preserved exactly (including
64-bit
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
ids).

``` r

library(tters)

# long person-time: one row per (id, period)
cohort <- data.frame(
  id        = c(1L, 1L, 1L, 2L, 2L),
  period    = c(0L, 1L, 2L, 0L, 1L),
  treatment = c(1L, 1L, 0L, 0L, 1L),
  eligible  = c(1L, 0L, 0L, 1L, 0L),
  outcome   = c(0L, 0L, 1L, 0L, 0L)
)

expanded <- expand_trial_df(cohort, estimand = "ITT")
head(expanded)
```

A Parquet-in / Parquet-out path
([`expand_trial()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial.md)),
pre-computed-weight
([`expand_trial_weighted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted.md))
and in-Rust weight-fitting
([`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md),
[`expand_trial_weighted_fitted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted.md))
variants exist too — see their help pages and the package reference.

## As a TrialEmulation backend

Set up a `trial_sequence()` exactly as you would for
`TrialEmulation::expand_trials()`, then run the expansion in Rust by
pointing the output at
[`save_to_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/save_to_tters.md)
and calling
[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md).
Everything downstream — sampling, `fit_msm()` — is unchanged and
bit-identical.

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
  calculate_weights() |>                    # weight MODELS fit in R
  set_outcome_model(adjustment_terms = ~x2) |>
  set_expansion_options(output = save_to_tters(), chunk_size = 0)

trial <- expand_trials_tters(trial)         # the EXPANSION runs in Rust
trial <- load_expanded_data(trial, seed = 1234, p_control = 0.5)
trial <- fit_msm(trial)                     # estimation stays in R
```

The produced frame is byte-equivalent to
`TrialEmulation::expand_trials()` (structural columns bit-exact,
`weight` to within machine precision), so `load_expanded_data()`,
`sample_controls()`, and `fit_msm()` behave identically. If
`TrialEmulation` or the Rust build is unavailable — or for the not-yet-
supported “as treated” estimand —
[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md)
falls back to `TrialEmulation::expand_trials()` with a message.
