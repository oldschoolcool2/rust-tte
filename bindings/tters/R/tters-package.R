# The importFrom tags make the hard runtime deps explicit to R CMD check.
# `bit64` stays a load-only dependency: importing `integer64` neither attaches
# bit64 nor changes the exact int64 round-trip — the Rust side constructs the
# classed vector directly (see docs/011-phase-9-exact-wide-integer-frame-io/).

#' tters: Sequential Target Trial Emulation Data Expansion
#'
#' A thin R binding over the verified `tte-expand` Rust + Polars engine.
#'
#' @importFrom bit64 integer64
#' @importFrom methods is isClass new setClass setMethod
#' @keywords internal
"_PACKAGE"

#' Expand a target-trial person-time dataset (ergonomic wrapper)
#'
#' User-facing wrapper around the extendr-generated [expand_parquet()] that
#' validates inputs and uses sensible defaults. The heavy lifting happens in the
#' Rust core crate.
#'
#' @param input_path Path to an existing input Parquet file.
#' @param output_path Path to write the expanded Parquet file.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` (intention-to-treat, no artificial censoring) or
#'   `"PP"` (per-protocol, censor each trial at the first treatment deviation).
#' @return `output_path`, invisibly.
#' @examples
#' \dontrun{
#' expand_trial("input.parquet", "expanded.parquet", estimand = "PP")
#' }
#' @export
expand_trial <- function(input_path,
                         output_path,
                         id_col = "id",
                         period_col = "period",
                         treatment_col = "treatment",
                         eligible_col = "eligible",
                         outcome_col = "outcome",
                         first_period = 0L,
                         last_period = .Machine$integer.max,
                         estimand = "ITT") {
  stopifnot(
    file.exists(input_path),
    is.character(output_path), length(output_path) == 1L
  )
  expand_parquet(
    input_path = input_path,
    output_path = output_path,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand
  )
  invisible(output_path)
}

#' Expand a dataset and attach pre-computed inverse-probability weights
#'
#' User-facing wrapper around the extendr-generated [expand_weighted_parquet()].
#' It expands `input_path` under `estimand`, joins the per-`(id, period)` factor
#' table at `factors_path` (`id, period, weight_factor`), and writes the six
#' structural columns plus the cumulative-product `weight`. Weight *values* are
#' produced upstream in R; the engine reproduces only their deterministic
#' accumulation.
#'
#' @param input_path Path to an existing input Parquet file.
#' @param factors_path Path to the per-`(id, period)` factor Parquet file.
#' @param output_path Path to write the weighted Parquet file.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` or `"PP"`; selects the weight model upstream, but the
#'   application arithmetic is identical for both.
#' @return `output_path`, invisibly.
#' @examples
#' \dontrun{
#' expand_trial_weighted(
#'   "input.parquet", "factors.parquet", "weighted.parquet", estimand = "PP"
#' )
#' }
#' @export
expand_trial_weighted <- function(input_path,
                                  factors_path,
                                  output_path,
                                  id_col = "id",
                                  period_col = "period",
                                  treatment_col = "treatment",
                                  eligible_col = "eligible",
                                  outcome_col = "outcome",
                                  first_period = 0L,
                                  last_period = .Machine$integer.max,
                                  estimand = "PP") {
  stopifnot(
    file.exists(input_path),
    file.exists(factors_path),
    is.character(output_path), length(output_path) == 1L
  )
  expand_weighted_parquet(
    input_path = input_path,
    factors_path = factors_path,
    output_path = output_path,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand
  )
  invisible(output_path)
}

# Internal: resolve the NULL-driven ergonomic weight arguments into the flat
# `(use_*, character-vector)` form the extendr shims expect. A switching model is
# present iff either switch covariate vector is supplied; an IPCW censoring model
# is present iff `censor_col` is supplied. `NULL` covariate vectors collapse to
# `character(0)` (an intercept-only model); an absent component is dropped, not
# emptied. Not exported.
.tters_weight_spec <- function(switch_numerator, switch_denominator,
                               censor_col, censor_numerator, censor_denominator) {
  as_chr <- function(x) if (is.null(x)) character(0) else as.character(x)
  use_switch <- !is.null(switch_numerator) || !is.null(switch_denominator)
  use_censor <- !is.null(censor_col)
  if (use_censor) {
    stopifnot(is.character(censor_col), length(censor_col) == 1L)
  }
  list(
    use_switch = use_switch,
    switch_numerator = as_chr(switch_numerator),
    switch_denominator = as_chr(switch_denominator),
    use_censor = use_censor,
    censor_col = if (use_censor) as.character(censor_col) else "",
    censor_numerator = as_chr(censor_numerator),
    censor_denominator = as_chr(censor_denominator)
  )
}

