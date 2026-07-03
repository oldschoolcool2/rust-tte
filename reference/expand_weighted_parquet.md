# Expand a person-time Parquet dataset and attach pre-computed inverse-probability weights, writing the weighted frame to `output_path`.

A thin FFI shim over `tte_expand::expand_weighted_parquet`: it expands
the input under `estimand`, joins the per-`(id, period)` factor table at
`factors_path` (`id, period, weight_factor`), and writes the six
structural columns plus the cumulative-product `weight`. The weight
*values* come from R (the `glm` fit); the engine only reproduces their
deterministic accumulation.

## Usage

``` r
expand_weighted_parquet(
  input_path,
  factors_path,
  output_path,
  id_col,
  period_col,
  treatment_col,
  eligible_col,
  outcome_col,
  first_period,
  last_period,
  estimand
)
```

## Arguments

- input_path:

  Path to the input Parquet file.

- factors_path:

  Path to the per-`(id, period)` factor Parquet
  (`id, period, weight_factor`).

- output_path:

  Path where the weighted Parquet is written.

- id_col, period_col, treatment_col:

  Column names in the input.

- eligible_col, outcome_col:

  Eligibility / outcome column names.

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` or `"PP"`; selects the weight *model* upstream, but the
  application arithmetic (join + cumulative product) is identical for
  both.

## Value

`NULL`, invisibly; the weighted expansion is written to `output_path`.
Errors in the core engine surface as R errors.

## Examples

``` r
if (FALSE) { # \dontrun{
expand_weighted_parquet(
  "input.parquet", "factors.parquet", "weighted.parquet",
  "id", "period", "treatment", "eligible", "outcome",
  0L, .Machine$integer.max, "PP"
)
} # }
```
