# Fit inverse-probability weights for a target-trial cohort (ergonomic wrapper)

User-facing wrapper around the extendr-generated
[`fit_weights_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_parquet.md)
that *fits* the IPW switching and/or IPCW censoring models in Rust and
writes the per-`(id, period)` factor table (`id, period, weight_factor`)
— the table
[`expand_trial_weighted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted.md)
consumes. Unlike that pre-computed-factor path, here the weight *models*
are fitted in Rust (the `weights-fit` surface): a faithful port of
`TrialEmulation`'s design preparation plus a deterministic
binomial-logit solver. Robust/sandwich variance and the marginal
structural model stay in R.

## Usage

``` r
fit_trial_weights(
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

  Path to write the `(id, period, weight_factor)` Parquet.

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

A switching model is fitted when either `switch_numerator` or
`switch_denominator` is non-`NULL`; an IPCW censoring model is fitted
when `censor_col` is non-`NULL`. Covariates are character vectors of
column names; `character(0)` (or `NULL`) yields an intercept-only model.

## See also

[`expand_trial_weighted_fitted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted.md)
to fit and expand in a single call.

## Examples

``` r
# Per-protocol switching weights (numerator ~ x2, denominator ~ x2 + x1):
input <- system.file("extdata", "weights", "input_data_censored.parquet",
                     package = "tters")
fit_trial_weights(input, tempfile(fileext = ".parquet"), estimand = "PP",
                  switch_numerator = "x2", switch_denominator = c("x2", "x1"))
```
