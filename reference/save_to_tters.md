# Create a `te_datastore_tters` storage backend

Constructor (the `save_to_*` convention) for the Rust-backed
`te_datastore` subclass. Like the reference backends it does no work —
it returns an empty store to hand to
`TrialEmulation::set_expansion_options()`. The expansion is run later by
[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md).

## Usage

``` r
save_to_tters()
```

## Value

A `te_datastore_tters` object with `N = 0L` and an empty data slot.

## Details

Requires the `TrialEmulation` (and `data.table`) package: the returned
object is an S4 subclass of `TrialEmulation`'s `te_datastore`, so the
class only exists when `TrialEmulation` is installed.

## See also

[`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md)
to populate it with a Rust-fast expansion.

## Examples

``` r
if (FALSE) { # \dontrun{
library(TrialEmulation)
trial_sequence("ITT") |>
  set_data(data = data_censored) |>
  set_outcome_model(adjustment_terms = ~x2) |>
  set_expansion_options(output = save_to_tters(), chunk_size = 0)
} # }
```
