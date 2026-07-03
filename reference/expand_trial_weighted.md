# Expand a dataset and attach pre-computed inverse-probability weights

User-facing wrapper around the extendr-generated
[`expand_weighted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_parquet.md).
It expands `input_path` under `estimand`, joins the per-`(id, period)`
factor table at `factors_path` (`id, period, weight_factor`), and writes
the six structural columns plus the cumulative-product `weight`. Weight
*values* are produced upstream in R; the engine reproduces only their
deterministic accumulation.

## Usage

``` r
expand_trial_weighted(
  input_path,
  factors_path,
  output_path,
  id_col = "id",
  period_col = "period",
  treatment_col = "treatment",
  eligible_col = "eligible",
  outcome_col = "outcome",
  first_period = 0L,
  last_period = .Machine$integer.max,
  estimand = "PP"
)
```

## Arguments

- input_path:

  Path to an existing input Parquet file.

- factors_path:

  Path to the per-`(id, period)` factor Parquet file.

- output_path:

  Path to write the weighted Parquet file.

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` or `"PP"`; selects the weight model upstream, but the
  application arithmetic is identical for both.

## Value

`output_path`, invisibly.

## Examples

``` r
if (FALSE) { # \dontrun{
expand_trial_weighted(
  "input.parquet", "factors.parquet", "weighted.parquet", estimand = "PP"
)
} # }
```
