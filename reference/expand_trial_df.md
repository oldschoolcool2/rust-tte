# Expand a target-trial cohort data.frame in memory (ergonomic wrapper)

Frame-in / frame-out analogue of
[`expand_trial()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial.md):
takes an in-memory cohort `data.frame` and returns the expanded trial
frame as a `data.frame`, with no intermediate Parquet. Wraps the
extendr-generated
[`expand_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_df.md).

## Usage

``` r
expand_trial_df(
  cohort,
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

- cohort:

  A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
  person-time rows. Coerced with
  [`as.data.frame()`](https://rdrr.io/r/base/as.data.frame.html), which
  preserves an `integer64` column's class.

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` (intention-to-treat, no artificial censoring) or `"PP"`
  (per-protocol, censor each trial at the first treatment deviation).

## Value

A `data.frame` with the six structural columns (`id`, `trial_period`,
`followup_time`, `assigned_treatment`, `treatment`, `outcome`); an
`integer64` input column is returned as `integer64`.

## Details

Column dtypes are preserved exactly: R `integer` \<-\> Polars `Int32`,
`double` \<-\> `Float64`, and
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
\<-\> `Int64`. A 64-bit integer column (e.g. a large `id`) round-trips
exactly as `integer64` with no precision loss above `2^53` (a pure-safe
bit reinterpret, not a numeric cast).

## See also

[`expand_trial()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial.md)
for the Parquet-path equivalent.

## Examples

``` r
if (FALSE) { # \dontrun{
cohort <- arrow::read_parquet("input.parquet")
expanded <- expand_trial_df(cohort, estimand = "PP")
} # }
```
