# Fit IPW weights and expand a cohort data.frame in one call, in memory (wrapper)

Frame-in / frame-out analogue of
[`expand_trial_weighted_fitted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted.md):
takes a raw cohort `data.frame` straight to a weighted, expanded
`data.frame` in one call — fitting the switching and/or IPCW models in
Rust (no pre-computed factor table), expanding under `estimand`, and
accumulating the fitted factor into the cumulative `weight`. The six
structural columns are bit-exact; `weight` matches the Oracle within the
staged ~1e-6 tolerance. Wraps the extendr-generated
[`expand_weighted_fitted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_df.md).
A
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
`id` round-trips exactly.

## Usage

``` r
expand_trial_weighted_fitted_df(
  cohort,
  id_col = "id",
  period_col = "period",
  treatment_col = "treatment",
  eligible_col = "eligible",
  outcome_col = "outcome",
  first_period = 0L,
  last_period = .Machine$integer.max,
  estimand = "PP",
  switch_numerator = NULL,
  switch_denominator = NULL,
  censor_col = NULL,
  censor_numerator = NULL,
  censor_denominator = NULL,
  pool_censor = "none"
)
```

## Arguments

- cohort:

  A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
  person-time rows. Coerced with
  [`as.data.frame()`](https://rdrr.io/r/base/as.data.frame.html).

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` or `"PP"`.

- switch_numerator, switch_denominator:

  Character vectors of covariate column names for the switching
  numerator / denominator models, or `NULL` to omit switching weights.

- censor_col:

  Name of the `{0,1}` censoring-indicator column; the modelled response
  is `1 - censor_col`. `NULL` omits IPCW weights.

- censor_numerator, censor_denominator:

  Character vectors of covariate column names for the IPCW numerator /
  denominator models.

- pool_censor:

  How the IPCW models are pooled across the previous-treatment strata:
  `"none"`, `"numerator"`, or `"both"`.

## Value

A `data.frame` with the six structural columns plus `weight`.

## Details

Model presence follows the same rule as
[`fit_trial_weights_df()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights_df.md).

## See also

[`expand_trial_weighted_fitted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted.md)
for the Parquet-path equivalent;
[`fit_trial_weights_df()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights_df.md)
to return only the factor table.

## Examples

``` r
if (requireNamespace("arrow", quietly = TRUE)) {
  cohort <- as.data.frame(arrow::read_parquet(system.file(
    "extdata", "weights", "input_data_censored.parquet", package = "tters")))
  weighted <- expand_trial_weighted_fitted_df(cohort, estimand = "PP",
    switch_numerator = "x2", switch_denominator = c("x2", "x1"))
}
```
