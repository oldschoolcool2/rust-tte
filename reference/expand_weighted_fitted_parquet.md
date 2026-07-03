# Fit the IPW weights in Rust, expand the cohort, apply the weights, and write the weighted trial frame — a raw cohort to a weighted, expanded frame in one call (no pre-computed factor table).

A thin FFI shim over `tte_expand::expand_weighted_fitted_parquet`: the
fully in-Rust analogue of
[`expand_weighted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_parquet.md).
It fits the switching and/or IPCW models from the spec (as
[`fit_weights_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_parquet.md)
does), expands under `estimand`, joins and accumulates the fitted
factor, and writes the six structural columns plus the
cumulative-product `weight`. Structural columns are bit-exact; `weight`
matches the Oracle within the staged ~1e-6 tolerance (ADR-2).

## Usage

``` r
expand_weighted_fitted_parquet(
  input_path,
  output_path,
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

- input_path:

  Path to the input Parquet cohort (long person-time).

- output_path:

  Path where the weighted, expanded Parquet is written.

- id_col, period_col, treatment_col:

  Column names in the input.

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

`NULL`, invisibly; the weighted expansion is written to `output_path`.
Errors in the core engine (including weight-fit failures) surface as R
errors.

## Examples

``` r
if (FALSE) { # \dontrun{
expand_weighted_fitted_parquet(
  "cohort.parquet", "weighted.parquet",
  "id", "period", "treatment", "eligible", "outcome",
  0L, .Machine$integer.max, "PP",
  TRUE, c("x2"), c("x2", "x1"),
  FALSE, "", character(0), character(0), "none"
)
} # }
```
