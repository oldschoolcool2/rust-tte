# Expand an in-memory cohort `data.frame` into the sequential target-trial layout and return the result as a `data.frame` — the frame-in/frame-out analogue of `expand_parquet()`, with no intermediate Parquet.

The cohort arrives as an R `data.frame` (a `list` of equal-length
columns); columns are marshalled dtype-exactly into a Polars frame (R
`integer` -\> `Int32`, `double` -\> `Float64`,
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
-\> `Int64`), expanded by the verified core, and the six structural
columns are marshalled back to an R `data.frame`. A 64-bit integer
column (an `integer64`, e.g. a large `id`) round-trips exactly via a
pure-safe bit reinterpret (no precision loss).

## Usage

``` r
expand_df(
  cohort,
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

- cohort:

  An R `data.frame` of long person-time rows.

- id_col, period_col, treatment_col:

  Column names in `cohort`.

- eligible_col, outcome_col:

  Eligibility / outcome column names.

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` or `"PP"`. Case-insensitive.

## Value

A `data.frame` with the six structural columns (an `integer64` input
column is returned as `integer64`). Errors in the core engine surface as
R errors.
