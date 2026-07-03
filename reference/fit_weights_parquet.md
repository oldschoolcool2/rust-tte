# Fit the inverse-probability **weight factor** for a Parquet cohort in Rust and write the per-`(id, period)` factor table (`id, period, weight_factor`).

A thin FFI shim over `tte_expand::fit_weights_parquet` (the
`weights-fit` surface). Unlike
[`expand_weighted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_parquet.md),
which *applies* a pre-computed factor table, this *fits* the IPW models
in Rust: it ports `TrialEmulation`'s `data_manipulation` + `censor_func`
design preparation and binds a deterministic binomial-logit solver for
the switching and/or IPCW censoring models, then forms
`wt = wt_switch * wtC`. The structural design is exact; the fitted
factors reproduce R `glm` within the staged ~1e-6 tolerance (ADR-2), not
bit-for-bit. Robust/sandwich variance and the marginal structural model
stay in R.

## Usage

``` r
fit_weights_parquet(
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

  Path where the `(id, period, weight_factor)` table is written.

- id_col, period_col, treatment_col:

  Column names in the input.

- eligible_col, outcome_col:

  Eligibility / outcome column names.

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` or `"PP"`; per-protocol runs the artificial-censoring state
  machine and (with switching covariates) the switch models.
  Case-insensitive.

- use_switch:

  Whether to fit per-protocol switching-weight models.

- switch_numerator, switch_denominator:

  Covariate columns for the switching numerator (stabiliser) and
  denominator models (ignored when `use_switch` is `FALSE`).

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

`NULL`, invisibly; the factor table is written to `output_path`. Errors
in the core engine (including weight-fit failures) surface as R errors.

## Examples

``` r
if (FALSE) { # \dontrun{
fit_weights_parquet(
  "cohort.parquet", "factors.parquet",
  "id", "period", "treatment", "eligible", "outcome",
  0L, .Machine$integer.max, "PP",
  TRUE, c("x2"), c("x2", "x1"),
  FALSE, "", character(0), character(0), "none"
)
} # }
```
