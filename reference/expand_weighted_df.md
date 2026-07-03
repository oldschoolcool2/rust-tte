# Expand an in-memory cohort and attach pre-computed inverse-probability weights, returning the weighted frame as a `data.frame` — the frame-in/frame-out analogue of `expand_weighted_parquet()`.

Both the cohort and the per-`(id, period)` factor table
(`id, period, weight_factor`) are passed as R `data.frame`s; the engine
expands under `estimand`, joins the factors, and accumulates the
cumulative-product `weight`. A 64-bit integer `id`
([`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html))
in either frame round-trips exactly.

## Usage

``` r
expand_weighted_df(
  cohort,
  factors,
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

- factors:

  An R `data.frame` with columns `id`, `period`, `weight_factor`.

- id_col, period_col, treatment_col:

  Column names in `cohort`.

- eligible_col, outcome_col:

  Eligibility / outcome column names.

- first_period, last_period:

  Inclusive integer bounds on `trial_period`.

- estimand:

  `"ITT"` or `"PP"`. Case-insensitive.

## Value

A `data.frame` with the six structural columns plus `weight`. Errors in
the core engine surface as R errors.
