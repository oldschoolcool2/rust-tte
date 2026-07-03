# Fit the IPW weights for an in-memory cohort, expand, apply, and return the weighted trial frame as a `data.frame` — a raw cohort `data.frame` straight to a weighted, expanded `data.frame` in one call (no pre-computed factor table, no intermediate Parquet). The frame-in/frame-out analogue of `expand_weighted_fitted_parquet()`. A 64-bit integer `id` (`bit64::integer64`) round-trips exactly.

Fit the IPW weights for an in-memory cohort, expand, apply, and return
the weighted trial frame as a `data.frame` — a raw cohort `data.frame`
straight to a weighted, expanded `data.frame` in one call (no
pre-computed factor table, no intermediate Parquet). The
frame-in/frame-out analogue of
[`expand_weighted_fitted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_parquet.md).
A 64-bit integer `id`
([`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html))
round-trips exactly.

## Usage

``` r
expand_weighted_fitted_df(
  cohort,
  id_col,
  period_col,
  treatment_col,
  eligible_col,
  outcome_col,
  first_period,
  last_period,
  estimand,
  use_switch,
  switch_numerator,
  switch_denominator,
  use_censor,
  censor_col,
  censor_numerator,
  censor_denominator,
  pool_censor
)
```

## Arguments

- cohort:

  An R `data.frame` of long person-time rows.

- id_col, period_col, treatment_col:

  Column names in `cohort`.

- eligible_col, outcome_col:

  Eligibility / outcome column names.

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` or `"PP"`. Case-insensitive.

- use_switch:

  Whether to fit per-protocol switching-weight models.

- switch_numerator, switch_denominator:

  Covariate columns for the switching numerator/denominator models
  (ignored when `use_switch` is `FALSE`).

- use_censor:

  Whether to fit inverse-probability-of-censoring (IPCW) models.

- censor_col:

  Name of the `{0,1}` censoring-indicator column; the response is
  `1 - censor_col` (ignored when `use_censor` is `FALSE`).

- censor_numerator, censor_denominator:

  Covariate columns for the IPCW numerator/denominator models (ignored
  when `use_censor` is `FALSE`).

- pool_censor:

  How the IPCW models are pooled across the previous-treatment strata:
  `"none"`, `"numerator"`, or `"both"`. Case-insensitive.

## Value

A `data.frame` with the six structural columns plus `weight`. Errors in
the core engine (including weight-fit failures) surface as R errors.
