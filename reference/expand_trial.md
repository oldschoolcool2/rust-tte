# Expand a target-trial person-time dataset (ergonomic wrapper)

User-facing wrapper around the extendr-generated
[`expand_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_parquet.md)
that validates inputs and uses sensible defaults. The heavy lifting
happens in the Rust core crate.

## Usage

``` r
expand_trial(
  input_path,
  output_path,
  id_col = "id",
  period_col = "period",
  treatment_col = "treatment",
  eligible_col = "eligible",
  outcome_col = "outcome",
  first_period = 0L,
  last_period = .Machine$integer.max,
  estimand = "ITT"
)
```

## Arguments

- input_path:

  Path to an existing input Parquet file.

- output_path:

  Path to write the expanded Parquet file.

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` (intention-to-treat, no artificial censoring) or `"PP"`
  (per-protocol, censor each trial at the first treatment deviation).

## Value

`output_path`, invisibly.

## Examples

``` r
if (FALSE) { # \dontrun{
expand_trial("input.parquet", "expanded.parquet", estimand = "PP")
} # }
```
