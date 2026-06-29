#' tters: Sequential Target Trial Emulation Data Expansion
#'
#' A thin R binding over the verified `tte-expand` Rust + Polars engine.
#'
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
