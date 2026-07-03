# Expand a cohort data.frame and attach pre-computed weights, in memory (wrapper)

Frame-in / frame-out analogue of
[`expand_trial_weighted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted.md):
takes an in-memory cohort `data.frame` and a pre-computed factor
`data.frame` (`id`, `period`, `weight_factor`), and returns the
weighted, expanded frame as a `data.frame`. Wraps the extendr-generated
[`expand_weighted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_df.md).
A
[`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html)
`id` in either frame round-trips exactly.

## Usage

``` r
expand_trial_weighted_df(
  cohort,
  factors,
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

- cohort:

  A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
  person-time rows. Coerced with
  [`as.data.frame()`](https://rdrr.io/r/base/as.data.frame.html).

- factors:

  A `data.frame` with columns `id`, `period`, `weight_factor`. Coerced
  with [`as.data.frame()`](https://rdrr.io/r/base/as.data.frame.html).

- id_col, period_col, treatment_col, eligible_col, outcome_col:

  Column names. Defaults match the TrialEmulation conventions.

- first_period, last_period:

  Inclusive integer period bounds.

- estimand:

  `"ITT"` or `"PP"`; selects the weight model upstream, but the
  application arithmetic is identical for both.

## Value

A `data.frame` with the six structural columns plus `weight`.

## See also

[`expand_trial_weighted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted.md)
for the Parquet-path equivalent.

## Examples

``` r
cohort <- data.frame(
  id = c(1L, 1L, 1L, 2L, 2L), period = c(0L, 1L, 2L, 0L, 1L),
  treatment = c(1L, 1L, 0L, 0L, 1L), eligible = c(1L, 0L, 0L, 1L, 0L),
  outcome = c(0L, 0L, 1L, 0L, 0L)
)
factors <- data.frame(
  id = c(1L, 1L, 1L, 2L, 2L), period = c(0L, 1L, 2L, 0L, 1L),
  weight_factor = c(1, 0.9, 1.05, 1, 1.1)
)
expand_trial_weighted_df(cohort, factors, estimand = "PP")
#>   id trial_period followup_time assigned_treatment treatment outcome weight
#> 1  1            0             0                  1         1       0    1.0
#> 2  1            0             1                  1         1       0    0.9
#> 3  2            0             0                  0         0       0    1.0
```
