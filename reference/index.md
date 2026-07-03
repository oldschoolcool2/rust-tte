# Package index

## TrialEmulation backend

Drop-in te_datastore backend — expand in Rust, estimate in R,
bit-identically to the default path.

- [`expand_trials_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trials_tters.md)
  : Expand a sequence of target trials with the Rust + Polars engine

- [`save_to_tters()`](https://oldschoolcool2.github.io/rust-tte/reference/save_to_tters.md)
  :

  Create a `te_datastore_tters` storage backend

## Trial expansion (Parquet path)

Read a person-time Parquet file, write the expanded trial frame.

- [`expand_trial()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial.md)
  : Expand a target-trial person-time dataset (ergonomic wrapper)
- [`expand_trial_weighted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted.md)
  : Expand a dataset and attach pre-computed inverse-probability weights
- [`expand_trial_weighted_fitted()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted.md)
  : Fit IPW weights and expand a cohort into a weighted trial frame
  (ergonomic wrapper)
- [`fit_trial_weights()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights.md)
  : Fit inverse-probability weights for a target-trial cohort (ergonomic
  wrapper)

## Trial expansion (in-memory data.frame)

Frame-in / frame-out, with no intermediate Parquet.

- [`expand_trial_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_df.md)
  : Expand a target-trial cohort data.frame in memory (ergonomic
  wrapper)
- [`expand_trial_weighted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_df.md)
  : Expand a cohort data.frame and attach pre-computed weights, in
  memory (wrapper)
- [`expand_trial_weighted_fitted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_trial_weighted_fitted_df.md)
  : Fit IPW weights and expand a cohort data.frame in one call, in
  memory (wrapper)
- [`fit_trial_weights_df()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_trial_weights_df.md)
  : Fit inverse-probability weights for a cohort data.frame, in memory
  (wrapper)

## Low-level engine bindings

Thin extendr shims wrapped by the ergonomic functions above.

- [`expand_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_parquet.md)
  :

  Expand a prepared person-time Parquet dataset into the sequential
  target-trial layout and write the result to `output_path`.

- [`expand_weighted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_parquet.md)
  :

  Expand a person-time Parquet dataset and attach pre-computed
  inverse-probability weights, writing the weighted frame to
  `output_path`.

- [`expand_weighted_fitted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_parquet.md)
  : Fit the IPW weights in Rust, expand the cohort, apply the weights,
  and write the weighted trial frame — a raw cohort to a weighted,
  expanded frame in one call (no pre-computed factor table).

- [`fit_weights_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_parquet.md)
  :

  Fit the inverse-probability **weight factor** for a Parquet cohort in
  Rust and write the per-`(id, period)` factor table
  (`id, period, weight_factor`).

- [`expand_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_df.md)
  :

  Expand an in-memory cohort `data.frame` into the sequential
  target-trial layout and return the result as a `data.frame` — the
  frame-in/frame-out analogue of
  [`expand_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_parquet.md),
  with no intermediate Parquet.

- [`expand_weighted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_df.md)
  :

  Expand an in-memory cohort and attach pre-computed inverse-probability
  weights, returning the weighted frame as a `data.frame` — the
  frame-in/frame-out analogue of
  [`expand_weighted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_parquet.md).

- [`expand_weighted_fitted_df()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_df.md)
  :

  Fit the IPW weights for an in-memory cohort, expand, apply, and return
  the weighted trial frame as a `data.frame` — a raw cohort `data.frame`
  straight to a weighted, expanded `data.frame` in one call (no
  pre-computed factor table, no intermediate Parquet). The
  frame-in/frame-out analogue of
  [`expand_weighted_fitted_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/expand_weighted_fitted_parquet.md).
  A 64-bit integer `id`
  ([`bit64::integer64`](https://bit64.r-lib.org/reference/bit64-package.html))
  round-trips exactly.

- [`fit_weights_df()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_df.md)
  :

  Fit the inverse-probability weight factor for an in-memory cohort and
  return the per-`(id, period)` factor table
  (`id, period, weight_factor`) as a `data.frame` — the
  frame-in/frame-out analogue of
  [`fit_weights_parquet()`](https://oldschoolcool2.github.io/rust-tte/reference/fit_weights_parquet.md).

## Package

- [`tters`](https://oldschoolcool2.github.io/rust-tte/reference/tters-package.md)
  [`tters-package`](https://oldschoolcool2.github.io/rust-tte/reference/tters-package.md)
  : tters: Sequential Target Trial Emulation Data Expansion
