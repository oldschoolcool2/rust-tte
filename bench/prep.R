# bench/prep.R — time TrialEmulation::data_preparation (ITT expansion) on an input
# parquet, as the upstream baseline. Whole-process peak RSS is measured by the
# `/usr/bin/time -v` wrapper around this process (see bench/run_bench.sh); this
# script reports the input/expanded row counts and the prep-only wall time.
#
# Usage: /usr/bin/time -v Rscript bench/prep.R <input.parquet>
suppressPackageStartupMessages({ library(arrow); library(TrialEmulation) })

pq <- commandArgs(trailingOnly = TRUE)[[1]]
df <- as.data.frame(arrow::read_parquet(pq))

# data_dir is a REQUIRED argument even with separate_files=FALSE (it writes no
# files in that mode, so the in-memory peak reflects the full expansion).
dd <- file.path(tempdir(), paste0("dp_", paste(sample(letters, 8, TRUE), collapse = "")))
dir.create(dd, showWarnings = FALSE, recursive = TRUE)

el <- system.time(
  prep <- TrialEmulation::data_preparation(
    data = df, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible", estimand_type = "ITT",
    outcome_cov = ~1, use_censor_weights = FALSE,
    data_dir = dd, separate_files = FALSE, quiet = TRUE
  )
)[["elapsed"]]

cat(sprintf("R input_rows=%d expanded_rows=%d prep_wall_s=%.3f\n",
            nrow(df), nrow(prep$data), el))