#' Fit inverse-probability weights for a target-trial cohort (ergonomic wrapper)
#'
#' User-facing wrapper around the extendr-generated [fit_weights_parquet()] that
#' *fits* the IPW switching and/or IPCW censoring models in Rust and writes the
#' per-`(id, period)` factor table (`id, period, weight_factor`) — the table
#' [expand_trial_weighted()] consumes. Unlike that pre-computed-factor path, here
#' the weight *models* are fitted in Rust (the `weights-fit` surface): a
#' faithful port of `TrialEmulation`'s design preparation plus a deterministic
#' binomial-logit solver. Robust/sandwich variance and the marginal structural
#' model stay in R.
#'
#' A switching model is fitted when either `switch_numerator` or
#' `switch_denominator` is non-`NULL`; an IPCW censoring model is fitted when
#' `censor_col` is non-`NULL`. Covariates are character vectors of column names;
#' `character(0)` (or `NULL`) yields an intercept-only model.
#'
#' @param input_path Path to an existing input Parquet cohort (long person-time).
#' @param output_path Path to write the `(id, period, weight_factor)` Parquet.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` or `"PP"`. Per-protocol runs the artificial-censoring
#'   state machine and the switching models; intention-to-treat skips both.
#' @param switch_numerator,switch_denominator Character vectors of covariate column
#'   names for the switching numerator (stabiliser) / denominator models, or `NULL`
#'   (the default) to omit switching weights.
#' @param censor_col Name of the `{0,1}` censoring-indicator column; the modelled
#'   response is `1 - censor_col`. `NULL` (the default) omits IPCW weights.
#' @param censor_numerator,censor_denominator Character vectors of covariate column
#'   names for the IPCW numerator / denominator models.
#' @param pool_censor How the IPCW models are pooled across the previous-treatment
#'   strata: `"none"`, `"numerator"`, or `"both"`.
#' @return `output_path`, invisibly.
#' @seealso [expand_trial_weighted_fitted()] to fit and expand in a single call.
#' @examples
#' \dontrun{
#' # Per-protocol switching weights (numerator ~ x2, denominator ~ x2 + x1):
#' fit_trial_weights(
#'   "cohort.parquet", "factors.parquet", estimand = "PP",
#'   switch_numerator = "x2", switch_denominator = c("x2", "x1")
#' )
#' }
#' @export
fit_trial_weights <- function(input_path,
                              output_path,
                              id_col = "id",
                              period_col = "period",
                              treatment_col = "treatment",
                              eligible_col = "eligible",
                              outcome_col = "outcome",
                              first_period = 0L,
                              last_period = .Machine$integer.max,
                              estimand = "PP",
                              switch_numerator = NULL,
                              switch_denominator = NULL,
                              censor_col = NULL,
                              censor_numerator = NULL,
                              censor_denominator = NULL,
                              pool_censor = "none") {
  stopifnot(
    file.exists(input_path),
    is.character(output_path), length(output_path) == 1L
  )
  spec <- .tters_weight_spec(
    switch_numerator, switch_denominator,
    censor_col, censor_numerator, censor_denominator
  )
  fit_weights_parquet(
    input_path = input_path,
    output_path = output_path,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand,
    use_switch = spec$use_switch,
    switch_numerator = spec$switch_numerator,
    switch_denominator = spec$switch_denominator,
    use_censor = spec$use_censor,
    censor_col = spec$censor_col,
    censor_numerator = spec$censor_numerator,
    censor_denominator = spec$censor_denominator,
    pool_censor = pool_censor
  )
  invisible(output_path)
}

