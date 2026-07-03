# Fit inverse-probability weights for a cohort data.frame, in memory (wrapper)

Frame-in / frame-out analogue of
[`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md):
fits the IPW switching and/or IPCW censoring models in Rust from an
in-memory cohort `data.frame` and returns the per-`(id, period)` factor
table as a `data.frame` (`id`, `period`, `weight_factor`) — the table
[`expand_trial_weighted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_df.md)
consumes. Wraps the extendr-generated
[`fit_weights_df()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_df.md).
A
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
`id` round-trips exactly (the returned `id` is `integer64`).

## Usage

``` r
fit_trial_weights_df(
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

A `data.frame` with columns `id`, `period`, `weight_factor`.

## Details

Model presence follows the same `NULL`-driven rule as
[`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md):
a switching model is fitted when either `switch_*` covariate vector is
non-`NULL`; an IPCW model is fitted when `censor_col` is non-`NULL`.

## See also

[`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md)
for the Parquet-path equivalent;
[`expand_trial_weighted_fitted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted_df.md)
to fit and expand in a single call.

## Examples

``` r
if (FALSE) { # \dontrun{
factors <- fit_trial_weights_df(
  cohort, estimand = "PP",
  switch_numerator = "x2", switch_denominator = c("x2", "x1")
)
} # }
```
