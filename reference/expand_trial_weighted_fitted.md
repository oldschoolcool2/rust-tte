# Fit IPW weights and expand a cohort into a weighted trial frame (ergonomic wrapper)

User-facing wrapper around the extendr-generated
[`expand_weighted_fitted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_parquet.md).
It takes a raw person-time cohort straight to a weighted, expanded trial
frame in one call — fitting the switching and/or IPCW models in Rust (no
pre-computed factor table), expanding under `estimand`, and accumulating
the fitted factor into the cumulative `weight`. The six structural
columns are bit-exact; `weight` matches the Oracle within the staged
~1e-6 tolerance. Robust/sandwich variance and the marginal structural
model stay in R.

## Usage

``` r
expand_trial_weighted_fitted(
  input_path,
  output_path,
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

- input_path:

  Path to an existing input Parquet cohort (long person-time).

- output_path:

  Path to write the weighted, expanded Parquet.

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` or `"PP"`. Per-protocol runs the artificial-censoring state
  machine and the switching models; intention-to-treat skips both.

- switch_numerator, switch_denominator:

  Character vectors of covariate column names for the switching
  numerator (stabiliser) / denominator models, or `NULL` (the default)
  to omit switching weights.

- censor_col:

  Name of the `{0,1}` censoring-indicator column; the modelled response
  is `1 - censor_col`. `NULL` (the default) omits IPCW weights.

- censor_numerator, censor_denominator:

  Character vectors of covariate column names for the IPCW numerator /
  denominator models.

- pool_censor:

  How the IPCW models are pooled across the previous-treatment strata:
  `"none"`, `"numerator"`, or `"both"`.

## Value

`output_path`, invisibly.

## Details

Model presence follows the same rule as
[`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md):
a switching model is fitted when either `switch_*` covariate vector is
non-`NULL`; an IPCW model is fitted when `censor_col` is non-`NULL`.

## See also

[`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md)
to write only the `(id, period, weight_factor)` factor table.

## Examples

``` r
# Raw cohort straight to a weighted, expanded frame in one call:
input <- system.file("extdata", "weights", "input_data_censored.parquet",
                     package = "tters")
expand_trial_weighted_fitted(input, tempfile(fileext = ".parquet"),
                             estimand = "PP", switch_numerator = "x2",
                             switch_denominator = c("x2", "x1"))
```