#' Fit IPW weights and expand a cohort into a weighted trial frame (ergonomic wrapper)
#'
#' User-facing wrapper around the extendr-generated
#' [expand_weighted_fitted_parquet()]. It takes a raw person-time cohort straight
#' to a weighted, expanded trial frame in one call — fitting the switching and/or
#' IPCW models in Rust (no pre-computed factor table), expanding under `estimand`,
#' and accumulating the fitted factor into the cumulative `weight`. The six
#' structural columns are bit-exact; `weight` matches the Oracle within the staged
#' ~1e-6 tolerance. Robust/sandwich variance and the marginal structural model
#' stay in R.
#'
#' Model presence follows the same rule as [fit_trial_weights()]: a switching model
#' is fitted when either `switch_*` covariate vector is non-`NULL`; an IPCW model is
#' fitted when `censor_col` is non-`NULL`.
#'
#' @inheritParams fit_trial_weights
#' @param output_path Path to write the weighted, expanded Parquet.
#' @return `output_path`, invisibly.
#' @seealso [fit_trial_weights()] to write only the `(id, period, weight_factor)`
#'   factor table.
#' @examples
#' \dontrun{
#' # Per-protocol switch + IPCW censoring, raw cohort to weighted frame in one call:
#' expand_trial_weighted_fitted(
#'   "cohort.parquet", "weighted.parquet", estimand = "PP",
#'   switch_numerator = "x2", switch_denominator = c("x2", "x1"),
#'   censor_col = "censored",
#'   censor_numerator = "x2", censor_denominator = c("x2", "x1"),
#'   pool_censor = "none"
#' )
#' }
#' @export
expand_trial_weighted_fitted <- function(input_path,
                                         output_path,
                                         id_col = "id",
                                         period_col = "period",
                                         treatment_col = "treatment",
                                         eligible_col = "eligible",
                                         outcome_col = "outcome",
                                         first_period = 0L,
                                         last_period = .Machine$integer.max,
                                         estimand = "PP",
                                         switch_numerator = NULL,
                                         switch_denominator = NULL,
                                         censor_col = NULL,
                                         censor_numerator = NULL,
                                         censor_denominator = NULL,
                                         pool_censor = "none") {
  stopifnot(
    file.exists(input_path),
    is.character(output_path), length(output_path) == 1L
  )
  spec <- .tters_weight_spec(
    switch_numerator, switch_denominator,
    censor_col, censor_numerator, censor_denominator
  )
  expand_weighted_fitted_parquet(
    input_path = input_path,
    output_path = output_path,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand,
    use_switch = spec$use_switch,
    switch_numerator = spec$switch_numerator,
    switch_denominator = spec$switch_denominator,
    use_censor = spec$use_censor,
    censor_col = spec$censor_col,
    censor_numerator = spec$censor_numerator,
    censor_denominator = spec$censor_denominator,
    pool_censor = pool_censor
  )
  invisible(output_path)
}

# In-memory (frame-in / frame-out) wrappers. These mirror the path
# wrappers above but take an in-memory cohort `data.frame` and RETURN a
# `data.frame`, with no intermediate Parquet. The cohort is coerced with
# `as.data.frame()` so tibbles, data.tables, and Arrow Tables are all accepted;
# columns marshal dtype-exactly (R `integer` <-> Int32, `double` <-> Float64).

#' Expand a target-trial cohort data.frame in memory (ergonomic wrapper)
#'
#' Frame-in / frame-out analogue of [expand_trial()]: takes an in-memory cohort
#' `data.frame` and returns the expanded trial frame as a `data.frame`, with no
#' intermediate Parquet. Wraps the extendr-generated [expand_df()].
#'
#' Column dtypes are preserved exactly: R `integer` <-> Polars `Int32`, `double`
#' <-> `Float64`, and `bit64::integer64` <-> `Int64`. A 64-bit integer column
#' (e.g. a large `id`) round-trips exactly as `integer64` with no precision loss
#' above `2^53` (a pure-safe bit reinterpret, not a numeric cast).
#'
#' @param cohort A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
#'   person-time rows. Coerced with `as.data.frame()`, which preserves an
#'   `integer64` column's class.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` (intention-to-treat, no artificial censoring) or
#'   `"PP"` (per-protocol, censor each trial at the first treatment deviation).
#' @return A `data.frame` with the six structural columns
#'   (`id`, `trial_period`, `followup_time`, `assigned_treatment`, `treatment`,
#'   `outcome`); an `integer64` input column is returned as `integer64`.
#' @seealso [expand_trial()] for the Parquet-path equivalent.
#' @examples
#' \dontrun{
#' cohort <- arrow::read_parquet("input.parquet")
#' expanded <- expand_trial_df(cohort, estimand = "PP")
#' }
#' @export
expand_trial_df <- function(cohort,
                            id_col = "id",
                            period_col = "period",
                            treatment_col = "treatment",
                            eligible_col = "eligible",
                            outcome_col = "outcome",
                            first_period = 0L,
                            last_period = .Machine$integer.max,
                            estimand = "ITT") {
  cohort <- as.data.frame(cohort)
  stopifnot(is.data.frame(cohort))
  expand_df(
    cohort = cohort,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand
  )
}

