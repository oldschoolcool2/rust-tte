# Expand a prepared person-time Parquet dataset into the sequential target-trial layout and write the result to `output_path`.

This is a thin FFI shim. All dtype-exact, deterministic Polars work
lives in the `tte_expand` core crate (which is
`#![forbid(unsafe_code)]`). The binding crate cannot forbid unsafe
because the extendr macros emit the FFI registrar. Every
`tte_expand::ExpandError` is mapped to an R error condition.

## Usage

``` r
expand_parquet(
  input_path,
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

- output_path:

  Path where the expanded Parquet is written.

- id_col, period_col, treatment_col:

  Column names in the input.

- eligible_col, outcome_col:

  Eligibility / outcome column names (`TrialEmulation` defaults are
  `"eligible"` / `"outcome"`).

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` (no artificial censoring) or `"PP"` (per-protocol, censor each
  trial at the first treatment deviation). Case-insensitive.

## Value

`NULL`, invisibly; the expansion is written to `output_path`. Errors in
the core engine surface as R errors.

## Examples

``` r
if (FALSE) { # \dontrun{
expand_parquet(
  "input.parquet", "expanded.parquet",
  "id", "period", "treatment", "eligible", "outcome",
  0L, .Machine$integer.max, "ITT"
)
} # }
```
