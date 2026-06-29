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
#' @param id_col,period_col,treatment_col Column names. Defaults match the
#'   TrialEmulation conventions.
#' @param first_period,last_period Inclusive integer period bounds.
#' @return `output_path`, invisibly.
#' @export
expand_trial <- function(input_path,
                         output_path,
                         id_col = "id",
                         period_col = "period",
                         treatment_col = "treatment",
                         first_period = 0L,
                         last_period = .Machine$integer.max) {
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
    first_period = as.integer(first_period),
    last_period = as.integer(last_period)
  )
  invisible(output_path)
}