#' Expand a cohort data.frame and attach pre-computed weights, in memory (wrapper)
#'
#' Frame-in / frame-out analogue of [expand_trial_weighted()]: takes an in-memory
#' cohort `data.frame` and a pre-computed factor `data.frame`
#' (`id`, `period`, `weight_factor`), and returns the weighted, expanded frame as a
#' `data.frame`. Wraps the extendr-generated [expand_weighted_df()]. A
#' `bit64::integer64` `id` in either frame round-trips exactly.
#'
#' @param cohort A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
#'   person-time rows. Coerced with `as.data.frame()`.
#' @param factors A `data.frame` with columns `id`, `period`, `weight_factor`.
#'   Coerced with `as.data.frame()`.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` or `"PP"`; selects the weight model upstream, but the
#'   application arithmetic is identical for both.
#' @return A `data.frame` with the six structural columns plus `weight`.
#' @seealso [expand_trial_weighted()] for the Parquet-path equivalent.
#' @examples
#' \dontrun{
#' weighted <- expand_trial_weighted_df(cohort, factors, estimand = "PP")
#' }
#' @export
expand_trial_weighted_df <- function(cohort,
                                     factors,
                                     id_col = "id",
                                     period_col = "period",
                                     treatment_col = "treatment",
                                     eligible_col = "eligible",
                                     outcome_col = "outcome",
                                     first_period = 0L,
                                     last_period = .Machine$integer.max,
                                     estimand = "PP") {
  cohort <- as.data.frame(cohort)
  factors <- as.data.frame(factors)
  stopifnot(is.data.frame(cohort), is.data.frame(factors))
  expand_weighted_df(
    cohort = cohort,
    factors = factors,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand
  )
}

#' Fit inverse-probability weights for a cohort data.frame, in memory (wrapper)
#'
#' Frame-in / frame-out analogue of [fit_trial_weights()]: fits the IPW
#' switching and/or IPCW censoring models in Rust from an in-memory cohort
#' `data.frame` and returns the per-`(id, period)` factor table as a `data.frame`
#' (`id`, `period`, `weight_factor`) — the table [expand_trial_weighted_df()]
#' consumes. Wraps the extendr-generated [fit_weights_df()]. A `bit64::integer64`
#' `id` round-trips exactly (the returned `id` is `integer64`).
#'
#' Model presence follows the same `NULL`-driven rule as [fit_trial_weights()]: a
#' switching model is fitted when either `switch_*` covariate vector is non-`NULL`;
#' an IPCW model is fitted when `censor_col` is non-`NULL`.
#'
#' @param cohort A `data.frame` (or tibble / `data.table` / Arrow `Table`) of long
#'   person-time rows. Coerced with `as.data.frame()`.
#' @param id_col,period_col,treatment_col,eligible_col,outcome_col Column names.
#'   Defaults match the TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @param estimand `"ITT"` or `"PP"`.
#' @param switch_numerator,switch_denominator Character vectors of covariate column
#'   names for the switching numerator / denominator models, or `NULL` to omit
#'   switching weights.
#' @param censor_col Name of the `{0,1}` censoring-indicator column; the modelled
#'   response is `1 - censor_col`. `NULL` omits IPCW weights.
#' @param censor_numerator,censor_denominator Character vectors of covariate column
#'   names for the IPCW numerator / denominator models.
#' @param pool_censor How the IPCW models are pooled across the previous-treatment
#'   strata: `"none"`, `"numerator"`, or `"both"`.
#' @return A `data.frame` with columns `id`, `period`, `weight_factor`.
#' @seealso [fit_trial_weights()] for the Parquet-path equivalent;
#'   [expand_trial_weighted_fitted_df()] to fit and expand in a single call.
#' @examples
#' \dontrun{
#' factors <- fit_trial_weights_df(
#'   cohort, estimand = "PP",
#'   switch_numerator = "x2", switch_denominator = c("x2", "x1")
#' )
#' }
#' @export
fit_trial_weights_df <- function(cohort,
                                 id_col = "id",
                                 period_col = "period",
                                 treatment_col = "treatment",
                                 eligible_col = "eligible",
                                 outcome_col = "outcome",
                                 first_period = 0L,
                                 last_period = .Machine$integer.max,
                                 estimand = "PP",
                                 switch_numerator = NULL,
                                 switch_denominator = NULL,
                                 censor_col = NULL,
                                 censor_numerator = NULL,
                                 censor_denominator = NULL,
                                 pool_censor = "none") {
  cohort <- as.data.frame(cohort)
  stopifnot(is.data.frame(cohort))
  spec <- .tters_weight_spec(
    switch_numerator, switch_denominator,
    censor_col, censor_numerator, censor_denominator
  )
  fit_weights_df(
    cohort = cohort,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand,
    use_switch = spec$use_switch,
    switch_numerator = spec$switch_numerator,
    switch_denominator = spec$switch_denominator,
    use_censor = spec$use_censor,
    censor_col = spec$censor_col,
    censor_numerator = spec$censor_numerator,
    censor_denominator = spec$censor_denominator,
    pool_censor = pool_censor
  )
}

