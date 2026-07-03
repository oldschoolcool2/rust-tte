# Expand a sequence of target trials with the Rust + Polars engine

A drop-in replacement for `TrialEmulation::expand_trials()` that runs
the expensive expansion in Rust (`tters`) instead of R, then stores the
result through the `trial_sequence`'s registered `te_datastore`. The
produced frame is byte-equivalent to the default path (structural
columns bit-exact, `weight` to within machine precision), so the
downstream — `load_expanded_data()`, `sample_controls()`, `fit_msm()` —
behaves identically.

## Usage

``` r
expand_trials_tters(object, fallback = TRUE, quiet = FALSE)
```

## Arguments

- object:

  A configured `trial_sequence` (ITT or PP). The AT estimand is not yet
  supported and falls back to R.

- fallback:

  If `TRUE` (default), any failure of the Rust path (including an
  unsupported estimand or a missing toolchain) falls back to
  `TrialEmulation::expand_trials()` with a message. If `FALSE`, the
  error is raised.

- quiet:

  If `TRUE`, suppress the fallback message.

## Value

The updated `trial_sequence`, with its `@expansion@datastore` populated
— the same object type `TrialEmulation::expand_trials()` returns.

## Details

Estimation stays entirely in R. Weight *models* are fit by
`calculate_weights()`; this function reads that per-period `wt` verbatim
and Rust performs only the deterministic expansion and weight
accumulation.

Set up the `trial_sequence` exactly as for
`TrialEmulation::expand_trials()` (`set_data()` -\> optional weight
models + `calculate_weights()` -\> `set_outcome_model()` -\>
`set_expansion_options()`), then call this instead of `expand_trials()`.
The registered output may be
[`save_to_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/save_to_tters.md)
or any other `te_datastore` (e.g. `save_to_datatable()`); the speedup
comes from the Rust expansion, not the store.

## See also

[`save_to_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/save_to_tters.md);
`TrialEmulation::expand_trials()`.

## Examples

``` r
# \donttest{
if (requireNamespace("TrialEmulation", quietly = TRUE)) {
  library(TrialEmulation)
  data("data_censored")
  trial <- trial_sequence("ITT") |>
    set_data(data = data_censored) |>
    set_censor_weight_model(
      censor_event = "censored", numerator = ~x2, denominator = ~ x2 + x1,
      pool_models = "numerator",
      model_fitter = stats_glm_logit(save_path = tempfile())
    ) |>
    calculate_weights() |>
    set_outcome_model(adjustment_terms = ~x2) |>
    set_expansion_options(output = save_to_tters(), chunk_size = 0)
  trial <- expand_trials_tters(trial)
  load_expanded_data(trial, seed = 1234, p_control = 0.5)
}
# }
```