#' Fit IPW weights and expand a cohort data.frame in one call, in memory (wrapper)
#'
#' Frame-in / frame-out analogue of [expand_trial_weighted_fitted()]: takes a raw
#' cohort `data.frame` straight to a weighted, expanded `data.frame` in one call —
#' fitting the switching and/or IPCW models in Rust (no pre-computed factor table),
#' expanding under `estimand`, and accumulating the fitted factor into the
#' cumulative `weight`. The six structural columns are bit-exact; `weight` matches
#' the Oracle within the staged ~1e-6 tolerance. Wraps the extendr-generated
#' [expand_weighted_fitted_df()]. A `bit64::integer64` `id` round-trips exactly.
#'
#' Model presence follows the same rule as [fit_trial_weights_df()].
#'
#' @inheritParams fit_trial_weights_df
#' @return A `data.frame` with the six structural columns plus `weight`.
#' @seealso [expand_trial_weighted_fitted()] for the Parquet-path equivalent;
#'   [fit_trial_weights_df()] to return only the factor table.
#' @examples
#' \dontrun{
#' weighted <- expand_trial_weighted_fitted_df(
#'   cohort, estimand = "PP",
#'   switch_numerator = "x2", switch_denominator = c("x2", "x1"),
#'   censor_col = "censored",
#'   censor_numerator = "x2", censor_denominator = c("x2", "x1"),
#'   pool_censor = "none"
#' )
#' }
#' @export
expand_trial_weighted_fitted_df <- function(cohort,
                                            id_col = "id",
                                            period_col = "period",
                                            treatment_col = "treatment",
                                            eligible_col = "eligible",
                                            outcome_col = "outcome",
                                            first_period = 0L,
                                            last_period = .Machine$integer.max,
                                            estimand = "PP",
                                            switch_numerator = NULL,
                                            switch_denominator = NULL,
                                            censor_col = NULL,
                                            censor_numerator = NULL,
                                            censor_denominator = NULL,
                                            pool_censor = "none") {
  cohort <- as.data.frame(cohort)
  stopifnot(is.data.frame(cohort))
  spec <- .tters_weight_spec(
    switch_numerator, switch_denominator,
    censor_col, censor_numerator, censor_denominator
  )
  expand_weighted_fitted_df(
    cohort = cohort,
    id_col = id_col,
    period_col = period_col,
    treatment_col = treatment_col,
    eligible_col = eligible_col,
    outcome_col = outcome_col,
    first_period = as.integer(first_period),
    last_period = as.integer(last_period),
    estimand = estimand,
    use_switch = spec$use_switch,
    switch_numerator = spec$switch_numerator,
    switch_denominator = spec$switch_denominator,
    use_censor = spec$use_censor,
    censor_col = spec$censor_col,
    censor_numerator = spec$censor_numerator,
    censor_denominator = spec$censor_denominator,
    pool_censor = pool_censor
  )
}
